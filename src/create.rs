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
use crate::quant;

/// Opciones del subcomando `create`.
#[derive(Debug, Clone)]
pub struct CreateOptions<'a> {
    /// Nombre del modelo a crear en Ollama.
    pub name: &'a str,
    /// Fichero GGUF o directorio de safetensors de origen (`FROM`).
    pub from: &'a Path,
    /// Proyector de visión (`mmproj`), si lo hay.
    pub mmproj: Option<&'a Path>,
    /// Tamaño de contexto (`num_ctx`).
    pub ctx: Option<u32>,
    /// Renderer explícito (`--renderer`).
    pub renderer: Option<&'a str>,
    /// Parser explícito (`--parser`).
    pub parser: Option<&'a str>,
    /// Cuantización nativa de Ollama (`--quantize`), si la hay.
    pub quantize: Option<&'a str>,
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
    /// Cuantización nativa aplicada, si la hay.
    pub quantize: Option<String>,
    /// Información verificada con `/api/show`.
    pub info: ModelInfo,
}

/// Etapas de progreso que [`run`] notifica al llamante, en orden real de ocurrencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// Preparación del origen: se recoge el snapshot nativo y se suben sus blobs.
    Converting,
    /// Cuantización durante la creación (`--quantize`).
    Quantizing,
    /// Creación del modelo en Ollama (`POST /api/create`).
    Creating,
}

impl Stage {
    /// Etiqueta legible de la etapa, alineada con la spec de progreso.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Stage::Converting => "conversión interna",
            Stage::Quantizing => "cuantización",
            Stage::Creating => "creación",
        }
    }
}

/// Observador de las etapas de [`run`], para reportar progreso sin acoplarse a la UI.
pub trait Progress {
    /// Notifica el inicio de `stage`.
    fn stage(&self, stage: Stage);

    /// Notifica el inicio de `stage` con un `value` asociado (p. ej. el `quantize`).
    fn stage_with_value(&self, stage: Stage, value: &str);
}

/// Implementación de [`Progress`] que ignora las etapas.
///
/// Permite llamar a [`run`] sin observador (`&()`) desde tests y contextos sin UI.
impl Progress for () {
    fn stage(&self, _stage: Stage) {}

    fn stage_with_value(&self, _stage: Stage, _value: &str) {}
}

