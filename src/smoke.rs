//! Lógica del subcomando `test`: pruebas de humo de texto, *thinking*, *tools* y visión.
//!
//! La construcción de la petición por modo y la clasificación de la respuesta son
//! lógica pura y se prueban sin red. La ejecución real vive en [`run`], detrás del
//! trait `crate::ollama::OllamaClient`.

use std::path::Path;

use serde::Serialize;

use crate::error::AppError;
use crate::ollama::{ChatMessage, ChatRequest, ChatResponse, OllamaClient, Tool};

/// Imagen PNG mínima (8×8) embebida para la prueba de visión por defecto.
pub const DEFAULT_IMAGE_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAIAAABLbSncAAAAEUlEQVR4nGO4o6GBFTEMLQkAe3tLAfuiUfAAAAAASUVORK5CYII=";

/// Prompt de la prueba de texto.
const TEXT_PROMPT: &str = "Responde en una sola frase: ¿qué es Rust?";
/// Prompt de la prueba de razonamiento.
const THINK_PROMPT: &str = "¿Cuánto es 17 × 23? Razona el resultado paso a paso.";
/// Prompt de la prueba de herramientas.
const TOOLS_PROMPT: &str = "¿Qué tiempo hace en Madrid? Usa la herramienta disponible.";
/// Prompt de la prueba de visión.
const VISION_PROMPT: &str = "Describe en una frase la imagen adjunta.";

/// Modo de prueba de humo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeMode {
    /// Respuesta de texto simple.
    Text,
    /// Respuesta con bloque de razonamiento.
    Thinking,
    /// *Tool-calling*: el modelo debe emitir `tool_calls`.
    Tools,
    /// Visión: se envía una imagen y se espera respuesta.
    Vision,
}

impl SmokeMode {
    /// Nombre canónico del modo.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Thinking => "thinking",
            Self::Tools => "tools",
            Self::Vision => "vision",
        }
    }
}

/// Opciones del subcomando `test`.
#[derive(Debug, Clone)]
pub struct SmokeOptions<'a> {
    /// Modelo a probar.
    pub model: &'a str,
    /// Incluye la prueba de *tool-calling*.
    pub tools: bool,
    /// Incluye la prueba de visión.
    pub vision: bool,
    /// Imagen para la prueba de visión; si es `None` se usa la embebida.
    pub image: Option<&'a Path>,
    /// Límite de tokens generados por prueba.
    pub num_predict: Option<u32>,
}

/// Resultado de una prueba individual.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CheckResult {
    /// Modo probado (`text`, `thinking`, `tools`, `vision`).
    pub mode: String,
    /// `true` si la prueba pasó.
    pub ok: bool,
    /// Detalle legible del resultado.
    pub detail: String,
}

/// Resultado agregado del subcomando `test`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SmokeReport {
    /// Modelo probado.
    pub model: String,
    /// `true` si todas las pruebas pasaron.
    pub passed: bool,
    /// Resultado de cada prueba, en orden.
    pub checks: Vec<CheckResult>,
}

/// Modos a ejecutar según las opciones y las capacidades del modelo.
///
/// `text` siempre; `thinking` solo si el modelo declara esa capacidad (pedir
/// `think: true` a un modelo sin razonamiento devuelve un 400 de Ollama);
/// `tools` y `vision` solo si se piden explícitamente.
#[must_use]
pub fn modes(options: &SmokeOptions<'_>, capabilities: &[String]) -> Vec<SmokeMode> {
    let mut modes = vec![SmokeMode::Text];
    if supports(capabilities, "thinking") {
        modes.push(SmokeMode::Thinking);
    }
    if options.tools {
        modes.push(SmokeMode::Tools);
    }
    if options.vision {
        modes.push(SmokeMode::Vision);
    }
    modes
}

/// `true` si la lista de capacidades contiene `name` (sin distinguir mayúsculas).
fn supports(capabilities: &[String], name: &str) -> bool {
    capabilities
        .iter()
        .any(|cap| cap.eq_ignore_ascii_case(name))
}

