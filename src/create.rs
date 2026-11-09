//! Lógica del subcomando `create`: render del Modelfile y creación en Ollama.
//!
//! Toda la interacción con Ollama llega a través del trait `crate::ollama::OllamaClient`,
//! de modo que la orquestación se prueba sin servidor con un doble de prueba.
//!
//! Flujo: se renderiza el Modelfile (para el reporte y la reproducibilidad), se
//! asegura cada GGUF como *blob* en Ollama (subiéndolo si falta), se crea el modelo
//! con `/api/create` y se verifica con `/api/show`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::AppError;
use crate::modelfile::{ModelfileSpec, default_parameters};
use crate::ollama::{CreateModelRequest, ModelInfo, OllamaClient, Sha256Digest};

/// Opciones del subcomando `create`.
#[derive(Debug, Clone)]
pub struct CreateOptions<'a> {
    /// Nombre del modelo a crear en Ollama.
    pub name: &'a str,
    /// Fichero GGUF de origen (`FROM`).
    pub from: &'a Path,
    /// Proyector de visión (`mmproj`), si lo hay.
    pub mmproj: Option<&'a Path>,
    /// Tamaño de contexto (`num_ctx`).
    pub ctx: Option<u32>,
    /// Renderer explícito (`--renderer`).
    pub renderer: Option<&'a str>,
    /// Parser explícito (`--parser`).
    pub parser: Option<&'a str>,
}

/// Resultado de una creación, serializable para `--json`.
#[derive(Debug, Clone, Serialize)]
pub struct CreateReport {
    /// Nombre del modelo creado.
    pub model: String,
    /// Modelfile generado (formato Ollama).
    pub modelfile: String,
    /// Digests de los blobs usados (nombre de fichero → digest).
    pub blobs: BTreeMap<String, String>,
    /// Información verificada con `/api/show`.
    pub info: ModelInfo,
}

/// Crea el modelo en Ollama a partir del GGUF indicado.
///
/// # Errors
///
/// - [`AppError::SourceNotFound`] si el GGUF (o el `mmproj`) no existe.
/// - [`AppError::OllamaUnavailable`] si Ollama no responde.
/// - [`AppError::Ollama`] si la API de Ollama rechaza la operación.
pub async fn run<C: OllamaClient + Sync>(
    client: &C,
    opts: &CreateOptions<'_>,
) -> Result<CreateReport, AppError> {
    let from = absolute(opts.from);
    ensure_file(&from)?;
    let mmproj = opts.mmproj.map(absolute);
    if let Some(path) = &mmproj {
        ensure_file(path)?;
    }

    let spec = ModelfileSpec::new(&from)
        .with_mmproj(mmproj.as_deref())
        .with_renderer(opts.renderer)
        .with_parser(opts.parser)
        .with_num_ctx(opts.ctx)
        .with_parameters(default_parameters());
    let modelfile = spec.render();

    let version = client.version().await?;
    tracing::info!(version = %version, model = opts.name, "Ollama disponible");

    let mut blobs = BTreeMap::new();
    for (name, path) in source_files(&from, mmproj.as_deref()) {
        let digest = sha256_digest(path).await?;
        if client.blob_exists(&digest).await? {
            tracing::debug!(file = %name, digest = %digest, "blob ya presente");
        } else {
            tracing::info!(file = %name, digest = %digest, "subiendo blob a Ollama");
            client.push_blob(&digest, path).await?;
        }
        blobs.insert(name, digest.to_string());
    }

    let mut parameters = BTreeMap::new();
    for parameter in &spec.parameters {
        parameters.insert(parameter.key.clone(), parameter.json_value());
    }
    parameters.insert(
        "num_ctx".to_string(),
        serde_json::Value::from(spec.effective_num_ctx()),
    );

    let request = CreateModelRequest {
        model: opts.name.to_string(),
        files: blobs.clone(),
        renderer: spec.effective_renderer().to_string(),
        parser: spec.effective_parser().to_string(),
        parameters,
        stream: false,
    };

    tracing::info!(model = opts.name, "creando modelo en Ollama");
    client.create(&request).await?;

    let info = client.show(opts.name).await?;
    tracing::info!(
        model = opts.name,
        vision = info.has_vision(),
        num_ctx = ?info.num_ctx(),
        "modelo creado y verificado"
    );

    Ok(CreateReport {
        model: opts.name.to_string(),
        modelfile,
        blobs,
        info,
    })
}