/// Crea el modelo en Ollama a partir del GGUF (o directorio de safetensors) indicado.
///
/// Reporta el avance por etapas a través de `progress`: primero la preparación del
/// origen (`conversión interna`), después la cuantización (si se pidió) y, por
/// último, la creación del modelo.
///
/// # Errors
///
/// - [`AppError::SourceNotFound`] si el GGUF (o el `mmproj`) no existe.
/// - [`AppError::NoSafetensorsFound`] si `from` es un directorio sin `.safetensors`.
/// - [`AppError::QuantizeNotSafetensors`] si se pide `--quantize` con un origen que no
///   es un directorio de safetensors.
/// - [`AppError::UnsupportedQuantize`] si `--quantize` no es un valor soportado.
/// - [`AppError::OllamaUnavailable`] si Ollama no responde.
/// - [`AppError::Ollama`] si la API de Ollama rechaza la operación.
pub async fn run<C, P>(
    client: &C,
    opts: &CreateOptions<'_>,
    progress: &P,
) -> Result<CreateReport, AppError>
where
    C: OllamaClient + Sync,
    P: Progress,
{
    let from = absolute(opts.from);
    if from.is_dir() {
        ensure_safetensors_dir(&from).await?;
    } else {
        ensure_file(&from)?;
    }
    let mmproj = opts.mmproj.map(absolute);
    if let Some(path) = &mmproj {
        ensure_file(path)?;
    }

    // Se valida la cuantización ANTES de tocar la red: es un error de uso.
    // Ollama solo cuantiza en la creación a partir de safetensors: con un GGUF ya
    // convertido el campo `quantize` se rechaza siempre.
    let quantize = match opts.quantize {
        Some(value) => {
            if !from.is_dir() {
                return Err(AppError::QuantizeNotSafetensors {
                    path: from.display().to_string(),
                });
            }
            match quant::canonical_quantize(value) {
                Some(canonical) => Some(canonical),
                None => {
                    return Err(AppError::UnsupportedQuantize {
                        value: value.to_string(),
                        supported: quant::supported_quantize_values(),
                    });
                }
            }
        }
        None => None,
    };

    // La preparación del origen (Modelfile, blobs) es la etapa más larga antes de
    // crear, pero se anuncia DESPUÉS de confirmar la conexión con Ollama: así no se
    // emite una etapa si el servidor está caído.
    let spec = ModelfileSpec::new(&from)
        .with_mmproj(mmproj.as_deref())
        .with_renderer(opts.renderer)
        .with_parser(opts.parser)
        .with_num_ctx(opts.ctx)
        .with_parameters(default_parameters());
    let modelfile = spec.render();

    let version = client.version().await?;
    tracing::info!(version = %version, model = opts.name, "Ollama disponible");

    progress.stage(Stage::Converting);

    tracing::info!(source = %from.display(), "preparando blobs del modelo");
    let mut blobs = BTreeMap::new();
    for (name, path) in source_files(&from, mmproj.as_deref()).await? {
        let digest = sha256_digest(&path).await?;
        if client.blob_exists(&digest).await? {
            tracing::debug!(file = %name, digest = %digest, "blob ya presente");
        } else {
            tracing::info!(file = %name, digest = %digest, "subiendo blob a Ollama");
            client.push_blob(&digest, &path).await?;
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
        quantize: quantize.clone(),
        stream: false,
    };

    if let Some(value) = &quantize {
        progress.stage_with_value(Stage::Quantizing, value);
        tracing::info!(quantize = %value, model = opts.name, "cuantizando durante la creación");
    }
    progress.stage(Stage::Creating);
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
        quantize,
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

/// Verifica que `dir` contiene al menos un fichero `.safetensors`.
///
/// La lectura del directorio se delega en el pool de bloqueo para no bloquear el
/// reactor de Tokio (véase [`sha256_digest`]).
///
/// # Errors
///
/// - [`AppError::NoSafetensorsFound`] si no hay ningún `.safetensors`.
/// - [`AppError::Io`] si el directorio no se puede leer o la tarea falla.
async fn ensure_safetensors_dir(dir: &Path) -> Result<(), AppError> {
    let display = dir.display().to_string();
    let dir = dir.to_path_buf();
    let has_weights = tokio::task::spawn_blocking(move || {
        let mut found = false;
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && is_safetensors(&path) {
                found = true;
                break;
            }
        }
        Ok::<bool, AppError>(found)
    })
    .await
    .map_err(|err| AppError::Io(std::io::Error::other(err)))??;

    if has_weights {
        Ok(())
    } else {
        Err(AppError::NoSafetensorsFound { path: display })
    }
}

/// Ficheros de origen con el nombre que usarán como blob: `(nombre, ruta)`.
///
/// Si `from` es un directorio (snapshot nativo), se suben los ficheros relevantes
/// de primer nivel ordenados por nombre; si es un fichero, solo ese.
///
/// # Errors
///
/// Propaga [`AppError::Io`] si el directorio no se puede leer.
async fn source_files(
    from: &Path,
    mmproj: Option<&Path>,
) -> Result<Vec<(String, PathBuf)>, AppError> {
    let mut files = if from.is_dir() {
        directory_files(from).await?
    } else {
        vec![(file_name(from), from.to_path_buf())]
    };
    if let Some(path) = mmproj {
        files.push((file_name(path), path.to_path_buf()));
    }
    Ok(files)
}

/// Ficheros relevante de primer nivel de `dir`, ordenados por nombre.
///
/// Solo se incluyen ficheros de peso, metadatos y tokenizer: subir como blob
/// ficheros ajenos al modelo (`README.md`, `.gitattributes`…) hace que Ollama los
/// interprete como parte del snapshot y falle con `errMixedModelTypes`.
///
/// # Errors
///
/// - [`AppError::Io`] si el directorio no se puede leer o la tarea de bloqueo falla.
async fn directory_files(dir: &Path) -> Result<Vec<(String, PathBuf)>, AppError> {
    let dir = dir.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && is_model_file(&path) {
                files.push((file_name(&path), path));
            }
        }
        files.sort();
        Ok::<Vec<(String, PathBuf)>, AppError>(files)
    })
    .await
    .map_err(|err| AppError::Io(std::io::Error::other(err)))?
}

