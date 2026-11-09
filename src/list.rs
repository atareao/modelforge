//! Lógica de los subcomandos `list` e `info`.
//!
//! `list` cruza los modelos instalados (`/api/tags`) con los cargados en memoria
//! (`/api/ps`) para reportar el contexto y la VRAM de cada uno; `info` añade el
//! `processor` y las capacidades de `show`. La lógica pura de emparejamiento y el
//! renderizado de la tabla se prueban con un doble del trait [`OllamaClient`].

use serde::Serialize;

use crate::bench::Processor;
use crate::error::AppError;
use crate::ollama::{OllamaClient, RunningModel, TaggedModel};
use crate::table::render_table;

/// Resumen de un modelo instalado, para `list`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelSummary {
    /// Nombre del modelo.
    pub name: String,
    /// Longitud de contexto cargada (`/api/ps`); `0` si no está cargado.
    pub context: u64,
    /// Bytes residentes en VRAM (`/api/ps`); `0` si no está cargado.
    pub size_vram: u64,
    /// Procesador efectivo derivado de `size` y `size_vram`.
    pub processor: Processor,
}

/// Resultado agregado del subcomando `list`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ListReport {
    /// Modelos instalados, en el orden reportado por `/api/tags`.
    pub models: Vec<ModelSummary>,
}

/// Detalles de un modelo, para `info`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ModelDetails {
    /// Nombre del modelo.
    pub name: String,
    /// Contexto configurado (`num_ctx` de `show`, o el cargado si no se indica).
    pub context: u64,
    /// Bytes residentes en VRAM (`/api/ps`); `0` si no está cargado.
    pub size_vram: u64,
    /// Procesador efectivo derivado de `size` y `size_vram`.
    pub processor: Processor,
    /// Capacidades reportadas por `show` (`completion`, `vision`, `tools`…).
    pub capabilities: Vec<String>,
}

/// Empareja un modelo cargado con un nombre dado, por `model` o `name`.
fn matching_running<'a>(running: &'a [RunningModel], name: &str) -> Option<&'a RunningModel> {
    running
        .iter()
        .find(|entry| entry.model == name || entry.name == name)
}

/// Resume un modelo instalado cruzando `/api/tags` con `/api/ps`.
#[must_use]
pub fn summarize(tagged: &TaggedModel, running: &[RunningModel]) -> ModelSummary {
    let name = tagged.canonical_name().to_string();
    match matching_running(running, &name).or_else(|| matching_running(running, &tagged.name)) {
        Some(loaded) => ModelSummary {
            name,
            context: loaded.context_length,
            size_vram: loaded.size_vram,
            processor: Processor::from_sizes(loaded.size, loaded.size_vram),
        },
        None => ModelSummary {
            name,
            context: 0,
            size_vram: 0,
            processor: Processor::Unknown,
        },
    }
}

/// Lista los modelos instalados con su contexto y VRAM.
///
/// # Errors
///
/// Devuelve [`AppError::Ollama`] / [`AppError::OllamaUnavailable`] si el servidor
/// falla al consultar `/api/tags` o `/api/ps`.
pub async fn run<C: OllamaClient + Sync>(client: &C) -> Result<ListReport, AppError> {
    let tags = client.tags().await?;
    let running = client.running().await?;
    let models = tags
        .iter()
        .map(|tagged| summarize(tagged, &running))
        .collect();
    Ok(ListReport { models })
}

/// Obtiene los detalles de un modelo cruzando `show` con `/api/ps`.
///
/// # Errors
///
/// Devuelve [`AppError::Ollama`] / [`AppError::OllamaUnavailable`] si el servidor
/// falla al consultar `/api/show` o `/api/ps`.
pub async fn run_info<C: OllamaClient + Sync>(
    client: &C,
    name: &str,
) -> Result<ModelDetails, AppError> {
    let info = client.show(name).await?;
    let running = client.running().await?;
    let loaded = matching_running(&running, name);

    let context = info
        .num_ctx()
        .map(u64::from)
        .or_else(|| loaded.map(|entry| entry.context_length))
        .unwrap_or(0);
    let (size_vram, processor) = loaded.map_or((0, Processor::Unknown), |entry| {
        (
            entry.size_vram,
            Processor::from_sizes(entry.size, entry.size_vram),
        )
    });

    Ok(ModelDetails {
        name: name.to_string(),
        context,
        size_vram,
        processor,
        capabilities: info.capabilities,
    })
}