/// Calcula el digest SHA-256 de `path` sin bloquear el reactor de Tokio.
///
/// El hash lee el fichero entero (puede ser un GGUF de varios GiB), así que se
/// delega en el pool de bloqueo con [`tokio::task::spawn_blocking`].
///
/// # Errors
///
/// Propaga [`AppError::Io`] si el fichero no se puede leer o si la tarea de
/// bloqueo falla.
async fn sha256_digest(path: &Path) -> Result<Sha256Digest, AppError> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || Sha256Digest::from_file(&path))
        .await
        .map_err(|err| AppError::Io(std::io::Error::other(err)))?
}

/// Verifica que la ruta existe y es un fichero.
fn ensure_file(path: &Path) -> Result<(), AppError> {
    if path.is_file() {
        Ok(())
    } else {
        Err(AppError::SourceNotFound {
            path: path.display().to_string(),
        })
    }
}

/// Ficheros de origen con el nombre que usarán como blob: `(nombre, ruta)`.
fn source_files<'a>(from: &'a Path, mmproj: Option<&'a Path>) -> Vec<(String, &'a Path)> {
    let mut files = vec![(file_name(from), from)];
    if let Some(path) = mmproj {
        files.push((file_name(path), path));
    }
    files
}

/// Nombre base de un fichero (o su ruta completa si no tiene).
fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Convierte una ruta en absoluta (relativa al directorio actual si hace falta).
///
/// Ollama resuelve las rutas relativas del Modelfile respecto a su propio
/// directorio, así que se absolutizan para que apunten al fichero correcto.
fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    match std::env::current_dir() {
        Ok(cwd) => cwd.join(path),
        Err(_) => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Cliente falso: registra las llamadas y devuelve una información fija.
    struct FakeOllama {
        existing: bool,
        pushed: Mutex<Vec<(String, PathBuf)>>,
        created: Mutex<Vec<CreateModelRequest>>,
        info: ModelInfo,
    }

    impl FakeOllama {
        fn new(existing: bool, info: ModelInfo) -> Self {
            Self {
                existing,
                pushed: Mutex::new(Vec::new()),
                created: Mutex::new(Vec::new()),
                info,
            }
        }

        fn pushed(&self) -> Vec<(String, PathBuf)> {
            self.pushed
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default()
        }

        fn created(&self) -> Vec<CreateModelRequest> {
            self.created
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default()
        }
    }

    impl OllamaClient for FakeOllama {
        async fn version(&self) -> Result<String, AppError> {
            Ok("1.2.3".to_string())
        }

        fn blob_exists(
            &self,
            _digest: &Sha256Digest,
        ) -> impl Future<Output = Result<bool, AppError>> + Send {
            let exists = self.existing;
            async move { Ok(exists) }
        }

        fn push_blob(
            &self,
            digest: &Sha256Digest,
            path: &Path,
        ) -> impl Future<Output = Result<(), AppError>> + Send {
            if let Ok(mut guard) = self.pushed.lock() {
                guard.push((digest.to_string(), path.to_path_buf()));
            }
            async { Ok(()) }
        }

        fn create(
            &self,
            request: &CreateModelRequest,
        ) -> impl Future<Output = Result<(), AppError>> + Send {
            if let Ok(mut guard) = self.created.lock() {
                guard.push(request.clone());
            }
            async { Ok(()) }
        }

        fn show(&self, _model: &str) -> impl Future<Output = Result<ModelInfo, AppError>> + Send {
            let info = self.info.clone();
            async move { Ok(info) }
        }

        async fn chat(
            &self,
            _request: &crate::ollama::ChatRequest,
        ) -> Result<crate::ollama::ChatResponse, AppError> {
            Ok(crate::ollama::ChatResponse::default())
        }

        async fn generate(
            &self,
            _request: &crate::ollama::GenerateRequest,
        ) -> Result<crate::ollama::GenerateResponse, AppError> {
            Ok(crate::ollama::GenerateResponse::default())
        }

        async fn running(&self) -> Result<Vec<crate::ollama::RunningModel>, AppError> {
            Ok(Vec::new())
        }

        async fn tags(&self) -> Result<Vec<crate::ollama::TaggedModel>, AppError> {
            Ok(Vec::new())
        }
    }

    fn vision_info() -> ModelInfo {
        ModelInfo {
            capabilities: vec!["completion".into(), "vision".into()],
            parameters: "num_ctx                        262144\n".into(),
        }
    }

    fn gguf(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, b"GGUF").expect("escribir gguf ficticio");
        path
    }

    fn options<'a>(name: &'a str, from: &'a Path) -> CreateOptions<'a> {
        CreateOptions {
            name,
            from,
            mmproj: None,
            ctx: None,
            renderer: None,
            parser: None,
        }
    }

    #[tokio::test]
    async fn uploads_blobs_and_creates_model_with_vision() {
        let dir = tempfile::tempdir().expect("tempdir");
        let from = gguf(dir.path(), "model-Q6_K.gguf");
        let mmproj = gguf(dir.path(), "mmproj-f16.gguf");
        let client = FakeOllama::new(false, vision_info());
        let opts = CreateOptions {
            name: "test-vision",
            from: &from,
            mmproj: Some(&mmproj),
            ctx: Some(262_144),
            renderer: None,
            parser: None,
        };

        let report = run(&client, &opts).await.expect("create ok");

        assert_eq!(report.model, "test-vision");
        assert!(report.info.has_vision());
        assert_eq!(report.info.num_ctx(), Some(262_144));

        // Se suben los dos blobs, con sus digests correctos.
        let pushed = client.pushed();
        assert_eq!(pushed.len(), 2, "debe subir GGUF y mmproj");
        let from_digest = Sha256Digest::from_file(&from).expect("digest");
        assert_eq!(pushed[0].0, from_digest.to_string());
        assert_eq!(pushed[0].1, from);
        assert_eq!(pushed[1].1, mmproj);

        // La petición a /api/create refleja el Modelfile y los parámetros.
        let created = client.created();
        assert_eq!(created.len(), 1);
        let request = &created[0];
        assert_eq!(request.model, "test-vision");
        assert_eq!(request.renderer, "qwen3.5");
        assert_eq!(request.parser, "qwen3.5");
        assert_eq!(
            request.files.keys().cloned().collect::<Vec<_>>(),
            vec!["mmproj-f16.gguf".to_string(), "model-Q6_K.gguf".to_string()]
        );
        assert_eq!(request.parameters["num_ctx"], serde_json::json!(262_144));
        assert_eq!(request.parameters["temperature"], serde_json::json!(1));

        // El Modelfile del reporte contiene las dos líneas FROM.
        assert!(
            report
                .modelfile
                .contains(&format!("FROM {}", from.display()))
        );
        assert!(
            report
                .modelfile
                .contains(&format!("FROM {}", mmproj.display()))
        );
        assert!(report.modelfile.contains("PARAMETER num_ctx 262144"));
    }

    #[tokio::test]
    async fn skips_blobs_that_already_exist() {
        let dir = tempfile::tempdir().expect("tempdir");
        let from = gguf(dir.path(), "model.gguf");
        let client = FakeOllama::new(true, ModelInfo::default());
        let opts = options("test-cached", &from);

        run(&client, &opts).await.expect("create ok");

        assert!(
            client.pushed().is_empty(),
            "no debe resubir blobs existentes"
        );
        assert_eq!(client.created().len(), 1);
    }

    #[tokio::test]
    async fn honors_renderer_parser_and_ctx_overrides() {
        let dir = tempfile::tempdir().expect("tempdir");
        let from = gguf(dir.path(), "model.gguf");
        let client = FakeOllama::new(true, ModelInfo::default());
        let opts = CreateOptions {
            name: "test-overrides",
            from: &from,
            mmproj: None,
            ctx: Some(8192),
            renderer: Some("deepseek"),
            parser: Some("deepseek"),
        };

        run(&client, &opts).await.expect("create ok");

        let created = client.created();
        let request = &created[0];
        assert_eq!(request.renderer, "deepseek");
        assert_eq!(request.parser, "deepseek");
        assert_eq!(request.parameters["num_ctx"], serde_json::json!(8192));
    }

    #[tokio::test]
    async fn fails_when_source_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("nope.gguf");
        let client = FakeOllama::new(false, ModelInfo::default());
        let opts = options("test-missing", &missing);

        let err = run(&client, &opts).await.expect_err("debe fallar");
        assert!(matches!(err, AppError::SourceNotFound { .. }), "{err}");
        assert!(client.created().is_empty(), "no debe llamar a create");
    }
}
