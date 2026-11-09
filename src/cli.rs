//! Definición de la interfaz de línea de comandos (flags globales y subcomandos).
//!
//! En este incremento solo se declara la *superficie* del CLI: flags globales y
//! subcomandos. La lógica de cada subcomando se implementa en incrementos
//! posteriores; aquí los cuerpos son *stubs*.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Importa modelos GGUF desde HuggingFace, créalos en Ollama y enrútalos en el gateway.
///
/// Resuelve repos de HuggingFace, descarga GGUF reanudables, genera un Modelfile
/// reproducible, valida el modelo y, opcionalmente, añade su ruta al
/// `gateway.yaml`.
#[derive(Debug, Parser)]
#[command(name = "modelforge", version, propagate_version = true)]
pub struct Cli {
    /// Flags globales aplicables a todos los subcomandos.
    #[command(flatten)]
    pub global: GlobalArgs,

    /// Subcomando a ejecutar.
    #[command(subcommand)]
    pub command: Command,
}

/// Flags globales compartidos por todos los subcomandos.
#[derive(Debug, Args)]
pub struct GlobalArgs {
    /// Ruta al fichero de configuración.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// URL base de la API de Ollama.
    #[arg(
        long,
        global = true,
        value_name = "URL",
        default_value = "http://localhost:11434"
    )]
    pub ollama: String,

    /// Emite la salida en formato JSON.
    #[arg(long, global = true)]
    pub json: bool,

    /// Aumenta el nivel de detalle de los logs.
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,
}

/// Subcomandos disponibles.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Importa un modelo: descarga un GGUF de HuggingFace o importa de forma nativa
    /// un directorio local con `.safetensors`.
    Import(ImportArgs),
    /// Crea un modelo en Ollama a partir de un Modelfile.
    Create(CreateArgs),
    /// Ejecuta pruebas de humo (texto, thinking, tools, vision).
    Test(TestArgs),
    /// Mide el rendimiento del modelo.
    Bench(BenchArgs),
    /// Lista los modelos disponibles en Ollama.
    List(ListArgs),
    /// Muestra el contexto, VRAM y processor de un modelo.
    Info(InfoArgs),
    /// Añade una ruta del modelo al `gateway.yaml`.
    Route(RouteArgs),
    /// Descarga un GGUF del hub delegando en Ollama (`hf.co/<repo>:<quant>`).
    Pull(PullArgs),
}

/// Backend con el que procesar un directorio de safetensors en `import`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum Backend {
    /// Backend nativo de Ollama (por defecto; en Linux solo si el host lo soporta).
    #[default]
    Ollama,
    /// Backend de llama.cpp: convierte safetensors → GGUF antes de crear en Ollama.
    #[value(name = "llama.cpp")]
    LlamaCpp,
}

/// Argumentos de `import`.
#[derive(Debug, Args)]
pub struct ImportArgs {
    /// Fuente: repo de HuggingFace (`org/model`), directorio local con
    /// `.safetensors` o referencia del hub (`hf.co/<repo>:<quant>`).
    pub source: String,

    /// Backend de import para un directorio de safetensors (`ollama` o `llama.cpp`).
    #[arg(long, value_name = "BACKEND", default_value = "ollama")]
    pub backend: Backend,

    /// Quant a descargar de un repo GGUF (p. ej. `Q6_K`).
    #[arg(long, short = 'q', value_name = "QUANT")]
    pub quant: Option<String>,

    /// Nombre del modelo a crear en Ollama (import nativo de safetensors).
    #[arg(long, value_name = "MODEL")]
    pub name: Option<String>,

    /// Cuantización nativa de Ollama a aplicar al crear (p. ej. `nvfp4`).
    #[arg(long, value_name = "QUANT")]
    pub quantize: Option<String>,

    /// Descarga también el `mmproj` (proyector de visión).
    #[arg(long)]
    pub mmproj: bool,

    /// Directorio de destino de la descarga.
    #[arg(long, value_name = "DIR")]
    pub dest: Option<PathBuf>,
}

/// Argumentos de `create`.
#[derive(Debug, Args)]
pub struct CreateArgs {
    /// Nombre del modelo a crear en Ollama.
    pub name: String,

    /// Fichero GGUF o directorio de safetensors de origen (`FROM`).
    #[arg(long, value_name = "PATH")]
    pub from: Option<PathBuf>,

    /// Proyector de visión para el modelo (`mmproj`).
    #[arg(long, value_name = "MMPROJ")]
    pub mmproj: Option<PathBuf>,

