//! Errores tipados de `modelforge`.

use thiserror::Error;

/// Error de aplicación con soporte de conversión a `serde_json` para `--json`.
#[derive(Debug, Error)]
pub enum AppError {
    /// No se indicó el quant a descargar con `--quant`.
    #[error("falta `--quant <QUANT>`: indica el quant a descargar (p. ej. `Q6_K`)")]
    MissingQuant,

    /// El repo no tiene el formato `owner/name`.
    #[error("repo inválido `{input}`: se espera `owner/model`")]
    InvalidRepo {
        /// Valor introducido por el usuario.
        input: String,
    },

    /// El repositorio no contiene el quant solicitado.
    #[error(
        "el repo `{repo}` no contiene el quant `{quant}`{}",
        available_hint(.available)
    )]
    QuantNotFound {
        /// Repositorio consultado (`owner/name`).
        repo: String,
        /// Quant solicitado.
        quant: String,
        /// Quants disponibles en el repo.
        available: Vec<String>,
    },

    /// El repositorio no contiene ningún fichero GGUF.
    #[error("el repo `{repo}` no contiene ningún fichero GGUF")]
    NoGgufFound {
        /// Repositorio consultado (`owner/name`).
        repo: String,
    },

    /// Se pidió `--mmproj` pero el repo no tiene proyector de visión.
    #[error("el repo `{repo}` no contiene ningún mmproj (proyección de visión)")]
    MmprojNotFound {
        /// Repositorio consultado (`owner/name`).
        repo: String,
    },

    /// Una ruta del repositorio remoto intenta escapar del directorio de destino.
    #[error("ruta insegura `{path}`: queda fuera del directorio de destino")]
    UnsafePath {
        /// Ruta ofrecida por el repositorio remoto.
        path: String,
    },

    /// La descarga terminó antes de recibir todos los bytes anunciados.
    #[error(
        "descarga incompleta de `{path}`: se esperaban {expected} bytes y llegaron {written}; \
         se conserva el fichero parcial para reanudar"
    )]
    IncompleteDownload {
        /// Ruta relativa del fichero dentro del repositorio.
        path: String,
        /// Bytes anunciados por el Hub.
        expected: u64,
        /// Bytes realmente escritos.
        written: u64,
    },

    /// El nombre de la ruta del gateway está vacío o solo tiene espacios.
    #[error(
        "el nombre de la ruta `{name}` no es válido: no puede estar vacío ni ser solo espacios"
    )]
    InvalidRouteName {
        /// Valor introducido por el usuario.
        name: String,
    },

    /// No se indicó el GGUF de origen con `--from`.
    #[error("falta `--from <GGUF>`: indica el fichero GGUF de origen del modelo")]
    MissingFrom,

    /// No se indicó el nombre del modelo a crear con `--name`.
    #[error("falta `--name <MODELO>`: indica el nombre del modelo a crear en Ollama")]
    MissingName,

    /// El directorio de safetensors no contiene ningún `.safetensors`.
    #[error(
        "el directorio `{path}` no contiene ningún fichero `.safetensors`: \
         indica un snapshot nativo (pesos + `config.json` + tokenizer)"
    )]
    NoSafetensorsFound {
        /// Ruta del directorio indicado.
        path: String,
    },

    /// El valor de `--quantize` no es una cuantización soportada (por Ollama o, con
    /// `--backend llama.cpp`, por `llama-quantize`).
    #[error(
        "cuantización `{value}` no soportada; valores válidos: {}",
        supported_quantize(.supported)
    )]
    UnsupportedQuantize {
        /// Valor introducido por el usuario.
        value: String,
        /// Valores soportados por el flujo en curso (nativos de Ollama o de llama.cpp).
        supported: Vec<String>,
    },

    /// Se pidió `--quantize` con un origen que no es un directorio de safetensors.
    #[error(
        "`--quantize` solo es válido para un directorio de safetensors; `{path}` no lo es. \
         Ollama rechaza cuantizar en la creación un GGUF ya convertido: \
         cuantízalo antes con las herramientas de llama.cpp"
    )]
    QuantizeNotSafetensors {
        /// Ruta del origen indicado.
        path: String,
    },

    /// El import nativo recibió flags que solo aplican al import de repos GGUF.
    #[error(
        "el import nativo de safetensors no acepta {}: \
         se descarga un snapshot local, no un repo del Hub",
        flags.join(", ")
    )]
    NativeImportUnsupportedFlags {
        /// Flags recibidos que no aplican (p. ej. `--quant`).
        flags: Vec<String>,
    },

    /// El import de un repo GGUF recibió flags que solo aplican al import nativo.
    #[error(
        "el import de un repo GGUF no acepta {}: `--name` y `--quantize` solo valen con un \
         directorio local de safetensors (para un GGUF ya convertido usa `--quant`)",
        flags.join(", ")
    )]
    GgufImportUnsupportedFlags {
        /// Flags recibidos que no aplican (p. ej. `--quantize`).
        flags: Vec<String>,
    },

    /// La referencia del hub no tiene el formato `hf.co/<repo>:<quant>`.
    #[error(
        "referencia inválida `{input}`: se espera `hf.co/<repo>:<quant>` (p. ej. `hf.co/usuario/modelo:Q4_K_M`)"
    )]
    InvalidHubRef {
        /// Valor introducido por el usuario.
        input: String,
    },

    /// No se encontró una herramienta de llama.cpp en `PATH`.
    #[error(
        "no se encontró `{tool}` en `PATH`: instala llama.cpp (o `llama.cpp` desde \
         https://github.com/ggml-org/llama.cpp) y Python 3, y asegúrate de que `{tool}` \
         está en el `PATH`; no se instala nada automáticamente"
    )]
    ConverterToolMissing {
        /// Nombre de la herramienta ausente (p. ej. `convert_hf_to_gguf.py`).
        tool: String,
    },

    /// Una herramienta externa de llama.cpp terminó con error.
    #[error("`{tool}` falló: {message}")]
    ConverterFailed {
        /// Herramienta que falló (p. ej. `llama-quantize`).
        tool: String,
        /// Detalle del fallo: código de salida, `stderr` y, si lo hay, `stdout`
        /// (ambos recogidos por `check_status`).
        message: String,
    },

    /// Se pidió `--backend llama.cpp` con una fuente que no es un directorio de safetensors.
    #[error(
        "`--backend llama.cpp` solo aplica a un directorio local de safetensors; `{input}` no lo es \
         (para un repo GGUF usa `--quant <QUANT>`, p. ej. `--quant Q4_K_M`)"
    )]
    LlamaCppUnsupportedSource {
        /// Fuente introducida por el usuario.
        input: String,
    },

    /// La fuente es una referencia del hub y debe usarse con `pull`.
    #[error("`{input}` es una referencia del hub; usa `modelforge pull {input}`")]
    UsePull {
        /// Referencia introducida por el usuario.
        input: String,
    },

    /// No se indicó el modelo de la ruta con `--model`.
    #[error("falta `--model <MODEL>`: indica el modelo de la ruta `{name}` del gateway")]
    MissingModel {
        /// Nombre de la ruta a añadir.
        name: String,
    },

    /// El `gateway.yaml` existente no es un YAML válido.
    #[error("el `gateway.yaml` en `{path}` no es válido: {message}")]
    InvalidGateway {
        /// Ruta del fichero de gateway.
        path: String,
        /// Mensaje del error de parseo.
        message: String,
    },

    /// No se pudo determinar el directorio home para la ruta por defecto del gateway.
    #[error("no se pudo determinar el directorio `$HOME`; usa `--config <PATH>`")]
    NoHomeDirectory,

    /// El fichero GGUF (o `mmproj`) indicado no existe.
    #[error("no existe el fichero `{path}`: comprueba la ruta indicada")]
    SourceNotFound {
        /// Ruta solicitada.
        path: String,
    },

    /// Ollama no responde en la URL configurada.
    #[error("Ollama no responde en `{url}`: {source}")]
    OllamaUnavailable {
        /// URL base de la API de Ollama.
        url: String,
        /// Error subyacente de transporte.
        source: reqwest::Error,
    },

    /// La API de Ollama devolvió un error.
    #[error("Ollama: {message}")]
    Ollama {
        /// Mensaje devuelto por Ollama.
        message: String,
    },

    /// Error devuelto por el cliente de HuggingFace.
    #[error("HuggingFace: {0}")]
    Hf(#[from] hf_hub::HFError),

    /// Error de entrada/salida local.
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),

    /// Error al serializar la salida JSON.
    #[error("error al serializar JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Sufijo legible con los quants disponibles, para el mensaje de [`AppError::QuantNotFound`].
