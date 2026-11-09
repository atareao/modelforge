//! Lógica del subcomando `bench`: mide el rendimiento de un modelo en Ollama.
//!
//! El cálculo de tokens/s, la clasificación del *processor* y el renderizado de la
//! tabla comparativa son lógica pura y se prueban sin red. La obtención de datos
//! (una llamada a `/api/generate` y una consulta a `/api/ps`) vive en [`run`],
//! detrás del trait `crate::ollama::OllamaClient`.

use std::fmt;

use serde::{Serialize, Serializer};

use crate::error::AppError;
use crate::format::format_bytes;
use crate::ollama::{GenerateRequest, OllamaClient};

/// Prompt por defecto del benchmark.
pub const DEFAULT_PROMPT: &str =
    "Escribe un párrafo de unas 50 palabras explicando qué es Rust y para qué sirve.";

/// Medición de rendimiento de un modelo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BenchMeasurement {
    /// Modelo medido.
    pub model: String,
    /// Tokens generados (`eval_count`).
    pub eval_count: u64,
    /// Duración de la generación en nanosegundos (`eval_duration`).
    pub eval_duration_ns: u64,
    /// Duración de la carga del modelo en nanosegundos (`load_duration`).
    pub load_duration_ns: u64,
    /// Bytes residentes en VRAM (`size_vram`); `0` si no hay datos.
    pub size_vram: u64,
    /// Longitud de contexto cargada (`context_length`); `0` si no hay datos.
    pub context: u64,
    /// Procesador usado (GPU/CPU), derivado de `size` y `size_vram`.
    pub processor: Processor,
    /// Tokens por segundo calculados durante la generación.
    pub tokens_per_second: f64,
}

/// Tokens por segundo a partir de `eval_count` y `eval_duration` (nanosegundos).
///
/// Devuelve `0.0` si la duración es cero, evitando una división por cero.
#[must_use]
pub fn tokens_per_second(eval_count: u64, eval_duration_ns: u64) -> f64 {
    if eval_duration_ns == 0 {
        return 0.0;
    }
    eval_count as f64 / (eval_duration_ns as f64 / 1_000_000_000.0)
}

/// Procesador efectivo usado por el modelo, derivado de su tamaño y su VRAM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Processor {
    /// Todo el modelo en GPU.
    Gpu,
    /// Todo el modelo en CPU.
    Cpu,
    /// Reparto entre CPU y GPU (porcentajes enteros).
    Mixed {
        /// Porcentaje en CPU.
        cpu_pct: u8,
        /// Porcentaje en GPU.
        gpu_pct: u8,
    },
    /// No hay datos suficientes para clasificar.
    Unknown,
}

impl Processor {
    /// Clasifica el procesador a partir del tamaño total y de la VRAM ocupada.
    #[must_use]
    pub fn from_sizes(size: u64, size_vram: u64) -> Self {
        if size == 0 {
            return Self::Unknown;
        }
        if size_vram == 0 {
            return Self::Cpu;
        }
        if size_vram >= size {
            return Self::Gpu;
        }
        #[allow(clippy::cast_possible_truncation)]
        let gpu_pct = (size_vram as f64 / size as f64 * 100.0).round() as u8;
        Self::Mixed {
            cpu_pct: 100 - gpu_pct,
            gpu_pct,
        }
    }
}

impl fmt::Display for Processor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gpu => f.write_str("100% GPU"),
            Self::Cpu => f.write_str("100% CPU"),
            Self::Unknown => f.write_str("desconocido"),
            Self::Mixed { cpu_pct, gpu_pct } => {
                write!(f, "{cpu_pct}%/{gpu_pct}% CPU/GPU")
            }
        }
    }
}

