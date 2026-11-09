//! Detección del tipo de fuente de un *import* y validación de referencias del hub.
//!
//! Lógica pura y sin red: a partir del argumento posicional decide si se trata de
//! un repo de HuggingFace con GGUF, de un directorio local con `.safetensors` o de
//! una referencia del hub de Ollama (`hf.co/<repo>:<quant>`).

use std::path::{Path, PathBuf};

use crate::error::AppError;

/// Prefijo de las referencias del hub de HuggingFace que entiende Ollama.
const HUB_PREFIX: &str = "hf.co/";

/// Tipo de fuente que determina el flujo de `import`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceKind {
    /// Repo de HuggingFace (`owner/model`) con ficheros GGUF.
    GgufRepo(String),
    /// Directorio local con ficheros `.safetensors`.
    SafetensorsDir(PathBuf),
    /// Referencia del hub de Ollama (`hf.co/<repo>:<quant>`).
    HubRef(String),
}

impl SourceKind {
    /// Clasifica el argumento posicional de `import`.
    ///
    /// El orden importa: primero las referencias del hub (nunca son rutas
    /// locales), después los directorios existentes y, por último, los repos.
    #[must_use]
    pub fn detect(input: &str) -> Self {
        if input.starts_with(HUB_PREFIX) {
            return Self::HubRef(input.to_string());
        }
        let path = Path::new(input);
        if path.is_dir() {
            return Self::SafetensorsDir(path.to_path_buf());
        }
        Self::GgufRepo(input.to_string())
    }
}

/// Referencia validada del hub de HuggingFace para `pull`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubRef {
    raw: String,
}

impl HubRef {
    /// Valida y normaliza una referencia `hf.co/<repo>:<quant>`.
    ///
    /// `<repo>` debe ser `owner/name` y `<quant>` no puede estar vacío.
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::InvalidHubRef`] si la referencia no tiene el formato
    /// esperado.
    pub fn parse(input: &str) -> Result<Self, AppError> {
        let invalid = || AppError::InvalidHubRef {
            input: input.to_string(),
        };

        let rest = input.strip_prefix(HUB_PREFIX).ok_or_else(invalid)?;
        let (repo, quant) = rest.split_once(':').ok_or_else(invalid)?;
        let (owner, name) = repo.split_once('/').ok_or_else(invalid)?;

        if owner.is_empty()
            || name.is_empty()
            || name.contains('/')
            || quant.is_empty()
            || quant.contains(':')
            || quant.contains('/')
        {
            return Err(invalid());
        }

        Ok(Self {
            raw: input.to_string(),
        })
    }

    /// Referencia completa, tal cual la entiende Ollama (`hf.co/<repo>:<quant>`).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_gguf_repo() {
        assert_eq!(
            SourceKind::detect("ggml-org/tiny-llamas"),
            SourceKind::GgufRepo("ggml-org/tiny-llamas".to_string())
        );
    }

    #[test]
    fn detects_hub_reference() {
        assert_eq!(
            SourceKind::detect("hf.co/usuario/repo:Q4_K_M"),
            SourceKind::HubRef("hf.co/usuario/repo:Q4_K_M".to_string())
        );
    }

    #[test]
    fn detects_existing_directory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let input = dir.path().to_str().expect("utf-8");
        assert_eq!(
            SourceKind::detect(input),
            SourceKind::SafetensorsDir(dir.path().to_path_buf())
        );
    }

    #[test]
    fn missing_directory_is_treated_as_repo() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("no-existe");
        assert_eq!(
            SourceKind::detect(missing.to_str().expect("utf-8")),
            SourceKind::GgufRepo(missing.display().to_string())
        );
    }

    #[test]
    fn parses_valid_hub_reference() {
        let hub = HubRef::parse("hf.co/usuario/repo:Q4_K_M").expect("válida");
        assert_eq!(hub.name(), "hf.co/usuario/repo:Q4_K_M");
    }

    #[test]
    fn rejects_invalid_hub_references() {
        for bad in [
            "usuario/repo:Q4_K_M",
            "hf.co/usuario/repo",
            "hf.co/usuario:Q4_K_M",
            "hf.co/usuario/repo:",
            "hf.co/:Q4_K_M",
            "hf.co/usuario/repo/sub:Q4_K_M",
            "hf.co/usuario/repo:Q4:K_M",
            "hf.co/usuario/repo:q/x",
            "hf.co/usuario/repo:Q4_K_M/extra",
        ] {
            let err = HubRef::parse(bad).expect_err("debe rechazarse");
            assert!(
                matches!(err, AppError::InvalidHubRef { .. }),
                "{bad}: {err}"
            );
        }
    }
}