fn available_hint(available: &[String]) -> String {
    if available.is_empty() {
        String::new()
    } else {
        format!("; disponibles: {}", available.join(", "))
    }
}

/// Lista legible de cuantizaciones soportadas, para [`AppError::UnsupportedQuantize`].
fn supported_quantize(supported: &[String]) -> String {
    supported.join(", ")
}

impl AppError {
    /// Representación estable del error para la salida `--json`.
    #[must_use]
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "ok": false,
            "error": self.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_renders_message() {
        let err = AppError::MissingModel {
            name: "mi-ruta".into(),
        };
        assert_eq!(
            err.to_string(),
            "falta `--model <MODEL>`: indica el modelo de la ruta `mi-ruta` del gateway"
        );
    }

    #[test]
    fn missing_model_renders_json_envelope() {
        let err = AppError::MissingModel {
            name: "mi-ruta".into(),
        };
        let value = err.to_json();
        assert_eq!(value["ok"], serde_json::json!(false));
        assert!(
            value["error"]
                .as_str()
                .is_some_and(|s| s.contains("mi-ruta"))
        );
    }

    #[test]
    fn quant_not_found_lists_available() {
        let err = AppError::QuantNotFound {
            repo: "org/model".into(),
            quant: "Q6_K".into(),
            available: vec!["Q4_K_M".into(), "Q5_K_M".into()],
        };
        let msg = err.to_string();
        assert!(msg.contains("Q6_K"), "{msg}");
        assert!(msg.contains("Q4_K_M, Q5_K_M"), "{msg}");
    }