impl Serialize for Processor {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Renderiza la tabla comparativa de varias mediciones (determinista).
#[must_use]
pub fn comparison_table(results: &[BenchMeasurement]) -> String {
    let rows = results
        .iter()
        .map(|result| {
            vec![
                result.model.clone(),
                format!("{:.2}", result.tokens_per_second),
                result.eval_count.to_string(),
                format_duration_ns(result.eval_duration_ns),
                format_duration_ns(result.load_duration_ns),
                format_bytes(result.size_vram),
                result.context.to_string(),
                result.processor.to_string(),
            ]
        })
        .collect::<Vec<_>>();

    crate::table::render_table(
        &[
            "model",
            "tok/s",
            "eval_count",
            "eval_duration",
            "load_duration",
            "size_vram",
            "context",
            "processor",
        ],
        &rows,
    )
}

/// Formatea una duración en nanosegundos con la unidad más legible.
#[must_use]
pub fn format_duration_ns(nanos: u64) -> String {
    if nanos >= 1_000_000_000 {
        format!("{:.2}s", nanos as f64 / 1_000_000_000.0)
    } else if nanos >= 1_000_000 {
        format!("{:.2}ms", nanos as f64 / 1_000_000.0)
    } else if nanos >= 1_000 {
        format!("{:.2}µs", nanos as f64 / 1_000.0)
    } else {
        format!("{nanos}ns")
    }
}

/// Opciones del subcomando `bench`.
#[derive(Debug, Clone)]
pub struct BenchOptions<'a> {
    /// Modelo a medir.
    pub model: &'a str,
    /// Modelo adicional con el que comparar (mismo *prompt*).
    pub compare: Option<&'a str>,
    /// *Prompt* a ejecutar en cada modelo.
    pub prompt: &'a str,
    /// Límite de tokens generados por ejecución.
    pub num_predict: Option<u32>,
}

/// Resultado agregado del subcomando `bench`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BenchReport {
    /// *Prompt* usado en las mediciones.
    pub prompt: String,
    /// Mediciones, en el orden de ejecución.
    pub results: Vec<BenchMeasurement>,
}

