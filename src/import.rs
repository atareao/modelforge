//! Lógica del subcomando `import`: selección del quant, descarga reanudable con
//! progreso y reporte del resultado.
//!
//! Toda la interacción con la red llega a través del trait `crate::hf::HfClient`,
//! de modo que la orquestación se prueba sin conexión con un doble de prueba.

use std::path::{Component, Path, PathBuf};

use serde::Serialize;
use tokio::fs::{self, File, OpenOptions};
use tokio::io::AsyncWriteExt as _;

use crate::error::AppError;
use crate::format::format_bytes;
use crate::hf::{HfClient, ProgressSink, RemoteFile, RepoId};
use crate::quant;

/// Opciones del subcomando `import`.
#[derive(Debug, Clone)]
pub struct ImportOptions<'a> {
    /// Repositorio de HuggingFace.
    pub repo: &'a RepoId,
    /// Quant a descargar (p. ej. `Q6_K`).
    pub quant: &'a str,
    /// Descarga también el `mmproj`.
    pub mmproj: bool,
    /// Directorio de destino.
    pub dest: &'a Path,
}

/// Fichero descargado dentro del reporte.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DownloadedFile {
    /// Ruta relativa dentro del repositorio.
    pub path: String,
    /// Tamaño en bytes.
    pub size: u64,
    /// `true` si ya estaba completo y no se volvió a descargar.
    pub cached: bool,
}

/// Resultado de una importación, serializable para `--json`.
#[derive(Debug, Clone, Serialize)]
pub struct ImportReport {
    /// Repositorio (`owner/name`).
    pub repo: String,
    /// Quant descargado.
    pub quant: String,
    /// Directorio de destino.
    pub dest: PathBuf,
    /// Ficheros descargados.
    pub files: Vec<DownloadedFile>,
}

/// Decisión de reanudación para un fichero, a partir de los tamaños conocidos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeAction {
    /// El fichero final ya está completo.
    AlreadyComplete,
    /// El fichero parcial tiene todos los bytes: solo falta renombrarlo.
    FinalizePartial,
    /// Hay que pedir el resto a partir de `offset` y anexarlo.
    Fetch {
        /// Byte desde el que continuar.
        offset: u64,
    },
}

/// Calcula cómo continuar una descarga.
///
/// `final_len` y `part_len` son los tamaños del fichero final y del `.part` si
/// existen; `remote` es el tamaño remoto (0 si el Hub no lo reporta).
#[must_use]
pub fn resume_plan(final_len: Option<u64>, part_len: Option<u64>, remote: u64) -> ResumeAction {
    if let Some(final_size) = final_len {
        let complete = if remote > 0 {
            final_size >= remote
        } else {
            final_size > 0
        };
        if complete {
            return ResumeAction::AlreadyComplete;
        }
    }

    if let Some(partial) = part_len
        && remote > 0
        && partial >= remote
    {
        return ResumeAction::FinalizePartial;
    }

    ResumeAction::Fetch {
        offset: part_len.unwrap_or(0),
    }
}

/// Descarga el GGUF del quant pedido (y su `mmproj`) con reanudación y progreso.
///
/// # Errors
///
/// Propaga los errores de selección ([`crate::quant::select`]) y de red/E/S.
pub async fn run<C: HfClient + Sync>(
    client: &C,
    opts: &ImportOptions<'_>,
    show_progress: bool,
) -> Result<ImportReport, AppError> {
    let repo_slug = opts.repo.slug();
    tracing::info!(repo = %repo_slug, "resolviendo repositorio en HuggingFace");
    let files = client.list_files(opts.repo).await?;
    tracing::info!(files = files.len(), "listados los ficheros del repositorio");

    let selection = quant::select(&repo_slug, &files, opts.quant, opts.mmproj)?;
    tracing::info!(
        quant = %opts.quant,
        gguf = %selection.gguf.path,
        size = %format_bytes(selection.gguf.size),
        "GGUF seleccionado"
    );
    if let Some(mmproj) = selection.mmproj {
        tracing::info!(
            mmproj = %mmproj.path,
            size = %format_bytes(mmproj.size),
            "mmproj seleccionado"
        );
    }

    fs::create_dir_all(opts.dest).await?;

    let mut downloaded = Vec::with_capacity(if selection.mmproj.is_some() { 2 } else { 1 });
    downloaded
        .push(download_one(client, opts.repo, selection.gguf, opts.dest, show_progress).await?);
    if let Some(mmproj) = selection.mmproj {
        downloaded.push(download_one(client, opts.repo, mmproj, opts.dest, show_progress).await?);
    }

    Ok(ImportReport {
        repo: repo_slug,
        quant: opts.quant.to_string(),
        dest: opts.dest.to_path_buf(),
        files: downloaded,
    })
}