    #[test]
    fn unsupported_quantize_lists_valid_values() {
        let err = AppError::UnsupportedQuantize {
            value: "NO_EXISTE".into(),
            supported: vec!["Q4_K_M".into(), "Q6_K".into()],
        };
        let msg = err.to_string();
        assert!(msg.contains("NO_EXISTE"), "{msg}");
        assert!(msg.contains("Q4_K_M, Q6_K"), "{msg}");
    }

    #[test]
    fn quantize_not_safetensors_states_reason() {
        let err = AppError::QuantizeNotSafetensors {
            path: "/tmp/model.gguf".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("/tmp/model.gguf"), "{msg}");
        assert!(msg.contains("safetensors"), "{msg}");
    }

    #[test]
    fn native_import_unsupported_flags_lists_them() {
        let err = AppError::NativeImportUnsupportedFlags {
            flags: vec!["--quant".into(), "--dest".into()],
        };
        let msg = err.to_string();
        assert!(msg.contains("--quant, --dest"), "{msg}");
    }

    #[test]
    fn gguf_import_unsupported_flags_lists_them() {
        let err = AppError::GgufImportUnsupportedFlags {
            flags: vec!["--quantize".into(), "--name".into()],
        };
        let msg = err.to_string();
        assert!(msg.contains("--quantize, --name"), "{msg}");
        assert!(msg.contains("no acepta"), "{msg}");
    }

    #[test]
    fn invalid_hub_ref_states_expected_format() {
        let err = AppError::InvalidHubRef {
            input: "usuario/repo:Q4_K_M".into(),
        };
        assert!(err.to_string().contains("hf.co/<repo>:<quant>"));
    }

    #[test]
    fn converter_tool_missing_is_actionable() {
        let err = AppError::ConverterToolMissing {
            tool: "llama-quantize".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("llama-quantize"), "{msg}");
        assert!(msg.contains("PATH"), "{msg}");
        assert!(msg.contains("llama.cpp"), "{msg}");
    }

    #[test]
    fn converter_failed_reports_tool_and_detail() {
        let err = AppError::ConverterFailed {
            tool: "convert_hf_to_gguf.py".into(),
            message: "código 1: boom".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("convert_hf_to_gguf.py"), "{msg}");
        assert!(msg.contains("boom"), "{msg}");
    }

    #[test]
    fn llamacpp_unsupported_source_explains_scope() {
        let err = AppError::LlamaCppUnsupportedSource {
            input: "org/model".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("llama.cpp"), "{msg}");
        assert!(msg.contains("safetensors"), "{msg}");
    }
}