/// Extensiones de fichero relevantes para un snapshot nativo de safetensors.
///
/// Cubre los pesos (`.safetensors`), la configuración (`.json`) y los tokenizers
/// binarios habituales (`tokenizer.model`, `*.tiktoken`). Los `.txt` **no** se
/// filtran por extensión: solo se admiten por **nombre** los tokenizers relevantes
/// (véase [`MODEL_TEXT_FILES`]) para no subir `README.txt`, `requirements.txt` ni
/// ruido similar.
const MODEL_FILE_EXTENSIONS: &[&str] = &[".safetensors", ".json", ".model", ".tiktoken"];

/// Ficheros de tokenizer con extensión `.txt` admitidos explícitamente.
///
/// Se comparan por **nombre completo** (en minúsculas), no por extensión: así solo
/// entran `vocab.txt` y `merges.txt` y se descartan `README.txt`, `requirements.txt`,
/// etc.
const MODEL_TEXT_FILES: &[&str] = &["vocab.txt", "merges.txt"];

/// `true` si `path` es un fichero con extensión o nombre relevante para el import nativo.
fn is_model_file(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    MODEL_FILE_EXTENSIONS
        .iter()
        .any(|extension| name.ends_with(extension))
        || MODEL_TEXT_FILES.iter().any(|allowed| name == *allowed)
}