/// Sufijo `.part` usado para las descargas en curso.
fn part_path_for(final_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.part", final_path.display()))
}

/// Devuelve el tamaño de `path` si existe.
async fn file_len(path: &Path) -> Option<u64> {
    fs::metadata(path).await.ok().map(|meta| meta.len())
}

/// Resuelve la ruta relativa `rel` dentro de `dest` rechazando cualquier intento
/// de escape: rutas absolutas, componentes `..`, raíces o prefijos.
///
/// Además de la comprobación léxica, canonicaliza `dest` y verifica que el
/// ancestro existente más cercano de la ruta final siga dentro de él (defensa
/// ante enlaces simbólicos).
///
/// # Errors
///
/// Devuelve [`AppError::UnsafePath`] si la ruta escapa del directorio de destino.
fn safe_join(dest: &Path, rel: &str) -> Result<PathBuf, AppError> {
    let unsafe_path = || AppError::UnsafePath {
        path: rel.to_string(),
    };

    let rel_path = Path::new(rel);
    if rel_path.is_absolute()
        || rel_path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(unsafe_path());
    }

    let dest = dest.canonicalize()?;
    let joined = dest.join(rel_path);

    let mut ancestor = joined.parent();
    while let Some(path) = ancestor {
        if let Ok(canonical) = path.canonicalize() {
            if !canonical.starts_with(&dest) {
                return Err(unsafe_path());
            }
            break;
        }
        ancestor = path.parent();
    }

    Ok(joined)
}

/// Abre (crea o anexa) el fichero parcial según el desplazamiento.
async fn open_part(path: &Path, offset: u64) -> Result<File, AppError> {
    let mut options = OpenOptions::new();
    options.create(true).write(true);
    if offset > 0 {
        options.append(true);
    } else {
        options.truncate(true);
    }
    Ok(options.open(path).await?)
}

/// Renombra el `.part` al nombre definitivo (atómico dentro del mismo directorio).
async fn finalize(part_path: &Path, final_path: &Path) -> Result<(), AppError> {
    fs::rename(part_path, final_path).await?;
    Ok(())
}

/// Descarga (o completa) un único fichero, reanudando si procede.
async fn download_one<C: HfClient + Sync>(
    client: &C,
    repo: &RepoId,
    file: &RemoteFile,
    dest: &Path,
    show_progress: bool,
) -> Result<DownloadedFile, AppError> {
    let final_path = safe_join(dest, &file.path)?;
    let part_path = part_path_for(&final_path);

    let final_len = file_len(&final_path).await;
    let part_len = file_len(&part_path).await;
    match resume_plan(final_len, part_len, file.size) {
        ResumeAction::AlreadyComplete => {
            tracing::info!(file = %file.path, size = %format_bytes(file.size), "ya descargado");
            Ok(DownloadedFile {
                path: file.path.clone(),
                size: file.size,
                cached: true,
            })
        }
        ResumeAction::FinalizePartial => {
            finalize(&part_path, &final_path).await?;
            tracing::info!(file = %file.path, size = %format_bytes(file.size), "descarga finalizada");
            Ok(DownloadedFile {
                path: file.path.clone(),
                size: file.size,
                cached: false,
            })
        }
        ResumeAction::Fetch { offset } => {
            if let Some(parent) = final_path.parent() {
                fs::create_dir_all(parent).await?;
            }
            let mut handle = open_part(&part_path, offset).await?;
            let mut reporter = Reporter::new(file.size, offset, show_progress);
            client
                .download_from(
                    repo,
                    &file.path,
                    offset,
                    file.size,
                    &mut handle,
                    &mut reporter,
                )
                .await?;
            handle.flush().await?;
            let written = handle.metadata().await?.len();
            drop(handle);

            // Un stream que termina corto sin error no debe darse por completo:
            // dejamos el `.part` para poder reanudar y devolvemos un error claro.
            if file.size > 0 && written < file.size {
                reporter.finish();
                return Err(AppError::IncompleteDownload {
                    path: file.path.clone(),
                    expected: file.size,
                    written,
                });
            }

            finalize(&part_path, &final_path).await?;
            reporter.finish();
            tracing::info!(
                file = %file.path,
                progress = %crate::format::progress_line(file.size, file.size),
                "descargado"
            );
            Ok(DownloadedFile {
                path: file.path.clone(),
                size: file.size,
                cached: false,
            })
        }
    }
}