/// Renderiza el listado como tabla de texto (determinista).
#[must_use]
pub fn table(report: &ListReport) -> String {
    let rows = report
        .models
        .iter()
        .map(|model| {
            vec![
                model.name.clone(),
                model.context.to_string(),
                crate::format::format_bytes(model.size_vram),
                model.processor.to_string(),
            ]
        })
        .collect::<Vec<_>>();

    render_table(&["model", "context", "size_vram", "processor"], &rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ollama::ModelInfo;
    use crate::ollama::fake::FakeOllamaClient;

    fn tagged(name: &str) -> TaggedModel {
        TaggedModel {
            name: name.to_string(),
            model: name.to_string(),
            size: 8_000_000_000,
            digest: "sha256:abc".into(),
            modified_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn running(name: &str, size: u64, size_vram: u64, context: u64) -> RunningModel {
        RunningModel {
            name: name.to_string(),
            model: name.to_string(),
            size,
            size_vram,
            context_length: context,
        }
    }

    #[test]
    fn summarize_uses_ps_data_for_loaded_model() {
        let summary = summarize(
            &tagged("qwen3:8b"),
            &[running("qwen3:8b", 9_000_000_000, 9_000_000_000, 32_768)],
        );

        assert_eq!(summary.name, "qwen3:8b");
        assert_eq!(summary.context, 32_768);
        assert_eq!(summary.size_vram, 9_000_000_000);
        assert_eq!(summary.processor, Processor::Gpu);
    }

    #[test]
    fn summarize_marks_not_loaded_model_as_unknown() {
        let summary = summarize(&tagged("llama3.2:3b"), &[]);

        assert_eq!(summary.name, "llama3.2:3b");
        assert_eq!(summary.context, 0);
        assert_eq!(summary.size_vram, 0);
        assert_eq!(summary.processor, Processor::Unknown);
    }

    #[test]
    fn summarize_classifies_mixed_processor() {
        let summary = summarize(&tagged("mixto"), &[running("mixto", 1000, 520, 4096)]);

        assert_eq!(
            summary.processor,
            Processor::Mixed {
                cpu_pct: 48,
                gpu_pct: 52
            }
        );
    }

    #[tokio::test]
    async fn run_lists_all_installed_models_in_order() {
        let client = FakeOllamaClient::new()
            .with_tags(vec![tagged("a:1"), tagged("b:2")])
            .with_running(vec![running("b:2", 1000, 1000, 8192)]);

        let report = run(&client).await.expect("list");

        assert_eq!(report.models.len(), 2);
        assert_eq!(report.models[0].name, "a:1");
        assert_eq!(report.models[0].processor, Processor::Unknown);
        assert_eq!(report.models[1].name, "b:2");
        assert_eq!(report.models[1].context, 8192);
        assert_eq!(report.models[1].processor, Processor::Gpu);
    }

    #[tokio::test]
    async fn run_surfaces_ollama_errors() {
        let client = FakeOllamaClient::failing("servidor caído");
        let err = run(&client).await.expect_err("debe propagar el error");
        assert!(matches!(err, AppError::Ollama { .. }), "{err}");
    }

    #[tokio::test]
    async fn run_info_reports_context_vram_and_processor() {
        let info = ModelInfo {
            capabilities: vec!["completion".into(), "tools".into()],
            parameters: "num_ctx                        262144\n".into(),
        };
        let client = FakeOllamaClient::new()
            .with_info(info)
            .with_running(vec![running(
                "grande:q6",
                12_000_000_000,
                12_000_000_000,
                262_144,
            )]);

        let details = run_info(&client, "grande:q6").await.expect("info");

        assert_eq!(details.name, "grande:q6");
        assert_eq!(details.context, 262_144);
        assert_eq!(details.size_vram, 12_000_000_000);
        assert_eq!(details.processor, Processor::Gpu);
        assert_eq!(details.capabilities, vec!["completion", "tools"]);
    }

    #[tokio::test]
    async fn run_info_falls_back_to_loaded_context() {
        let client = FakeOllamaClient::new()
            .with_info(ModelInfo::default())
            .with_running(vec![running("m", 1000, 1000, 4096)]);

        let details = run_info(&client, "m").await.expect("info");

        assert_eq!(details.context, 4096);
        assert_eq!(details.processor, Processor::Gpu);
    }

    #[tokio::test]
    async fn run_info_without_loaded_model_is_unknown() {
        let client = FakeOllamaClient::new().with_info(ModelInfo {
            capabilities: vec!["completion".into()],
            parameters: "num_ctx                        32768\n".into(),
        });

        let details = run_info(&client, "m").await.expect("info");

        assert_eq!(details.context, 32_768);
        assert_eq!(details.size_vram, 0);
        assert_eq!(details.processor, Processor::Unknown);
    }

    #[test]
    fn renders_listing_table() {
        let report = ListReport {
            models: vec![ModelSummary {
                name: "qwen3:8b".into(),
                context: 32_768,
                size_vram: 9_000_000_000,
                processor: Processor::Gpu,
            }],
        };

        let table = table(&report);
        let lines: Vec<&str> = table.lines().collect();
        assert!(lines[0].contains("model"));
        assert!(lines[0].contains("context"));
        assert!(lines[0].contains("size_vram"));
        assert!(lines[0].contains("processor"));
        assert!(lines[2].contains("qwen3:8b"));
        assert!(lines[2].contains("32768"));
        assert!(lines[2].contains("100% GPU"));
    }
}