/// `true` si `path` es un fichero `.safetensors`.
fn is_safetensors(path: &Path) -> bool {
    path.file_name()
        .map(|name| {
            name.to_string_lossy()
                .to_ascii_lowercase()
                .ends_with(".safetensors")
        })
        .unwrap_or(false)
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
        create_error: Option<String>,
    }

    impl FakeOllama {
        fn new(existing: bool, info: ModelInfo) -> Self {
            Self {
                existing,
                pushed: Mutex::new(Vec::new()),
                created: Mutex::new(Vec::new()),
                info,
                create_error: None,
            }
        }

        /// Cliente que falla al crear con el mensaje indicado (simula a Ollama).
        fn failing(message: &str) -> Self {
            Self {
                create_error: Some(message.to_string()),
                ..Self::new(true, ModelInfo::default())
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
            let error = self.create_error.clone();
            async move {
                match error {
                    Some(message) => Err(AppError::Ollama { message }),
                    None => Ok(()),
                }
            }
        }

        async fn pull(&self, _request: &crate::ollama::PullRequest) -> Result<String, AppError> {
            Ok("success".to_string())
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
            quantize: None,
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
            quantize: None,
        };

        let report = run(&client, &opts, &()).await.expect("create ok");

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

        run(&client, &opts, &()).await.expect("create ok");

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
            quantize: None,
        };

        run(&client, &opts, &()).await.expect("create ok");

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

        let err = run(&client, &opts, &()).await.expect_err("debe fallar");
        assert!(matches!(err, AppError::SourceNotFound { .. }), "{err}");
        assert!(client.created().is_empty(), "no debe llamar a create");
    }

    /// Directorio con un `.safetensors` y metadatos, como un snapshot nativo.
    fn safetensors_dir(dir: &Path) -> PathBuf {
        let model = dir.join("model.safetensors");
        std::fs::write(&model, b"SAFETENSORS").expect("escribir pesos");
        std::fs::write(dir.join("config.json"), b"{}").expect("escribir config");
        std::fs::write(dir.join("tokenizer.json"), b"{}").expect("escribir tokenizer");
        dir.to_path_buf()
    }

    #[tokio::test]
    async fn creates_model_from_safetensors_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = safetensors_dir(dir.path());
        let client = FakeOllama::new(true, ModelInfo::default());
        let opts = CreateOptions {
            name: "nativo",
            from: &source,
            mmproj: None,
            ctx: None,
            renderer: None,
            parser: None,
            quantize: None,
        };

        let report = run(&client, &opts, &()).await.expect("create ok");

        assert_eq!(report.model, "nativo");
        assert!(
            report
                .modelfile
                .contains(&format!("FROM {}", source.display())),
            "el Modelfile debe usar el directorio: {}",
            report.modelfile
        );
        let created = client.created();
        assert_eq!(created.len(), 1);
        assert_eq!(
            created[0].files.keys().cloned().collect::<Vec<_>>(),
            vec![
                "config.json".to_string(),
                "model.safetensors".to_string(),
                "tokenizer.json".to_string(),
            ]
        );
        assert_eq!(created[0].quantize, None);
    }

    #[tokio::test]
    async fn passes_quantize_to_create_request() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = safetensors_dir(dir.path());
        let client = FakeOllama::new(true, ModelInfo::default());
        let opts = CreateOptions {
            name: "nativo-quant",
            from: &source,
            mmproj: None,
            ctx: None,
            renderer: None,
            parser: None,
            quantize: Some("NVFP4"),
        };

        let report = run(&client, &opts, &()).await.expect("create ok");

        // El valor se canonicaliza a minúsculas, como lo devuelve Ollama.
        assert_eq!(report.quantize.as_deref(), Some("nvfp4"));
        assert_eq!(
            client.created()[0].quantize.as_deref(),
            Some("nvfp4"),
            "la petición debe llevar el campo `quantize`"
        );
    }

    #[tokio::test]
    async fn rejects_quantize_with_gguf_source() {
        let dir = tempfile::tempdir().expect("tempdir");
        let from = gguf(dir.path(), "model.gguf");
        let client = FakeOllama::new(false, ModelInfo::default());
        let opts = CreateOptions {
            name: "gguf-quant",
            from: &from,
            mmproj: None,
            ctx: None,
            renderer: None,
            parser: None,
            quantize: Some("nvfp4"),
        };

        let err = run(&client, &opts, &()).await.expect_err("debe fallar");
        assert!(
            matches!(err, AppError::QuantizeNotSafetensors { .. }),
            "{err}"
        );
        assert!(client.created().is_empty(), "no debe llamar a create");
    }

    /// Observador de etapas que registra el orden en que se notifican.
    #[derive(Default)]
    struct RecordingProgress {
        stages: Mutex<Vec<(Stage, Option<String>)>>,
    }

    impl RecordingProgress {
        fn recorded(&self) -> Vec<(Stage, Option<String>)> {
            self.stages
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default()
        }
    }

    impl Progress for RecordingProgress {
        fn stage(&self, stage: Stage) {
            if let Ok(mut guard) = self.stages.lock() {
                guard.push((stage, None));
            }
        }

        fn stage_with_value(&self, stage: Stage, value: &str) {
            if let Ok(mut guard) = self.stages.lock() {
                guard.push((stage, Some(value.to_string())));
            }
        }
    }

    #[tokio::test]
    async fn reports_stages_in_order_with_quantize() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = safetensors_dir(dir.path());
        let client = FakeOllama::new(true, ModelInfo::default());
        let progress = RecordingProgress::default();
        let opts = CreateOptions {
            name: "etapas",
            from: &source,
            mmproj: None,
            ctx: None,
            renderer: None,
            parser: None,
            quantize: Some("int4"),
        };

        run(&client, &opts, &progress).await.expect("create ok");

        assert_eq!(
            progress.recorded(),
            vec![
                (Stage::Converting, None),
                (Stage::Quantizing, Some("int4".to_string())),
                (Stage::Creating, None),
            ]
        );
    }

    #[tokio::test]
    async fn reports_stages_in_order_without_quantize() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = safetensors_dir(dir.path());
        let client = FakeOllama::new(true, ModelInfo::default());
        let progress = RecordingProgress::default();
        let opts = options("sin-quant", &source);

        run(&client, &opts, &progress).await.expect("create ok");

        assert_eq!(
            progress.recorded(),
            vec![(Stage::Converting, None), (Stage::Creating, None),]
        );
    }

    #[tokio::test]
    async fn ignores_irrelevant_files_in_safetensors_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = safetensors_dir(dir.path());
        std::fs::write(dir.path().join("README.md"), b"# modelo").expect("escribir README");
        std::fs::write(
            dir.path().join(".gitattributes"),
            b"*.safetensors filter=lfs",
        )
        .expect("escribir gitattributes");
        // `.txt` ajenos al tokenizer: NO deben subirse.
        std::fs::write(dir.path().join("README.txt"), b"# modelo").expect("escribir README.txt");
        std::fs::write(dir.path().join("requirements.txt"), b"torch").expect("escribir reqs");
        // `.txt` de tokenizer: SÍ deben subirse (filtro por nombre, no por extensión).
        std::fs::write(dir.path().join("vocab.txt"), b"tok").expect("escribir vocab");
        std::fs::write(dir.path().join("merges.txt"), b"a b").expect("escribir merges");
        let client = FakeOllama::new(true, ModelInfo::default());
        let opts = options("sin-ruido", &source);

        run(&client, &opts, &()).await.expect("create ok");

        assert_eq!(
            client.created()[0]
                .files
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec![
                "config.json".to_string(),
                "merges.txt".to_string(),
                "model.safetensors".to_string(),
                "tokenizer.json".to_string(),
                "vocab.txt".to_string(),
            ],
            "solo entran los `.txt` de tokenizer (vocab/merges), no README.txt ni requirements.txt"
        );
    }

    #[tokio::test]
    async fn rejects_unsupported_quantize_before_create() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = safetensors_dir(dir.path());
        let client = FakeOllama::new(true, ModelInfo::default());
        let opts = CreateOptions {
            name: "nativo-malo",
            from: &source,
            mmproj: None,
            ctx: None,
            renderer: None,
            parser: None,
            quantize: Some("NO_EXISTE"),
        };

        let err = run(&client, &opts, &()).await.expect_err("debe fallar");
        assert!(matches!(err, AppError::UnsupportedQuantize { .. }), "{err}");
        assert!(err.to_string().contains("nvfp4"), "{err}");
        assert!(client.created().is_empty(), "no debe llamar a create");
    }

    #[tokio::test]
    async fn fails_when_directory_has_no_safetensors() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("config.json"), b"{}").expect("escribir config");
        let client = FakeOllama::new(true, ModelInfo::default());
        let opts = options("sin-pesos", dir.path());

        let err = run(&client, &opts, &()).await.expect_err("debe fallar");
        assert!(matches!(err, AppError::NoSafetensorsFound { .. }), "{err}");
        assert!(client.created().is_empty(), "no debe llamar a create");
    }

    #[tokio::test]
    async fn propagates_ollama_error_on_create() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = safetensors_dir(dir.path());
        let client = FakeOllama::failing("arquitectura no soportada: `llama`");
        let opts = options("no-soportado", &source);

        let err = run(&client, &opts, &())
            .await
            .expect_err("debe propagar el error");
        assert!(matches!(err, AppError::Ollama { .. }), "{err}");
        assert!(
            err.to_string().contains("no soportada"),
            "el error de Ollama debe propagarse tal cual: {err}"
        );
    }
}
