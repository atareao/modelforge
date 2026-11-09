//! Generación determinista del *Modelfile* de Ollama a partir de un GGUF.
//!
//! Lógica pura y sin red: produce el texto del Modelfile con las líneas `FROM`
//! (una para el GGUF y otra para el `mmproj` si hay visión), `RENDERER`,
//! `PARSER`, los `PARAMETER` de muestreo y `num_ctx`.

use std::path::Path;

/// Renderer por defecto del Modelfile (coincide con el modelo de referencia).
pub const DEFAULT_RENDERER: &str = "qwen3.5";

/// Parser por defecto del Modelfile (coincide con el modelo de referencia).
pub const DEFAULT_PARSER: &str = "qwen3.5";

/// `num_ctx` por defecto cuando no se indica `--ctx`.
pub const DEFAULT_NUM_CTX: u32 = 262_144;

/// Parámetro del Modelfile en forma `PARAMETER <key> <value>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    /// Nombre del parámetro (p. ej. `temperature`).
    pub key: String,
    /// Valor del parámetro (p. ej. `1`).
    pub value: String,
}

impl Parameter {
    /// Crea un parámetro a partir de su clave y su valor.
    #[must_use]
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    /// Valor interpretado como JSON: entero, decimal, booleano o cadena.
    ///
    /// La API de Ollama espera los valores tipados en el mapa `parameters`.
    #[must_use]
    pub fn json_value(&self) -> serde_json::Value {
        if let Ok(int) = self.value.parse::<i64>() {
            return serde_json::Value::from(int);
        }
        if let Ok(float) = self.value.parse::<f64>() {
            return serde_json::Value::from(float);
        }
        match self.value.to_ascii_lowercase().as_str() {
            "true" => serde_json::Value::Bool(true),
            "false" => serde_json::Value::Bool(false),
            _ => serde_json::Value::String(self.value.clone()),
        }
    }
}

/// Especificación del Modelfile a renderizar.
#[derive(Debug, Clone)]
pub struct ModelfileSpec<'a> {
    /// Fichero GGUF de origen (`FROM`).
    pub from: &'a Path,
    /// Proyector de visión (`FROM` adicional), si hay.
    pub mmproj: Option<&'a Path>,
    /// Renderer (`RENDERER`); si es `None` se usa [`DEFAULT_RENDERER`].
    pub renderer: Option<&'a str>,
    /// Parser (`PARSER`); si es `None` se usa [`DEFAULT_PARSER`].
    pub parser: Option<&'a str>,
    /// Tamaño de contexto (`num_ctx`); si es `None` se usa [`DEFAULT_NUM_CTX`].
    pub num_ctx: Option<u32>,
    /// Parámetros de muestreo, en el orden en que se emiten.
    pub parameters: Vec<Parameter>,
}

impl<'a> ModelfileSpec<'a> {
    /// Crea una especificación con los valores por defecto y los parámetros de muestreo.
    #[must_use]
    pub fn new(from: &'a Path) -> Self {
        Self {
            from,
            mmproj: None,
            renderer: None,
            parser: None,
            num_ctx: None,
            parameters: default_parameters(),
        }
    }

    /// Fija el `mmproj` (proyector de visión).
    #[must_use]
    pub fn with_mmproj(mut self, mmproj: Option<&'a Path>) -> Self {
        self.mmproj = mmproj;
        self
    }

    /// Fija un `RENDERER` explícito.
    #[must_use]
    pub fn with_renderer(mut self, renderer: Option<&'a str>) -> Self {
        self.renderer = renderer;
        self
    }

    /// Fija un `PARSER` explícito.
    #[must_use]
    pub fn with_parser(mut self, parser: Option<&'a str>) -> Self {
        self.parser = parser;
        self
    }

    /// Fija un `num_ctx` explícito.
    #[must_use]
    pub fn with_num_ctx(mut self, num_ctx: Option<u32>) -> Self {
        self.num_ctx = num_ctx;
        self
    }

    /// Sustituye la lista de parámetros de muestreo.
    #[must_use]
    pub fn with_parameters(mut self, parameters: Vec<Parameter>) -> Self {
        self.parameters = parameters;
        self
    }

    /// Renderer efectivo (el indicado o [`DEFAULT_RENDERER`]).
    #[must_use]
    pub fn effective_renderer(&self) -> &str {
        self.renderer.unwrap_or(DEFAULT_RENDERER)
    }

    /// Parser efectivo (el indicado o [`DEFAULT_PARSER`]).
    #[must_use]
    pub fn effective_parser(&self) -> &str {
        self.parser.unwrap_or(DEFAULT_PARSER)
    }

