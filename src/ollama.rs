//! Cliente de la API HTTP de Ollama.
//!
//! Toda la red queda aislada tras el trait [`OllamaClient`], de modo que la lógica
//! de creación (`crate::create`) se prueba con un doble de prueba sin servidor real.
//!
//! La implementación real, [`OllamaHttpClient`], sigue el flujo de la API moderna
//! de Ollama para crear un modelo a partir de GGUF locales:
//!
//! 1. se sube cada fichero con `POST /api/blobs/:digest` (si no existe ya);
//! 2. se llama a `POST /api/create` con el mapa `files` (nombre → digest), el
//!    `renderer`, el `parser` y los `parameters`;
//! 3. se verifica el resultado con `POST /api/show`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::future::Future;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::error::AppError;

/// Digest SHA-256 de un blob, en el formato `sha256:<hex>` que usa Ollama.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    /// Calcula el digest de un fichero leyéndolo por bloques.
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::Io`] si el fichero no se puede leer.
    pub fn from_file(path: &Path) -> Result<Self, AppError> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 256 * 1024];
        loop {
            let read = std::io::Read::read(&mut file, &mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }

        let mut hex = String::with_capacity(64);
        for byte in hasher.finalize() {
            let _ = write!(hex, "{byte:02x}");
        }
        Ok(Self(format!("sha256:{hex}")))
    }

    /// Digest como cadena (`sha256:<hex>`).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Sha256Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Información de un modelo devuelta por `/api/show`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ModelInfo {
    /// Capacidades detectadas por Ollama (`completion`, `vision`, `tools`…).
    pub capabilities: Vec<String>,
    /// Bloque de parámetros tal cual lo devuelve Ollama.
    pub parameters: String,
}

impl ModelInfo {
    /// `true` si Ollama reconoce la capacidad `vision`.
    #[must_use]
    pub fn has_vision(&self) -> bool {
        self.capabilities
            .iter()
            .any(|cap| cap.eq_ignore_ascii_case("vision"))
    }

    /// Valor de un parámetro del bloque `parameters` (p. ej. `num_ctx`).
    #[must_use]
    pub fn parameter(&self, key: &str) -> Option<&str> {
        self.parameters.lines().find_map(|line| {
            let mut parts = line.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(k), Some(v)) if k == key => Some(v),
                _ => None,
            }
        })
    }

    /// `num_ctx` configurado, si está presente.
    #[must_use]
    pub fn num_ctx(&self) -> Option<u32> {
        self.parameter("num_ctx")
            .and_then(|value| value.parse().ok())
    }
}

/// Petición de creación de un modelo (`POST /api/create`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CreateModelRequest {
    /// Nombre del modelo a crear.
    pub model: String,
    /// Mapa de nombre de fichero → digest del blob subido.
    pub files: BTreeMap<String, String>,
    /// Renderer del modelo.
    pub renderer: String,
    /// Parser del modelo.
    pub parser: String,
    /// Parámetros del modelo (valores tipados).
    pub parameters: BTreeMap<String, serde_json::Value>,
    /// Cuantización nativa de Ollama a aplicar (p. ej. `nvfp4`); opcional.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantize: Option<String>,
    /// Desactiva el *streaming* de la respuesta.
    pub stream: bool,
}

/// Petición de descarga de un modelo del hub (`POST /api/pull`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PullRequest {
    /// Nombre del modelo tal cual lo entiende Ollama (`hf.co/<repo>:<quant>`).
    pub name: String,
    /// Desactiva el *streaming* de la respuesta.
    pub stream: bool,
}

/// Cliente de Ollama, abstracto para permitir dobles de prueba sin servidor real.
pub trait OllamaClient {
    /// Comprueba conectividad devolviendo la versión del servidor.
    fn version(&self) -> impl Future<Output = Result<String, AppError>> + Send;

    /// `true` si el blob con ese digest ya existe en el servidor.
    fn blob_exists(
        &self,
        digest: &Sha256Digest,
    ) -> impl Future<Output = Result<bool, AppError>> + Send;

    /// Sube el fichero `path` como blob con el digest indicado.
    fn push_blob(
        &self,
        digest: &Sha256Digest,
        path: &Path,
    ) -> impl Future<Output = Result<(), AppError>> + Send;

    /// Crea (o actualiza) el modelo descrito por la petición.
    fn create(
        &self,
        request: &CreateModelRequest,
    ) -> impl Future<Output = Result<(), AppError>> + Send;

    /// Descarga un modelo del hub vía `/api/pull` y devuelve el estado reportado.
    fn pull(&self, request: &PullRequest) -> impl Future<Output = Result<String, AppError>> + Send;

    /// Devuelve la información de `model` vía `/api/show`.
    fn show(&self, model: &str) -> impl Future<Output = Result<ModelInfo, AppError>> + Send;

    /// Envía una conversación a `/api/chat` y devuelve la respuesta completa.
    fn chat(
        &self,
        request: &ChatRequest,
    ) -> impl Future<Output = Result<ChatResponse, AppError>> + Send;

    /// Envía un *prompt* a `/api/generate` y devuelve la respuesta completa.
    fn generate(
        &self,
        request: &GenerateRequest,
    ) -> impl Future<Output = Result<GenerateResponse, AppError>> + Send;

    /// Lista los modelos actualmente cargados en memoria vía `/api/ps`.
    fn running(&self) -> impl Future<Output = Result<Vec<RunningModel>, AppError>> + Send;

    /// Lista los modelos instalados en el servidor vía `/api/tags`.
    fn tags(&self) -> impl Future<Output = Result<Vec<TaggedModel>, AppError>> + Send;
}

/// Mensaje de una conversación (`/api/chat`).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatMessage {
    /// Rol del mensaje (`user`, `assistant`, `system`…).
    pub role: String,
    /// Texto del mensaje.
    pub content: String,
    /// Imágenes en base64 asociadas al mensaje (solo visión).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
}

impl ChatMessage {
    /// Mensaje de usuario sin imágenes.
    #[must_use]
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            images: Vec::new(),
        }
    }

    /// Mensaje de usuario con imágenes en base64.
    #[must_use]
    pub fn user_with_images(content: impl Into<String>, images: Vec<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            images,
        }
    }
}

/// Herramienta (*tool*) expuesta al modelo en `/api/chat`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tool {
    /// Tipo de herramienta; siempre `function` para Ollama.
    pub r#type: String,
    /// Función descrita por la herramienta.
    pub function: ToolFunction,
}

/// Descripción de una función expuesta como herramienta.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolFunction {
    /// Nombre de la función.
    pub name: String,
    /// Descripción para el modelo.
    pub description: String,
    /// Esquema JSON de los parámetros.
    pub parameters: serde_json::Value,
}

impl Tool {
    /// Crea una herramienta de tipo `function`.
    #[must_use]
    pub fn function(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: serde_json::Value,
    ) -> Self {
        Self {
            r#type: "function".to_string(),
            function: ToolFunction {
                name: name.into(),
                description: description.into(),
                parameters,
            },
        }
    }
}

/// Petición a `/api/chat`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ChatRequest {
    /// Modelo a usar.
    pub model: String,
    /// Conversación enviada.
    pub messages: Vec<ChatMessage>,
    /// Herramientas disponibles.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<Tool>,
    /// Desactiva el *streaming*.
    pub stream: bool,
    /// Modo *thinking* (`true`/`false`); `None` deja el valor por defecto.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think: Option<bool>,
    /// Opciones de muestreo (valores tipados).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
}

/// Llamada a herramienta emitida por el modelo.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolCall {
    /// Función invocada.
    pub function: ToolCallFunction,
}

/// Función invocada dentro de una llamada a herramienta.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ToolCallFunction {
    /// Nombre de la función.
    pub name: String,
    /// Argumentos, en formato JSON.
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Mensaje de la respuesta de `/api/chat`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ChatMessageResponse {
    /// Rol (`assistant`).
    #[serde(default)]
    pub role: String,
    /// Texto de la respuesta.
    #[serde(default)]
    pub content: String,
    /// Bloque de razonamiento, si el modelo lo emite.
    #[serde(default)]
    pub thinking: Option<String>,
    /// Herramientas invocadas por el modelo.
    #[serde(default)]
    pub tool_calls: Vec<ToolCall>,
}

/// Respuesta de `/api/chat`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ChatResponse {
    /// Mensaje del asistente.
    #[serde(default)]
    pub message: ChatMessageResponse,
    /// Tokens generados.
    #[serde(default)]
    pub eval_count: u64,
    /// Duración de la generación en nanosegundos.
    #[serde(default)]
    pub eval_duration: u64,
    /// Duración de la carga del modelo en nanosegundos.
    #[serde(default)]
    pub load_duration: u64,
    /// Duración total en nanosegundos.
    #[serde(default)]
    pub total_duration: u64,
    /// Tokens del *prompt* evaluados.
    #[serde(default)]
    pub prompt_eval_count: u64,
}

/// Petición a `/api/generate`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GenerateRequest {
    /// Modelo a usar.
    pub model: String,
    /// *Prompt* enviado.
    pub prompt: String,
    /// Desactiva el *streaming*.
    pub stream: bool,
    /// Modo *thinking* (`true`/`false`); `None` deja el valor por defecto.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think: Option<bool>,
    /// Opciones de muestreo (valores tipados).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<serde_json::Value>,
}

/// Respuesta de `/api/generate`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct GenerateResponse {
    /// Texto generado.
    #[serde(default)]
    pub response: String,
    /// Tokens generados (`eval_count`).
    #[serde(default)]
    pub eval_count: u64,
    /// Duración de la generación en nanosegundos (`eval_duration`).
    #[serde(default)]
    pub eval_duration: u64,
    /// Duración de la carga del modelo en nanosegundos (`load_duration`).
    #[serde(default)]
    pub load_duration: u64,
    /// Duración total en nanosegundos.
    #[serde(default)]
    pub total_duration: u64,
    /// Tokens del *prompt* evaluados.
    #[serde(default)]
    pub prompt_eval_count: u64,
}

/// Modelo cargado en memoria, según `/api/ps`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct RunningModel {
    /// Nombre del modelo.
    #[serde(default)]
    pub name: String,
    /// Nombre canónico del modelo.
    #[serde(default)]
    pub model: String,
    /// Tamaño total en bytes.
    #[serde(default)]
    pub size: u64,
    /// Bytes residentes en VRAM.
    #[serde(default)]
    pub size_vram: u64,
    /// Longitud de contexto cargada.
    #[serde(default)]
    pub context_length: u64,
}

/// Modelo instalado en el servidor, según `/api/tags`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct TaggedModel {
    /// Nombre del modelo tal cual lo reporta Ollama (p. ej. `qwen3:8b`).
    #[serde(default)]
    pub name: String,
    /// Nombre canónico del modelo.
    #[serde(default)]
    pub model: String,
    /// Tamaño total del modelo en bytes.
    #[serde(default)]
    pub size: u64,
    /// Digest del manifiesto (`sha256:…`).
    #[serde(default)]
    pub digest: String,
    /// Fecha de última modificación (RFC 3339).
    #[serde(default)]
    pub modified_at: String,
}

impl TaggedModel {
    /// Nombre canónico del modelo, prefiriendo `model` sobre `name`.
    #[must_use]
    pub fn canonical_name(&self) -> &str {
        if self.model.is_empty() {
            &self.name
        } else {
            &self.model
        }
    }
}

/// Implementación real de [`OllamaClient`] sobre la API HTTP de Ollama.
pub struct OllamaHttpClient {
    base: String,
    http: reqwest::Client,
}

impl OllamaHttpClient {
    /// Crea el cliente para una URL base (p. ej. `http://localhost:11434`).
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::Ollama`] si no se puede construir el cliente HTTP.
    pub fn new(base: impl Into<String>) -> Result<Self, AppError> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(3600))
            .build()
            .map_err(|err| AppError::Ollama {
                message: err.to_string(),
            })?;
        Ok(Self {
            base: base.into().trim_end_matches('/').to_string(),
            http,
        })
    }

    /// Construye la URL absoluta de un endpoint.
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// Envuelve un error de transporte como Ollama no disponible.
    fn unavailable(&self, err: reqwest::Error) -> AppError {
        AppError::OllamaUnavailable {
            url: self.base.clone(),
            source: err,
        }
    }
}

/// Respuesta de `/api/version`.
#[derive(Debug, Deserialize)]
struct VersionResponse {
    version: String,
}

/// Respuesta de `/api/create`.
#[derive(Debug, Deserialize)]
struct CreateResponse {
    #[serde(default)]
    error: Option<String>,
}

/// Respuesta de `/api/pull` (una línea de estado o el objeto final).
#[derive(Debug, Deserialize)]
struct PullResponse {
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// Cuerpo de la petición a `/api/show`.
#[derive(Debug, Serialize)]
struct ShowRequest {
    model: String,
}

/// Respuesta de `/api/show`.
#[derive(Debug, Deserialize)]
struct ShowResponse {
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    parameters: String,
}

/// Respuesta de `/api/ps`.
#[derive(Debug, Deserialize)]
struct PsResponse {
    #[serde(default)]
    models: Vec<RunningModel>,
}

/// Respuesta de `/api/tags`.
#[derive(Debug, Deserialize)]
struct TagsResponse {
    #[serde(default)]
    models: Vec<TaggedModel>,
}

/// Stream de lectura por bloques de un fichero, para subir blobs sin cargarlos en RAM.
fn file_stream(
    path: PathBuf,
) -> impl futures_util::Stream<Item = Result<Vec<u8>, std::io::Error>> + Send {
    futures_util::stream::try_unfold(None::<tokio::fs::File>, move |state| {
        let path = path.clone();
        async move {
            use tokio::io::AsyncReadExt as _;

            let mut file = match state {
                Some(file) => file,
                None => tokio::fs::File::open(&path).await?,
            };
            let mut buffer = vec![0u8; 256 * 1024];
            let read = file.read(&mut buffer).await?;
            if read == 0 {
                return Ok(None);
            }
            buffer.truncate(read);
            Ok(Some((buffer, Some(file))))
        }
    })
}

impl OllamaClient for OllamaHttpClient {
    async fn version(&self) -> Result<String, AppError> {
        let response = self
            .http
            .get(self.url("/api/version"))
            .send()
            .await
            .map_err(|err| self.unavailable(err))?;

        if !response.status().is_success() {
            return Err(AppError::Ollama {
                message: format!("GET /api/version devolvió {}", response.status()),
            });
        }

        let body: VersionResponse = response.json().await.map_err(|err| AppError::Ollama {
            message: err.to_string(),
        })?;
        Ok(body.version)
    }

    fn blob_exists(
        &self,
        digest: &Sha256Digest,
    ) -> impl Future<Output = Result<bool, AppError>> + Send {
        let url = self.url(&format!("/api/blobs/{}", digest.as_str()));
        async move {
            let response = self
                .http
                .head(url)
                .send()
                .await
                .map_err(|err| self.unavailable(err))?;

            let status = response.status();
            if status.is_success() {
                Ok(true)
            } else if status == reqwest::StatusCode::NOT_FOUND {
                Ok(false)
            } else {
                Err(AppError::Ollama {
                    message: format!("HEAD /api/blobs devolvió {status}"),
                })
            }
        }
    }

    fn push_blob(
        &self,
        digest: &Sha256Digest,
        path: &Path,
    ) -> impl Future<Output = Result<(), AppError>> + Send {
        let url = self.url(&format!("/api/blobs/{}", digest.as_str()));
        let path = path.to_path_buf();
        async move {
            let body = reqwest::Body::wrap_stream(file_stream(path));
            let response = self
                .http
                .post(url)
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .body(body)
                .send()
                .await
                .map_err(|err| self.unavailable(err))?;

            if !response.status().is_success() {
                return Err(AppError::Ollama {
                    message: format!("POST /api/blobs devolvió {}", response.status()),
                });
            }
            Ok(())
        }
    }

    fn create(
        &self,
        request: &CreateModelRequest,
    ) -> impl Future<Output = Result<(), AppError>> + Send {
        let url = self.url("/api/create");
        let request = request.clone();
        async move {
            let response = self
                .http
                .post(url)
                .json(&request)
                .send()
                .await
                .map_err(|err| self.unavailable(err))?;

            let status = response.status();
            let body = response.text().await.map_err(|err| AppError::Ollama {
                message: err.to_string(),
            })?;

            // En respuestas de error se incluye el cuerpo de Ollama: suele traer el
            // motivo real (p. ej. `unsupported quantize type "…"`).
            if !status.is_success() {
                return Err(AppError::Ollama {
                    message: format!("POST /api/create devolvió {status}: {body}"),
                });
            }

            // Con `stream: false` Ollama devuelve un objeto final, pero algunos
            // servidores emiten varias líneas JSON; se acepta la última no vacía.
            let line = body
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("");
            let parsed: CreateResponse =
                serde_json::from_str(line).map_err(|err| AppError::Ollama {
                    message: err.to_string(),
                })?;
            if let Some(error) = parsed.error {
                return Err(AppError::Ollama { message: error });
            }
            Ok(())
        }
    }

    fn pull(&self, request: &PullRequest) -> impl Future<Output = Result<String, AppError>> + Send {
        let url = self.url("/api/pull");
        let request = request.clone();
        async move {
            let response = self
                .http
                .post(url)
                .json(&request)
                .send()
                .await
                .map_err(|err| self.unavailable(err))?;

            let status = response.status();
            let body = response.text().await.map_err(|err| AppError::Ollama {
                message: err.to_string(),
            })?;

            if !status.is_success() {
                return Err(AppError::Ollama {
                    message: format!("POST /api/pull devolvió {status}: {body}"),
                });
            }

            // Con `stream: false` Ollama devuelve un objeto final, pero algunos
            // servidores emiten líneas de estado; se acepta la última línea JSON.
            let line = body
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("");
            let parsed: PullResponse =
                serde_json::from_str(line).map_err(|err| AppError::Ollama {
                    message: err.to_string(),
                })?;
            if let Some(error) = parsed.error {
                return Err(AppError::Ollama { message: error });
            }
            Ok(parsed.status.unwrap_or_else(|| "ok".to_string()))
        }
    }

    fn show(&self, model: &str) -> impl Future<Output = Result<ModelInfo, AppError>> + Send {
        let url = self.url("/api/show");
        let request = ShowRequest {
            model: model.to_string(),
        };
        async move {
            let response = self
                .http
                .post(url)
                .json(&request)
                .send()
                .await
                .map_err(|err| self.unavailable(err))?;

            if !response.status().is_success() {
                return Err(AppError::Ollama {
                    message: format!("POST /api/show devolvió {}", response.status()),
                });
            }

            let body: ShowResponse = response.json().await.map_err(|err| AppError::Ollama {
                message: err.to_string(),
            })?;
            Ok(ModelInfo {
                capabilities: body.capabilities,
                parameters: body.parameters,
            })
        }
    }

    fn chat(
        &self,
        request: &ChatRequest,
    ) -> impl Future<Output = Result<ChatResponse, AppError>> + Send {
        let url = self.url("/api/chat");
        let request = request.clone();
        async move {
            let response = self
                .http
                .post(url)
                .json(&request)
                .send()
                .await
                .map_err(|err| self.unavailable(err))?;

            let status = response.status();
            let body = response.text().await.map_err(|err| AppError::Ollama {
                message: err.to_string(),
            })?;

            if !status.is_success() {
                return Err(AppError::Ollama {
                    message: format!("POST /api/chat devolvió {status}: {body}"),
                });
            }
            serde_json::from_str(&body).map_err(|err| AppError::Ollama {
                message: err.to_string(),
            })
        }
    }

    fn generate(
        &self,
        request: &GenerateRequest,
    ) -> impl Future<Output = Result<GenerateResponse, AppError>> + Send {
        let url = self.url("/api/generate");
        let request = request.clone();
        async move {
            let response = self
                .http
                .post(url)
                .json(&request)
                .send()
                .await
                .map_err(|err| self.unavailable(err))?;

            let status = response.status();
            let body = response.text().await.map_err(|err| AppError::Ollama {
                message: err.to_string(),
            })?;

            if !status.is_success() {
                return Err(AppError::Ollama {
                    message: format!("POST /api/generate devolvió {status}: {body}"),
                });
            }
            serde_json::from_str(&body).map_err(|err| AppError::Ollama {
                message: err.to_string(),
            })
        }
    }

    fn running(&self) -> impl Future<Output = Result<Vec<RunningModel>, AppError>> + Send {
        let url = self.url("/api/ps");
        async move {
            let response = self
                .http
                .get(url)
                .send()
                .await
                .map_err(|err| self.unavailable(err))?;

            if !response.status().is_success() {
                return Err(AppError::Ollama {
                    message: format!("GET /api/ps devolvió {}", response.status()),
                });
            }

            let body: PsResponse = response.json().await.map_err(|err| AppError::Ollama {
                message: err.to_string(),
            })?;
            Ok(body.models)
        }
    }

    fn tags(&self) -> impl Future<Output = Result<Vec<TaggedModel>, AppError>> + Send {
        let url = self.url("/api/tags");
        async move {
            let response = self
                .http
                .get(url)
                .send()
                .await
                .map_err(|err| self.unavailable(err))?;

            if !response.status().is_success() {
                return Err(AppError::Ollama {
                    message: format!("GET /api/tags devolvió {}", response.status()),
                });
            }

            let body: TagsResponse = response.json().await.map_err(|err| AppError::Ollama {
                message: err.to_string(),
            })?;
            Ok(body.models)
        }
    }
}

/// Doble de prueba de [`OllamaClient`] para los tests sin red.
///
/// Guarda respuestas preparadas y registra las peticiones de `chat` y
/// `generate`, de modo que la lógica de `smoke` y `bench` se prueba sin servidor.
#[cfg(test)]
pub mod fake {
    use std::sync::Mutex;

    use super::{
        ChatRequest, ChatResponse, CreateModelRequest, GenerateRequest, GenerateResponse,
        ModelInfo, OllamaClient, PullRequest, RunningModel, Sha256Digest, TaggedModel,
    };
    use crate::error::AppError;

    /// Cliente falso con respuestas configurables y registro de peticiones.
    pub struct FakeOllamaClient {
        version: String,
        error: Option<String>,
        chat_response: ChatResponse,
        generate_response: GenerateResponse,
        running: Vec<RunningModel>,
        info: ModelInfo,
        tags: Vec<TaggedModel>,
        pull_status: String,
        chat_requests: Mutex<Vec<ChatRequest>>,
        generate_requests: Mutex<Vec<GenerateRequest>>,
        pull_requests: Mutex<Vec<PullRequest>>,
    }

    impl Default for FakeOllamaClient {
        fn default() -> Self {
            Self::new()
        }
    }

    impl FakeOllamaClient {
        /// Crea un cliente falso con valores por defecto razonables.
        #[must_use]
        pub fn new() -> Self {
            Self {
                version: "test".to_string(),
                error: None,
                chat_response: ChatResponse::default(),
                generate_response: GenerateResponse::default(),
                running: Vec::new(),
                info: ModelInfo::default(),
                tags: Vec::new(),
                pull_status: "success".to_string(),
                chat_requests: Mutex::new(Vec::new()),
                generate_requests: Mutex::new(Vec::new()),
                pull_requests: Mutex::new(Vec::new()),
            }
        }

        /// Fija el estado devuelto por `/api/pull`.
        #[must_use]
        pub fn with_pull_status(mut self, status: &str) -> Self {
            self.pull_status = status.to_string();
            self
        }

        /// Peticiones de `pull` registradas, en orden.
        #[must_use]
        pub fn pull_requests(&self) -> Vec<PullRequest> {
            self.pull_requests
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default()
        }

        /// Fija la respuesta de `/api/chat`.
        #[must_use]
        pub fn with_chat_response(mut self, response: ChatResponse) -> Self {
            self.chat_response = response;
            self
        }

        /// Fija la respuesta de `/api/generate`.
        #[must_use]
        pub fn with_generate_response(mut self, response: GenerateResponse) -> Self {
            self.generate_response = response;
            self
        }

        /// Fija los modelos devueltos por `/api/ps`.
        #[must_use]
        pub fn with_running(mut self, running: Vec<RunningModel>) -> Self {
            self.running = running;
            self
        }

        /// Fija los modelos devueltos por `/api/tags`.
        #[must_use]
        pub fn with_tags(mut self, tags: Vec<TaggedModel>) -> Self {
            self.tags = tags;
            self
        }

        /// Fija la información devuelta por `/api/show`.
        #[must_use]
        pub fn with_info(mut self, info: ModelInfo) -> Self {
            self.info = info;
            self
        }

        /// Crea un cliente falso que falla con el mensaje indicado.
        #[must_use]
        pub fn failing(message: &str) -> Self {
            Self {
                error: Some(message.to_string()),
                ..Self::new()
            }
        }

        /// Peticiones de `chat` registradas, en orden.
        #[must_use]
        pub fn chat_requests(&self) -> Vec<ChatRequest> {
            self.chat_requests
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default()
        }

        /// Peticiones de `generate` registradas, en orden.
        #[must_use]
        pub fn generate_requests(&self) -> Vec<GenerateRequest> {
            self.generate_requests
                .lock()
                .map(|guard| guard.clone())
                .unwrap_or_default()
        }

        /// Error configurado, si lo hay.
        fn check(&self) -> Result<(), AppError> {
            match &self.error {
                Some(message) => Err(AppError::Ollama {
                    message: message.clone(),
                }),
                None => Ok(()),
            }
        }
    }

    impl OllamaClient for FakeOllamaClient {
        async fn version(&self) -> Result<String, AppError> {
            self.check()?;
            Ok(self.version.clone())
        }

        async fn blob_exists(&self, _digest: &Sha256Digest) -> Result<bool, AppError> {
            self.check()?;
            Ok(true)
        }

        async fn push_blob(
            &self,
            _digest: &Sha256Digest,
            _path: &std::path::Path,
        ) -> Result<(), AppError> {
            self.check()
        }

        async fn create(&self, _request: &CreateModelRequest) -> Result<(), AppError> {
            self.check()
        }

        async fn pull(&self, request: &PullRequest) -> Result<String, AppError> {
            self.check()?;
            if let Ok(mut guard) = self.pull_requests.lock() {
                guard.push(request.clone());
            }
            Ok(self.pull_status.clone())
        }

        async fn show(&self, _model: &str) -> Result<ModelInfo, AppError> {
            self.check()?;
            Ok(self.info.clone())
        }

        async fn chat(&self, request: &ChatRequest) -> Result<ChatResponse, AppError> {
            self.check()?;
            if let Ok(mut guard) = self.chat_requests.lock() {
                guard.push(request.clone());
            }
            Ok(self.chat_response.clone())
        }

        async fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, AppError> {
            self.check()?;
            if let Ok(mut guard) = self.generate_requests.lock() {
                guard.push(request.clone());
            }
            Ok(self.generate_response.clone())
        }

        async fn running(&self) -> Result<Vec<RunningModel>, AppError> {
            self.check()?;
            Ok(self.running.clone())
        }

        async fn tags(&self) -> Result<Vec<TaggedModel>, AppError> {
            self.check()?;
            Ok(self.tags.clone())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_sha256_digest_of_a_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("blob");
        std::fs::write(&path, b"abc").expect("escribir");

        let digest = Sha256Digest::from_file(&path).expect("digest");
        assert_eq!(
            digest.as_str(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn parses_capabilities_parameters_and_num_ctx() {
        let info = ModelInfo {
            capabilities: vec!["completion".into(), "vision".into(), "tools".into()],
            parameters: "top_k                          20\nnum_ctx                        262144\ntemperature                    1\n"
                .into(),
        };

        assert!(info.has_vision());
        assert_eq!(info.parameter("top_k"), Some("20"));
        assert_eq!(info.num_ctx(), Some(262_144));
    }

    #[test]
    fn reports_absence_of_vision_and_num_ctx() {
        let info = ModelInfo {
            capabilities: vec!["completion".into()],
            parameters: "temperature                    1\n".into(),
        };

        assert!(!info.has_vision());
        assert_eq!(info.num_ctx(), None);
    }

    #[tokio::test]
    async fn version_fails_when_ollama_is_down() {
        let client = OllamaHttpClient::new("http://127.0.0.1:1").expect("cliente");
        let err = client.version().await.expect_err("debe fallar");
        assert!(matches!(err, AppError::OllamaUnavailable { .. }), "{err}");
    }

    /// Levanta un servidor HTTP mínimo que responde siempre con `status` y `body`.
    ///
    /// Devuelve la URL base a la que apuntar el cliente.
    fn spawn_http_mock(status: u16, reason: &str, body: &str) -> String {
        use std::io::{Read as _, Write as _};
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
        let addr = listener.local_addr().expect("local_addr");
        let response = format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 8192];
                let _ = stream.read(&mut buffer);
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn create_propagates_the_ollama_error_body() {
        let ollama_message = "unsupported quantize type \"Q4_K_M\": supported types are int4, int8, nvfp4, mxfp4, mxfp8";
        let body = serde_json::json!({ "error": ollama_message }).to_string();
        let base = spawn_http_mock(400, "Bad Request", &body);
        let client = OllamaHttpClient::new(&base).expect("cliente");

        let request = CreateModelRequest {
            model: "probe".to_string(),
            files: BTreeMap::new(),
            renderer: "qwen3.5".to_string(),
            parser: "qwen3.5".to_string(),
            parameters: BTreeMap::new(),
            quantize: Some("mxfp4".to_string()),
            stream: false,
        };

        let err = client
            .create(&request)
            .await
            .expect_err("debe propagar el error de Ollama");
        assert!(matches!(err, AppError::Ollama { .. }), "{err}");
        let message = err.to_string();
        assert!(
            message.contains("unsupported quantize type"),
            "el mensaje de Ollama debe propagarse: {message}"
        );
        assert!(message.contains("400"), "debe incluir el estado: {message}");
    }
}