    /// Tamaño de contexto (`num_ctx`).
    #[arg(long, value_name = "N")]
    pub ctx: Option<u32>,

    /// Renderer del Modelfile (`RENDERER`).
    #[arg(long, value_name = "NAME")]
    pub renderer: Option<String>,

    /// Parser del Modelfile (`PARSER`).
    #[arg(long, value_name = "NAME")]
    pub parser: Option<String>,

    /// Cuantización nativa de Ollama a aplicar en la creación de safetensors (p. ej. `nvfp4`).
    #[arg(long, value_name = "QUANT")]
    pub quantize: Option<String>,
}

/// Argumentos de `test`.
#[derive(Debug, Args)]
pub struct TestArgs {
    /// Nombre del modelo a probar.
    pub name: String,

    /// Prueba de *tool-calling*.
    #[arg(long)]
    pub tools: bool,

    /// Prueba de visión.
    #[arg(long)]
    pub vision: bool,

    /// Imagen para la prueba de visión (por defecto, una embebida de prueba).
    #[arg(long, value_name = "PATH")]
    pub image: Option<PathBuf>,

    /// Límite de tokens generados por prueba.
    #[arg(long, value_name = "N")]
    pub num_predict: Option<u32>,
}

/// Argumentos de `bench`.
#[derive(Debug, Args)]
pub struct BenchArgs {
    /// Nombre del modelo a medir.
    pub name: String,

    /// Modelo con el que comparar (mismo prompt).
    #[arg(long, value_name = "MODEL")]
    pub compare: Option<String>,

    /// Prompt a ejecutar en cada modelo.
    #[arg(long, value_name = "TEXT")]
    pub prompt: Option<String>,

    /// Límite de tokens generados por ejecución.
    #[arg(long, value_name = "N")]
    pub num_predict: Option<u32>,
}

/// Argumentos de `list`.
#[derive(Debug, Args)]
pub struct ListArgs {}

/// Argumentos de `info`.
#[derive(Debug, Args)]
pub struct InfoArgs {
    /// Nombre del modelo a inspeccionar.
    pub name: String,
}

/// Argumentos de `route`.
#[derive(Debug, Args)]
pub struct RouteArgs {
    /// Nombre de la ruta a añadir.
    pub name: String,

    /// Modelo asociado a la ruta.
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Muestra los cambios sin escribir el fichero.
    #[arg(long)]
    pub dry_run: bool,
}

/// Argumentos de `pull`.
#[derive(Debug, Args)]
pub struct PullArgs {
    /// Referencia `hf.co/<repo>:<quant>` (p. ej. `hf.co/usuario/modelo:Q4_K_M`).
    pub reference: String,
}

impl Command {
    /// Nombre canónico del subcomando, usado en mensajes de error.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Command::Import(_) => "import",
            Command::Create(_) => "create",
            Command::Test(_) => "test",
            Command::Bench(_) => "bench",
            Command::List(_) => "list",
            Command::Info(_) => "info",
            Command::Route(_) => "route",
            Command::Pull(_) => "pull",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{CommandFactory, Parser};

    /// Los cuatro flags globales se parsean con los valores indicados.
    #[test]
    fn parses_global_flags() {
        let cli = Cli::try_parse_from([
            "modelforge",
            "--config",
            "/tmp/modelforge.yaml",
            "--ollama",
            "http://ollama.local:11434",
            "--json",
            "--verbose",
            "list",
        ])
        .expect("los flags globales deben parsearse");

        assert_eq!(
            cli.global.config.as_deref(),
            Some(std::path::Path::new("/tmp/modelforge.yaml"))
        );
        assert_eq!(cli.global.ollama, "http://ollama.local:11434");
        assert!(cli.global.json);
        assert!(cli.global.verbose);
    }

    /// Sin flags, se aplican los valores por defecto sensatos.
    #[test]
    fn applies_global_defaults() {
        let cli = Cli::try_parse_from(["modelforge", "list"]).expect("debe parsear");

        assert_eq!(cli.global.config, None);
        assert_eq!(cli.global.ollama, "http://localhost:11434");
        assert!(!cli.global.json);
        assert!(!cli.global.verbose);
    }

    /// Los flags globales también se aceptan después del subcomando.
    #[test]
    fn accepts_global_flags_after_subcommand() {
        let cli = Cli::try_parse_from(["modelforge", "list", "--json", "--verbose"])
            .expect("los flags globales deben ser globales");

        assert!(cli.global.json);
        assert!(cli.global.verbose);
        assert!(matches!(cli.command, Command::List(_)));
    }

