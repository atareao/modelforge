//! Lógica del subcomando `pull`: atajo remoto para GGUF del hub de HuggingFace.
//!
//! La descarga se delega por completo en Ollama (`/api/pull`), que ya sabe
//! resolver y registrar los GGUF publicados en el hub a partir de una referencia
//! `hf.co/<repo>:<quant>`. La red queda tras el trait `crate::ollama::OllamaClient`.

use serde::Serialize;

use crate::error::AppError;
use crate::ollama::{OllamaClient, PullRequest};
use crate::source::HubRef;

/// Opciones del subcomando `pull`.
#[derive(Debug, Clone)]
pub struct PullOptions<'a> {
    /// Referencia `hf.co/<repo>:<quant>`.
    pub reference: &'a str,
}

/// Resultado de un `pull`, serializable para `--json`.
#[derive(Debug, Clone, Serialize)]
pub struct PullReport {
    /// Referencia descargada.
    pub reference: String,
    /// Estado final reportado por Ollama (p. ej. `success`).
    pub status: String,
}

/// Descarga el GGUF del hub delegando en Ollama.
///
/// # Errors
///
/// - [`AppError::InvalidHubRef`] si la referencia no tiene el formato esperado.
/// - [`AppError::OllamaUnavailable`] si Ollama no responde.
/// - [`AppError::Ollama`] si la API de Ollama rechaza la descarga.
pub async fn run<C: OllamaClient + Sync>(
    client: &C,
    opts: &PullOptions<'_>,
) -> Result<PullReport, AppError> {
    let hub = HubRef::parse(opts.reference)?;
    let request = PullRequest {
        name: hub.name().to_string(),
        stream: false,
    };

    tracing::info!(
        reference = opts.reference,
        "solicitando la descarga a Ollama"
    );
    let status = client.pull(&request).await?;

    Ok(PullReport {
        reference: opts.reference.to_string(),
        status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ollama::fake::FakeOllamaClient;

    #[tokio::test]
    async fn delegates_download_to_ollama() {
        let client = FakeOllamaClient::new();
        let opts = PullOptions {
            reference: "hf.co/usuario/repo:Q4_K_M",
        };

        let report = run(&client, &opts).await.expect("pull ok");

        assert_eq!(report.reference, "hf.co/usuario/repo:Q4_K_M");
        assert_eq!(report.status, "success");
        let requests = client.pull_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].name, "hf.co/usuario/repo:Q4_K_M");
        assert!(!requests[0].stream);
    }

    #[tokio::test]
    async fn reports_status_from_ollama() {
        let client = FakeOllamaClient::new().with_pull_status("pulling manifest");
        let opts = PullOptions {
            reference: "hf.co/usuario/repo:Q4_K_M",
        };

        let report = run(&client, &opts).await.expect("pull ok");

        assert_eq!(report.status, "pulling manifest");
    }

    #[tokio::test]
    async fn rejects_invalid_reference_without_calling_ollama() {
        let client = FakeOllamaClient::new();
        let opts = PullOptions {
            reference: "usuario/repo:Q4_K_M",
        };

        let err = run(&client, &opts).await.expect_err("debe fallar");
        assert!(matches!(err, AppError::InvalidHubRef { .. }), "{err}");
        assert!(
            client.pull_requests().is_empty(),
            "no debe llamar a Ollama con una referencia inválida"
        );
    }
}