/// Construye la petición de `/api/chat` correspondiente a un modo.
#[must_use]
pub fn chat_request(
    model: &str,
    mode: SmokeMode,
    image: Option<&str>,
    num_predict: Option<u32>,
) -> ChatRequest {
    let options = num_predict.map(|limit| serde_json::json!({ "num_predict": limit }));
    let (messages, tools, think) = match mode {
        SmokeMode::Text => (vec![user_message(mode)], Vec::new(), Some(false)),
        SmokeMode::Thinking => (vec![user_message(mode)], Vec::new(), Some(true)),
        SmokeMode::Tools => (vec![user_message(mode)], vec![weather_tool()], Some(false)),
        SmokeMode::Vision => {
            let images = image.map_or_else(Vec::new, |data| vec![data.to_string()]);
            (
                vec![ChatMessage::user_with_images(VISION_PROMPT, images)],
                Vec::new(),
                Some(false),
            )
        }
    };

    ChatRequest {
        model: model.to_string(),
        messages,
        tools,
        stream: false,
        think,
        options,
    }
}

/// Clasifica la respuesta de un modo como prueba superada o fallida.
#[must_use]
pub fn classify(mode: SmokeMode, response: &ChatResponse) -> CheckResult {
    let message = &response.message;
    let content = message.content.trim();
    let thinking = message.thinking.as_deref().unwrap_or("").trim();
    let content_len = content.chars().count();

    let (ok, detail) = match mode {
        SmokeMode::Text => (
            !content.is_empty(),
            format!("{content_len} caracteres de respuesta"),
        ),
        SmokeMode::Thinking => {
            if !thinking.is_empty() {
                (true, "el modelo emitió razonamiento".to_string())
            } else if !content.is_empty() {
                (
                    true,
                    "sin bloque de razonamiento, pero hay respuesta".to_string(),
                )
            } else {
                (false, "ni razonamiento ni respuesta".to_string())
            }
        }
        SmokeMode::Tools => {
            if message.tool_calls.is_empty() {
                (false, "el modelo no emitió `tool_calls`".to_string())
            } else {
                let names = message
                    .tool_calls
                    .iter()
                    .map(|call| call.function.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                (true, format!("tool_calls: {names}"))
            }
        }
        SmokeMode::Vision => (
            !content.is_empty(),
            format!("{content_len} caracteres de respuesta a la imagen"),
        ),
    };

    CheckResult {
        mode: mode.as_str().to_string(),
        ok,
        detail,
    }
}

/// Ejecuta las pruebas de humo seleccionadas contra el modelo.
///
/// # Errors
///
/// Devuelve [`AppError::SourceNotFound`] si la imagen indicada no existe y
/// [`AppError::Ollama`] / [`AppError::OllamaUnavailable`] si el servidor falla.
pub async fn run<C: OllamaClient + Sync>(
    client: &C,
    options: &SmokeOptions<'_>,
) -> Result<SmokeReport, AppError> {
    let image = match options.image {
        Some(path) => Some(encode_image(path)?),
        None => None,
    };

    let info = client.show(options.model).await?;

    let mut checks = Vec::new();
    for mode in modes(options, &info.capabilities) {
        let payload = if mode == SmokeMode::Vision {
            Some(
                image
                    .clone()
                    .unwrap_or_else(|| DEFAULT_IMAGE_BASE64.to_string()),
            )
        } else {
            None
        };
        let request = chat_request(options.model, mode, payload.as_deref(), options.num_predict);
        let response = client.chat(&request).await?;
        checks.push(classify(mode, &response));
    }

    let passed = checks.iter().all(|check| check.ok);
    Ok(SmokeReport {
        model: options.model.to_string(),
        passed,
        checks,
    })
}

/// Lee una imagen y la codifica en base64.
fn encode_image(path: &Path) -> Result<String, AppError> {
    use base64::Engine as _;

    if !path.is_file() {
        return Err(AppError::SourceNotFound {
            path: path.display().to_string(),
        });
    }
    let bytes = std::fs::read(path)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// Herramienta de ejemplo para la prueba de *tool-calling*.
#[must_use]
pub fn weather_tool() -> Tool {
    Tool::function(
        "get_weather",
        "Devuelve el tiempo actual de una ciudad",
        serde_json::json!({
            "type": "object",
            "properties": {
                "city": { "type": "string", "description": "Nombre de la ciudad" }
            },
            "required": ["city"]
        }),
    )
}

/// Mensaje de usuario para un modo (sin imágenes).
fn user_message(mode: SmokeMode) -> ChatMessage {
    let prompt = match mode {
        SmokeMode::Thinking => THINK_PROMPT,
        SmokeMode::Tools => TOOLS_PROMPT,
        SmokeMode::Vision => VISION_PROMPT,
        SmokeMode::Text => TEXT_PROMPT,
    };
    ChatMessage::user(prompt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ollama::{
        ChatMessageResponse, ModelInfo, ToolCall, ToolCallFunction, fake::FakeOllamaClient,
    };

    fn options<'a>(model: &'a str, tools: bool, vision: bool) -> SmokeOptions<'a> {
        SmokeOptions {
            model,
            tools,
            vision,
            image: None,
            num_predict: Some(64),
        }
    }

    fn response(content: &str) -> ChatResponse {
        ChatResponse {
            message: ChatMessageResponse {
                role: "assistant".into(),
                content: content.into(),
                ..ChatMessageResponse::default()
            },
            ..ChatResponse::default()
        }
    }

    /// Capacidades como las devolvería `/api/show`.
    fn caps(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    /// Modelo de prueba con soporte de razonamiento.
    fn thinking_model() -> ModelInfo {
        ModelInfo {
            capabilities: caps(&["completion", "thinking"]),
            parameters: String::new(),
        }
    }

    #[test]
    fn modes_always_include_text_and_thinking() {
        let all = caps(&["completion", "thinking"]);
        assert_eq!(
            modes(&options("m", false, false), &all),
            vec![SmokeMode::Text, SmokeMode::Thinking]
        );
    }

    #[test]
    fn modes_add_tools_and_vision_on_demand() {
        let all = caps(&["completion", "thinking"]);
        assert_eq!(
            modes(&options("m", true, true), &all),
            vec![
                SmokeMode::Text,
                SmokeMode::Thinking,
                SmokeMode::Tools,
                SmokeMode::Vision
            ]
        );
    }

    #[test]
    fn modes_skip_thinking_when_the_model_does_not_support_it() {
        let plain = caps(&["completion", "tools"]);
        assert_eq!(
            modes(&options("m", true, false), &plain),
            vec![SmokeMode::Text, SmokeMode::Tools]
        );
        assert_eq!(
            modes(&options("m", false, false), &[]),
            vec![SmokeMode::Text]
        );
    }

    #[test]
    fn text_request_sends_the_prompt_without_tools() {
        let request = chat_request("m", SmokeMode::Text, None, Some(64));

        assert_eq!(request.model, "m");
        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].role, "user");
        assert!(!request.messages[0].content.is_empty());
        assert!(request.messages[0].images.is_empty());
        assert!(request.tools.is_empty());
        assert_eq!(request.think, Some(false));
        assert!(!request.stream);
        assert_eq!(
            request.options.unwrap()["num_predict"],
            serde_json::json!(64)
        );
    }

    #[test]
    fn thinking_request_enables_the_think_flag() {
        let request = chat_request("m", SmokeMode::Thinking, None, None);
        assert_eq!(request.think, Some(true));
    }

    #[test]
    fn tools_request_declares_a_function_tool() {
        let request = chat_request("m", SmokeMode::Tools, None, None);

        assert_eq!(request.tools.len(), 1);
        assert_eq!(request.tools[0].r#type, "function");
        assert_eq!(request.tools[0].function.name, "get_weather");
        assert!(request.tools[0].function.parameters.is_object());
    }

    #[test]
    fn vision_request_embeds_the_image() {
        let request = chat_request("m", SmokeMode::Vision, Some("BASE64IMG"), None);

        assert_eq!(request.messages.len(), 1);
        assert_eq!(request.messages[0].images, vec!["BASE64IMG".to_string()]);
        assert!(request.tools.is_empty());
    }

    #[test]
    fn classifies_text_answer() {
        let ok = classify(
            SmokeMode::Text,
            &response("Rust es un lenguaje de sistemas."),
        );
        assert!(ok.ok, "{ok:?}");
        assert_eq!(ok.mode, "text");

        let fail = classify(SmokeMode::Text, &response("   "));
        assert!(!fail.ok);
    }

    #[test]
    fn classifies_thinking_answer() {
        let mut with_thinking = response("");
        with_thinking.message.thinking = Some("17 × 23 = 391".into());
        assert!(classify(SmokeMode::Thinking, &with_thinking).ok);

        // Sin razonamiento pero con respuesta, también se acepta.
        assert!(classify(SmokeMode::Thinking, &response("391")).ok);
    }

    #[test]
    fn classifies_tool_calls() {
        let mut tool_response = response("");
        tool_response.message.tool_calls = vec![ToolCall {
            function: ToolCallFunction {
                name: "get_weather".into(),
                arguments: serde_json::json!({ "city": "Madrid" }),
            },
        }];

        let ok = classify(SmokeMode::Tools, &tool_response);
        assert!(ok.ok, "{ok:?}");
        assert!(ok.detail.contains("get_weather"), "{ok:?}");

        assert!(!classify(SmokeMode::Tools, &response("No puedo.")).ok);
    }

    #[test]
    fn classifies_vision_answer() {
        assert!(classify(SmokeMode::Vision, &response("Un cuadro rojo.")).ok);
        assert!(!classify(SmokeMode::Vision, &response("")).ok);
    }

    #[tokio::test]
    async fn run_executes_every_selected_mode() {
        // Respuesta que satisface texto, thinking y tools a la vez.
        let mut full = response("vale");
        full.message.tool_calls = vec![ToolCall {
            function: ToolCallFunction {
                name: "get_weather".into(),
                arguments: serde_json::json!({ "city": "Madrid" }),
            },
        }];
        let client = FakeOllamaClient::new()
            .with_info(thinking_model())
            .with_chat_response(full);
        let opts = options("mi-modelo", true, false);

        let report = run(&client, &opts).await.expect("test ok");

        assert_eq!(report.model, "mi-modelo");
        assert_eq!(report.checks.len(), 3, "text + thinking + tools");
        assert!(report.passed);
        assert_eq!(client.chat_requests().len(), 3);
    }

    #[tokio::test]
    async fn run_skips_thinking_without_capability() {
        let client = FakeOllamaClient::new()
            .with_info(ModelInfo {
                capabilities: caps(&["completion", "tools"]),
                parameters: String::new(),
            })
            .with_chat_response(response("vale"));
        let opts = options("mi-modelo", false, false);

        let report = run(&client, &opts).await.expect("test ok");

        assert_eq!(report.checks.len(), 1, "solo text");
        assert_eq!(report.checks[0].mode, "text");
        assert!(report.passed);
    }

    #[tokio::test]
    async fn run_marks_report_as_failed_when_a_check_fails() {
        // Sin `tool_calls` la prueba de tools falla, aunque el texto responda.
        let client = FakeOllamaClient::new()
            .with_info(thinking_model())
            .with_chat_response(response("vale"));
        let opts = options("mi-modelo", true, false);

        let report = run(&client, &opts).await.expect("la ejecución no falla");

        assert!(!report.passed);
        let tools = report
            .checks
            .iter()
            .find(|check| check.mode == "tools")
            .expect("check de tools");
        assert!(!tools.ok);
    }

    #[tokio::test]
    async fn run_surfaces_ollama_errors() {
        let client = FakeOllamaClient::failing("modelo desconocido");
        let opts = options("mi-modelo", false, false);

        let err = run(&client, &opts)
            .await
            .expect_err("debe propagar el error");
        assert!(matches!(err, AppError::Ollama { .. }), "{err}");
    }
}
