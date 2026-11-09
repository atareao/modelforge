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
}