    /// `num_ctx` efectivo (el indicado o [`DEFAULT_NUM_CTX`]).
    #[must_use]
    pub fn effective_num_ctx(&self) -> u32 {
        self.num_ctx.unwrap_or(DEFAULT_NUM_CTX)
    }

    /// Renderiza el Modelfile completo (termina en salto de línea).
    #[must_use]
    pub fn render(&self) -> String {
        let mut lines = vec![format!("FROM {}", self.from.display())];
        if let Some(mmproj) = self.mmproj {
            lines.push(format!("FROM {}", mmproj.display()));
        }
        lines.push(format!("RENDERER {}", self.effective_renderer()));
        lines.push(format!("PARSER {}", self.effective_parser()));
        for parameter in &self.parameters {
            lines.push(format!("PARAMETER {} {}", parameter.key, parameter.value));
        }
        lines.push(format!("PARAMETER num_ctx {}", self.effective_num_ctx()));

        let mut out = lines.join("\n");
        out.push('\n');
        out
    }
}

/// Parámetros de muestreo por defecto (alineados con el modelo de referencia).
#[must_use]
pub fn default_parameters() -> Vec<Parameter> {
    vec![
        Parameter::new("temperature", "1"),
        Parameter::new("top_k", "20"),
        Parameter::new("top_p", "0.95"),
        Parameter::new("repeat_penalty", "1"),
        Parameter::new("presence_penalty", "1.5"),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Une líneas con `\n` y añade el salto de línea final del Modelfile.
    fn text(lines: &[&str]) -> String {
        let mut out = lines.join("\n");
        out.push('\n');
        out
    }

    #[test]
    fn renders_defaults_without_mmproj() {
        let spec = ModelfileSpec::new(Path::new("/models/model-Q6_K.gguf"));
        assert_eq!(
            spec.render(),
            text(&[
                "FROM /models/model-Q6_K.gguf",
                "RENDERER qwen3.5",
                "PARSER qwen3.5",
                "PARAMETER temperature 1",
                "PARAMETER top_k 20",
                "PARAMETER top_p 0.95",
                "PARAMETER repeat_penalty 1",
                "PARAMETER presence_penalty 1.5",
                "PARAMETER num_ctx 262144",
            ])
        );
    }

    #[test]
    fn renders_vision_with_two_from_lines() {
        let spec = ModelfileSpec::new(Path::new("/models/model-Q6_K.gguf"))
            .with_mmproj(Some(Path::new("/models/mmproj-f16.gguf")));
        assert_eq!(
            spec.render(),
            text(&[
                "FROM /models/model-Q6_K.gguf",
                "FROM /models/mmproj-f16.gguf",
                "RENDERER qwen3.5",
                "PARSER qwen3.5",
                "PARAMETER temperature 1",
                "PARAMETER top_k 20",
                "PARAMETER top_p 0.95",
                "PARAMETER repeat_penalty 1",
                "PARAMETER presence_penalty 1.5",
                "PARAMETER num_ctx 262144",
            ])
        );
    }

    #[test]
    fn honors_custom_ctx() {
        let spec = ModelfileSpec::new(Path::new("/models/model.gguf")).with_num_ctx(Some(8192));
        assert!(spec.render().contains("PARAMETER num_ctx 8192\n"));
        assert!(!spec.render().contains("262144"));
    }

    #[test]
    fn honors_renderer_and_parser_overrides() {
        let spec = ModelfileSpec::new(Path::new("/models/model.gguf"))
            .with_renderer(Some("deepseek"))
            .with_parser(Some("deepseek"));
        let rendered = spec.render();
        assert!(rendered.contains("RENDERER deepseek\n"), "{rendered}");
        assert!(rendered.contains("PARSER deepseek\n"), "{rendered}");
        assert!(!rendered.contains("qwen3.5"), "{rendered}");
    }

    #[test]
    fn renders_custom_parameters_in_order() {
        let spec = ModelfileSpec::new(Path::new("/models/model.gguf")).with_parameters(vec![
            Parameter::new("temperature", "0.6"),
            Parameter::new("top_k", "40"),
        ]);
        assert_eq!(
            spec.render(),
            text(&[
                "FROM /models/model.gguf",
                "RENDERER qwen3.5",
                "PARSER qwen3.5",
                "PARAMETER temperature 0.6",
                "PARAMETER top_k 40",
                "PARAMETER num_ctx 262144",
            ])
        );
    }

    #[test]
    fn default_parameters_match_reference_model() {
        assert_eq!(
            default_parameters(),
            vec![
                Parameter::new("temperature", "1"),
                Parameter::new("top_k", "20"),
                Parameter::new("top_p", "0.95"),
                Parameter::new("repeat_penalty", "1"),
                Parameter::new("presence_penalty", "1.5"),
            ]
        );
    }
}