/// Mide el rendimiento de uno o dos modelos con el mismo *prompt*.
///
/// # Errors
///
/// Devuelve [`AppError::Ollama`] / [`AppError::OllamaUnavailable`] si el servidor
/// falla al generar o al consultar `/api/ps`.
pub async fn run<C: OllamaClient + Sync>(
    client: &C,
    options: &BenchOptions<'_>,
) -> Result<BenchReport, AppError> {
    let models: Vec<&str> = std::iter::once(options.model)
        .chain(options.compare)
        .collect();

    let mut results = Vec::with_capacity(models.len());
    for model in models {
        let request = GenerateRequest {
            model: model.to_string(),
            prompt: options.prompt.to_string(),
            stream: false,
            think: Some(false),
            options: options
                .num_predict
                .map(|limit| serde_json::json!({ "num_predict": limit })),
        };
        let response = client.generate(&request).await?;

        let running = client
            .running()
            .await?
            .into_iter()
            .find(|entry| entry.name == model || entry.model == model);
        let (size, size_vram, context) = running.map_or((0, 0, 0), |entry| {
            (entry.size, entry.size_vram, entry.context_length)
        });

        results.push(BenchMeasurement {
            model: model.to_string(),
            eval_count: response.eval_count,
            eval_duration_ns: response.eval_duration,
            load_duration_ns: response.load_duration,
            size_vram,
            context,
            processor: Processor::from_sizes(size, size_vram),
            tokens_per_second: tokens_per_second(response.eval_count, response.eval_duration),
        });
    }

    Ok(BenchReport {
        prompt: options.prompt.to_string(),
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ollama::{GenerateResponse, RunningModel, fake::FakeOllamaClient};

    fn measurement(model: &str) -> BenchMeasurement {
        BenchMeasurement {
            model: model.to_string(),
            eval_count: 100,
            eval_duration_ns: 2_000_000_000,
            load_duration_ns: 500_000,
            size_vram: 8_000_000_000,
            context: 262_144,
            processor: Processor::Gpu,
            tokens_per_second: 50.0,
        }
    }

    #[test]
    fn computes_tokens_per_second() {
        assert_eq!(tokens_per_second(100, 1_000_000_000), 100.0);
        assert!((tokens_per_second(8, 191_183_000) - 41.844_2).abs() < 0.01);
    }

    #[test]
    fn tokens_per_second_is_zero_for_zero_duration() {
        assert_eq!(tokens_per_second(100, 0), 0.0);
        assert_eq!(tokens_per_second(0, 1_000_000_000), 0.0);
    }

    #[test]
    fn classifies_processor_from_sizes() {
        assert_eq!(Processor::from_sizes(1000, 1000), Processor::Gpu);
        assert_eq!(Processor::from_sizes(1000, 0), Processor::Cpu);
        assert_eq!(Processor::from_sizes(0, 0), Processor::Unknown);
        assert_eq!(
            Processor::from_sizes(1000, 520),
            Processor::Mixed {
                cpu_pct: 48,
                gpu_pct: 52
            }
        );
    }

    #[test]
    fn formats_processor_labels() {
        assert_eq!(Processor::Gpu.to_string(), "100% GPU");
        assert_eq!(Processor::Cpu.to_string(), "100% CPU");
        assert_eq!(Processor::Unknown.to_string(), "desconocido");
        assert_eq!(
            Processor::Mixed {
                cpu_pct: 48,
                gpu_pct: 52
            }
            .to_string(),
            "48%/52% CPU/GPU"
        );
    }

    #[test]
    fn renders_comparison_table() {
        let table = comparison_table(&[measurement("a:latest"), measurement("bb:latest")]);

        let lines: Vec<&str> = table.lines().collect();
        assert!(lines[0].contains("model"));
        assert!(lines[0].contains("tok/s"));
        assert!(lines[0].contains("processor"));
        assert!(lines[2].contains("a:latest"));
        assert!(lines[2].contains("50.00"));
        assert!(lines[2].contains("100% GPU"));
        assert!(lines[3].contains("bb:latest"));
    }

    #[test]
    fn comparison_table_reports_sizes_and_context() {
        let table = comparison_table(&[measurement("a:latest")]);
        // `size_vram` se muestra en unidades binarias legibles.
        assert!(table.contains(&format_bytes(8_000_000_000)), "{table}");
        assert!(table.contains("262144"), "{table}");
    }

    fn options<'a>(model: &'a str, compare: Option<&'a str>, prompt: &'a str) -> BenchOptions<'a> {
        BenchOptions {
            model,
            compare,
            prompt,
            num_predict: Some(64),
        }
    }

    fn running_model(name: &str) -> RunningModel {
        crate::ollama::RunningModel {
            name: name.to_string(),
            model: name.to_string(),
            size: 8_000_000_000,
            size_vram: 8_000_000_000,
            context_length: 262_144,
        }
    }

    #[tokio::test]
    async fn run_measures_a_single_model() {
        let generate = GenerateResponse {
            response: "texto".into(),
            eval_count: 100,
            eval_duration: 2_000_000_000,
            load_duration: 500_000,
            ..GenerateResponse::default()
        };
        let client = FakeOllamaClient::new()
            .with_generate_response(generate)
            .with_running(vec![running_model("mi-modelo")]);

        let report = run(&client, &options("mi-modelo", None, "hola"))
            .await
            .expect("bench");

        assert_eq!(report.prompt, "hola");
        assert_eq!(report.results.len(), 1);
        let result = &report.results[0];
        assert_eq!(result.model, "mi-modelo");
        assert_eq!(result.eval_count, 100);
        assert_eq!(result.eval_duration_ns, 2_000_000_000);
        assert_eq!(result.load_duration_ns, 500_000);
        assert_eq!(result.tokens_per_second, 50.0);
        assert_eq!(result.processor, Processor::Gpu);
        assert_eq!(result.context, 262_144);
        assert_eq!(result.size_vram, 8_000_000_000);

        let requests = client.generate_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].model, "mi-modelo");
        assert_eq!(requests[0].prompt, "hola");
        assert!(!requests[0].stream);
    }

    #[tokio::test]
    async fn run_compares_two_models_in_order() {
        let generate = GenerateResponse {
            eval_count: 10,
            eval_duration: 1_000_000_000,
            ..GenerateResponse::default()
        };
        let client = FakeOllamaClient::new().with_generate_response(generate);

        let report = run(&client, &options("a", Some("b"), "prompt"))
            .await
            .expect("bench");

        let models: Vec<&str> = report.results.iter().map(|r| r.model.as_str()).collect();
        assert_eq!(models, vec!["a", "b"]);
        assert_eq!(client.generate_requests().len(), 2);
    }

    #[tokio::test]
    async fn run_without_running_model_reports_unknown_processor() {
        let client = FakeOllamaClient::new().with_generate_response(GenerateResponse::default());

        let report = run(&client, &options("a", None, "prompt"))
            .await
            .expect("bench");

        let result = &report.results[0];
        assert_eq!(result.processor, Processor::Unknown);
        assert_eq!(result.size_vram, 0);
        assert_eq!(result.context, 0);
    }

    #[tokio::test]
    async fn run_surfaces_ollama_errors() {
        let client = FakeOllamaClient::failing("modelo desconocido");

        let err = run(&client, &options("a", None, "prompt"))
            .await
            .expect_err("debe propagar el error");
        assert!(matches!(err, AppError::Ollama { .. }), "{err}");
    }
}