/// Reporter de progreso para la descarga de un fichero.
struct Reporter {
    bar: Option<indicatif::ProgressBar>,
}

impl Reporter {
    /// Crea el reporter; dibuja barra solo si `show` y el tamaño es conocido.
    fn new(total: u64, start: u64, show: bool) -> Self {
        let bar = if show && total > 0 {
            let bar = indicatif::ProgressBar::new(total);
            if let Ok(style) = indicatif::ProgressStyle::with_template(
                "{bar:40} {bytes}/{total_bytes} ({percent}%)",
            ) {
                bar.set_style(style);
            }
            bar.set_position(start);
            Some(bar)
        } else {
            None
        };
        Self { bar }
    }

    /// Retira la barra al terminar.
    fn finish(&self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
    }
}

impl ProgressSink for Reporter {
    fn on_bytes(&mut self, downloaded: u64, _total: u64) {
        if let Some(bar) = &self.bar {
            bar.set_position(downloaded);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::future::Future;
    use std::sync::Mutex;

    use tokio::io::AsyncWriteExt as _;

    use super::*;

    #[test]
    fn resume_plan_covers_all_cases() {
        assert_eq!(
            resume_plan(Some(100), None, 100),
            ResumeAction::AlreadyComplete
        );
        assert_eq!(
            resume_plan(Some(100), Some(100), 100),
            ResumeAction::AlreadyComplete
        );
        assert_eq!(
            resume_plan(None, Some(100), 100),
            ResumeAction::FinalizePartial
        );
        assert_eq!(
            resume_plan(None, Some(40), 100),
            ResumeAction::Fetch { offset: 40 }
        );
        assert_eq!(
            resume_plan(None, None, 100),
            ResumeAction::Fetch { offset: 0 }
        );
        assert_eq!(
            resume_plan(Some(50), Some(40), 100),
            ResumeAction::Fetch { offset: 40 }
        );
        assert_eq!(
            resume_plan(None, None, 0),
            ResumeAction::Fetch { offset: 0 }
        );
    }

    /// Cliente falso: sirve contenido fijo por ruta y registra los offsets pedidos.
    struct FakeHf {
        files: Vec<RemoteFile>,
        contents: HashMap<String, Vec<u8>>,
        calls: Mutex<Vec<(String, u64)>>,
        fail_list: bool,
        /// Si es `Some(n)`, escribe como mucho `n` bytes del tramo pendiente
        /// (simula un stream que termina corto sin error).
        limit: Option<usize>,
    }

    impl FakeHf {
        fn new(entries: &[(&str, Vec<u8>)]) -> Self {
            Self::with_limit(entries, None)
        }

        fn with_limit(entries: &[(&str, Vec<u8>)], limit: Option<usize>) -> Self {
            let files = entries
                .iter()
                .map(|(path, content)| RemoteFile {
                    path: (*path).to_string(),
                    size: content.len() as u64,
                })
                .collect();
            let contents = entries
                .iter()
                .map(|(path, content)| ((*path).to_string(), content.clone()))
                .collect();
            Self {
                files,
                contents,
                calls: Mutex::new(Vec::new()),
                fail_list: false,
                limit,
            }
        }

        fn calls(&self) -> Vec<(String, u64)> {
            self.calls
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default()
        }
    }

    impl HfClient for FakeHf {
        fn list_files(
            &self,
            _repo: &RepoId,
        ) -> impl Future<Output = Result<Vec<RemoteFile>, AppError>> + Send {
            let files = self.files.clone();
            let fail = self.fail_list;
            async move {
                if fail {
                    return Err(AppError::NoGgufFound {
                        repo: "fake".to_string(),
                    });
                }
                Ok(files)
            }
        }

        fn download_from(
            &self,
            _repo: &RepoId,
            path: &str,
            offset: u64,
            total: u64,
            file: &mut File,
            progress: &mut dyn ProgressSink,
        ) -> impl Future<Output = Result<(), AppError>> + Send {
            let data = self.contents.get(path).cloned().unwrap_or_default();
            if let Ok(mut calls) = self.calls.lock() {
                calls.push((path.to_string(), offset));
            }
            let start = (offset as usize).min(data.len());
            let mut remaining = data[start..].to_vec();
            if let Some(limit) = self.limit {
                remaining.truncate(limit);
            }
            async move {
                let mut written = offset;
                for chunk in remaining.chunks(7) {
                    file.write_all(chunk).await?;
                    written += chunk.len() as u64;
                    progress.on_bytes(written, total);
                }
                Ok(())
            }
        }
    }

    fn opts<'a>(
        repo: &'a RepoId,
        dest: &'a Path,
        quant: &'a str,
        mmproj: bool,
    ) -> ImportOptions<'a> {
        ImportOptions {
            repo,
            quant,
            mmproj,
            dest,
        }
    }