    /// Existen todos los subcomandos exigidos por el spec.
    #[test]
    fn recognizes_all_subcommands() {
        let cases: [(&[&str], &str); 8] = [
            (&["modelforge", "import", "org/repo"], "import"),
            (&["modelforge", "create", "model"], "create"),
            (&["modelforge", "test", "model"], "test"),
            (&["modelforge", "bench", "model"], "bench"),
            (&["modelforge", "list"], "list"),
            (&["modelforge", "info", "model"], "info"),
            (&["modelforge", "route", "model"], "route"),
            (&["modelforge", "pull", "hf.co/usuario/repo:Q4_K_M"], "pull"),
        ];

        for (argv, expected) in cases {
            let cli = Cli::try_parse_from(argv)
                .unwrap_or_else(|e| panic!("`{}` debe parsearse: {e}", argv.join(" ")));
            assert_eq!(
                cli.command.name(),
                expected,
                "subcomando incorrecto en {argv:?}"
            );
        }
    }

    /// `import` acepta `--name` y `--quantize` además de `--quant`.
    #[test]
    fn import_accepts_native_flags() {
        let cli = Cli::try_parse_from([
            "modelforge",
            "import",
            "./modelo",
            "--name",
            "mi-modelo",
            "--quantize",
            "nvfp4",
        ])
        .expect("debe parsear");

        match cli.command {
            Command::Import(args) => {
                assert_eq!(args.source, "./modelo");
                assert_eq!(args.name.as_deref(), Some("mi-modelo"));
                assert_eq!(args.quantize.as_deref(), Some("nvfp4"));
                assert_eq!(args.quant, None);
            }
            other => panic!("se esperaba Import, se obtuvo {other:?}"),
        }
    }

    /// Sin `--backend`, `import` usa el backend `ollama` por defecto.
    #[test]
    fn import_backend_defaults_to_ollama() {
        let cli = Cli::try_parse_from(["modelforge", "import", "./modelo"]).expect("debe parsear");
        match cli.command {
            Command::Import(args) => assert_eq!(args.backend, Backend::Ollama),
            other => panic!("se esperaba Import, se obtuvo {other:?}"),
        }
    }

    /// `import` acepta `--backend llama.cpp`.
    #[test]
    fn import_accepts_llamacpp_backend() {
        let cli = Cli::try_parse_from([
            "modelforge",
            "import",
            "./modelo",
            "--backend",
            "llama.cpp",
            "--name",
            "mi-modelo",
        ])
        .expect("debe parsear");
        match cli.command {
            Command::Import(args) => assert_eq!(args.backend, Backend::LlamaCpp),
            other => panic!("se esperaba Import, se obtuvo {other:?}"),
        }
    }

    /// Un backend desconocido se rechaza al parsear.
    #[test]
    fn rejects_unknown_backend() {
        assert!(
            Cli::try_parse_from(["modelforge", "import", "./modelo", "--backend", "vllm"]).is_err()
        );
    }

    /// `create` acepta `--quantize`.
    #[test]
    fn create_accepts_quantize_flag() {
        let cli = Cli::try_parse_from([
            "modelforge",
            "create",
            "m",
            "--from",
            "./modelo",
            "--quantize",
            "mxfp4",
        ])
        .expect("debe parsear");

        match cli.command {
            Command::Create(args) => assert_eq!(args.quantize.as_deref(), Some("mxfp4")),
            other => panic!("se esperaba Create, se obtuvo {other:?}"),
        }
    }

    /// El nombre del subcomando se reporta de forma canónica.
    #[test]
    fn subcommand_names_are_canonical() {
        let cli = Cli::try_parse_from(["modelforge", "bench", "model"]).expect("debe parsear");
        assert_eq!(cli.command.name(), "bench");
    }

    /// Un subcomando desconocido es un error.
    #[test]
    fn rejects_unknown_subcommand() {
        assert!(Cli::try_parse_from(["modelforge", "frobnicate"]).is_err());
    }

    /// El `--help` lista los subcomandos y los flags globales.
    #[test]
    fn help_lists_subcommands_and_flags() {
        let help = Cli::command().render_help().to_string();
        for name in [
            "import", "create", "test", "bench", "list", "info", "route", "pull",
        ] {
            assert!(help.contains(name), "la ayuda debe mencionar `{name}`");
        }
        for flag in ["--config", "--ollama", "--json", "--verbose"] {
            assert!(help.contains(flag), "la ayuda debe mencionar `{flag}`");
        }
    }
}