    #[tokio::test]
    async fn downloads_gguf_and_mmproj() {
        let dir = tempfile::tempdir().expect("tempdir");
        let gguf = b"GGUF-gguf-payload".to_vec();
        let mmproj = b"mmproj-payload".to_vec();
        let client = FakeHf::new(&[
            ("model-Q6_K.gguf", gguf.clone()),
            ("mmproj-model-f16.gguf", mmproj.clone()),
            ("model-Q4_K_M.gguf", b"other".to_vec()),
        ]);
        let repo = RepoId::parse("org/model").expect("repo");
        let options = opts(&repo, dir.path(), "Q6_K", true);

        let report = run(&client, &options, false).await.expect("import ok");

        assert_eq!(report.files.len(), 2);
        assert_eq!(
            std::fs::read(dir.path().join("model-Q6_K.gguf")).expect("leer gguf"),
            gguf
        );
        assert_eq!(
            std::fs::read(dir.path().join("mmproj-model-f16.gguf")).expect("leer mmproj"),
            mmproj
        );
        assert_eq!(
            client.calls(),
            vec![
                ("model-Q6_K.gguf".to_string(), 0),
                ("mmproj-model-f16.gguf".to_string(), 0),
            ]
        );
    }

    #[tokio::test]
    async fn resumes_partial_download() {
        let dir = tempfile::tempdir().expect("tempdir");
        let full: Vec<u8> = (0..40u8).collect();
        std::fs::write(dir.path().join("model-Q6_K.gguf.part"), &full[..16]).expect("part");
        let client = FakeHf::new(&[("model-Q6_K.gguf", full.clone())]);
        let repo = RepoId::parse("org/model").expect("repo");
        let options = opts(&repo, dir.path(), "Q6_K", false);

        run(&client, &options, false).await.expect("import ok");

        assert_eq!(
            client.calls(),
            vec![("model-Q6_K.gguf".to_string(), 16)],
            "debe reanudar desde el byte 16"
        );
        assert_eq!(
            std::fs::read(dir.path().join("model-Q6_K.gguf")).expect("leer"),
            full
        );
        assert!(!dir.path().join("model-Q6_K.gguf.part").exists());
    }

    #[tokio::test]
    async fn skips_already_complete_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let full: Vec<u8> = (0..40u8).collect();
        std::fs::write(dir.path().join("model-Q6_K.gguf"), &full).expect("write");
        let client = FakeHf::new(&[("model-Q6_K.gguf", full)]);
        let repo = RepoId::parse("org/model").expect("repo");
        let options = opts(&repo, dir.path(), "Q6_K", false);

        let report = run(&client, &options, false).await.expect("import ok");

        assert!(client.calls().is_empty(), "no debe volver a descargar");
        assert!(report.files[0].cached);
    }

    #[tokio::test]
    async fn propagates_quant_not_found() {
        let dir = tempfile::tempdir().expect("tempdir");
        let client = FakeHf::new(&[("model-Q4_K_M.gguf", vec![1, 2, 3])]);
        let repo = RepoId::parse("org/model").expect("repo");
        let options = opts(&repo, dir.path(), "Q6_K", false);

        let err = run(&client, &options, false)
            .await
            .expect_err("debe fallar");
        assert!(matches!(err, AppError::QuantNotFound { .. }), "{err}");
    }

    #[tokio::test]
    async fn resumes_partial_download_and_matches_final_hash() {
        use sha2::{Digest as _, Sha256};

        let dir = tempfile::tempdir().expect("tempdir");
        let full: Vec<u8> = (0..64u8).collect();
        std::fs::write(dir.path().join("model-Q6_K.gguf.part"), &full[..20]).expect("part");
        let client = FakeHf::new(&[("model-Q6_K.gguf", full.clone())]);
        let repo = RepoId::parse("org/model").expect("repo");
        let options = opts(&repo, dir.path(), "Q6_K", false);

        run(&client, &options, false).await.expect("import ok");

        assert_eq!(
            client.calls(),
            vec![("model-Q6_K.gguf".to_string(), 20)],
            "debe reanudar desde el byte 20"
        );
        let downloaded = std::fs::read(dir.path().join("model-Q6_K.gguf")).expect("leer");
        assert_eq!(
            downloaded.len(),
            64,
            "el fichero final debe tener todos los bytes"
        );
        assert_eq!(downloaded, full, "el contenido reanudado debe ser idéntico");
        assert_eq!(
            Sha256::digest(&downloaded),
            Sha256::digest(&full),
            "el hash final debe coincidir con el del contenido completo"
        );
        assert!(!dir.path().join("model-Q6_K.gguf.part").exists());
    }

    #[tokio::test]
    async fn short_stream_errors_and_keeps_part_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let full: Vec<u8> = vec![7u8; 40];
        // El stream sólo entrega 10 de los 40 bytes anunciados, sin error.
        let client = FakeHf::with_limit(&[("model-Q6_K.gguf", full)], Some(10));
        let repo = RepoId::parse("org/model").expect("repo");
        let options = opts(&repo, dir.path(), "Q6_K", false);

        let err = run(&client, &options, false)
            .await
            .expect_err("debe fallar por descarga incompleta");

        assert!(matches!(err, AppError::IncompleteDownload { .. }), "{err}");
        let part = dir.path().join("model-Q6_K.gguf.part");
        assert!(part.exists(), "debe conservar el `.part` para reanudar");
        assert!(
            std::fs::metadata(&part).expect("meta").len() < 40,
            "el `.part` debe tener sólo los bytes recibidos"
        );
        assert!(
            !dir.path().join("model-Q6_K.gguf").exists(),
            "no debe finalizar la descarga incompleta"
        );
    }

    #[test]
    fn safe_join_rejects_traversal_and_absolute_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let dest = dir.path();

        for malicious in [
            "../evil",
            "../../etc/passwd",
            "/etc/passwd",
            "a/../../b",
            "sub/../../../outside",
            "..",
        ] {
            let err = safe_join(dest, malicious).expect_err("debe rechazar la ruta");
            assert!(
                matches!(err, AppError::UnsafePath { .. }),
                "`{malicious}` debe ser insegura, pero fue: {err}"
            );
        }
    }

    #[test]
    fn safe_join_accepts_nested_relative_paths() {
        let dir = tempfile::tempdir().expect("tempdir");
        let joined = safe_join(dir.path(), "sub/dir/model.gguf").expect("ruta válida");
        assert!(joined.starts_with(dir.path().canonicalize().expect("canon")));
        assert!(joined.ends_with("sub/dir/model.gguf"));
    }

    #[tokio::test]
    async fn rejects_traversal_path_from_remote_repo() {
        let dir = tempfile::tempdir().expect("tempdir");
        for malicious in ["../evil-Q6_K.gguf", "a/../../evil-Q6_K.gguf"] {
            let client = FakeHf::new(&[(malicious, vec![1, 2, 3])]);
            let repo = RepoId::parse("org/model").expect("repo");
            let options = opts(&repo, dir.path(), "Q6_K", false);

            let err = run(&client, &options, false)
                .await
                .expect_err("debe rechazar la ruta del repo");
            assert!(
                matches!(err, AppError::UnsafePath { .. }),
                "`{malicious}` debe rechazarse, pero fue: {err}"
            );
        }
    }

    #[tokio::test]
    #[ignore = "requiere red; ejecutar con `cargo test -- --ignored`"]
    async fn downloads_real_gguf_from_hub() {
        let dir = tempfile::tempdir().expect("tempdir");
        let client = crate::hf::HfHubClient::new().expect("cliente hf");
        let repo = RepoId::parse("ggml-org/tiny-llamas").expect("repo");
        let options = opts(&repo, dir.path(), "q4_0", false);

        let report = run(&client, &options, false).await.expect("descarga real");
        let path = dir.path().join(&report.files[0].path);
        assert!(path.exists(), "debe existir {}", path.display());
        assert_eq!(std::fs::metadata(&path).expect("meta").len(), 19_077_344);
    }
}
