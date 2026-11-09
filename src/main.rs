//! `modelforge`: CLI para importar modelos GGUF desde HuggingFace, crearlos en
//! Ollama, validarlos y enrutarlos en el gateway.

mod bench;
mod cli;
mod convert;
mod create;
mod error;
mod format;
mod hf;
mod import;
mod list;
mod modelfile;
mod ollama;
mod pull;
mod quant;
mod route;
mod smoke;
mod source;
mod table;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::cli::{
    Backend, BenchArgs, Cli, Command, CreateArgs, GlobalArgs, ImportArgs, InfoArgs, ListArgs,
    PullArgs, RouteArgs, TestArgs,
};
use crate::error::AppError;
use crate::source::SourceKind;

fn main() -> ExitCode {
    let cli = Cli::parse();

    init_tracing(cli.global.verbose, cli.global.json);
    tracing::debug!(command = cli.command.name(), "ejecutando subcomando");

    match run(&cli) {
        Ok(code) => code,
        Err(err) => {
            report(&err, cli.global.json);
            ExitCode::FAILURE
        }
    }
}

/// Inicializa el subsistema de logging en función del nivel de detalle.
///
/// El nivel también puede fijarse con la variable `RUST_LOG`.
///
/// Con `--json`, `stderr` debe contener **solo** las líneas NDJSON de etapa
/// ([`emit_stage`]), así que se **silencia** el logging humano del subscriber para
/// no mezclar formatos: `stderr` queda reservado al contrato NDJSON.
fn init_tracing(verbose: bool, json: bool) {
    if json {
        return;
    }

    let filter = if verbose { "debug" } else { "info" };
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    // `try_init` evita el pánico si el suscriptor ya está instalado. Los logs van
    // a `stderr` para no contaminar la salida JSON de `stdout`.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .try_init();
}

/// Ejecuta el subcomando seleccionado.
///
/// Devuelve el código de salida del proceso: `SUCCESS` salvo error o, en `test`,
/// cuando alguna prueba de humo falla.
fn run(cli: &Cli) -> Result<ExitCode, AppError> {
    match &cli.command {
        Command::Import(args) => {
            run_import(args, &cli.global)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Create(args) => {
            run_create(args, &cli.global)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Test(args) => run_test(args, &cli.global),
        Command::Bench(args) => {
            run_bench(args, &cli.global)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::List(args) => {
            run_list(args, &cli.global)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Info(args) => {
            run_info(args, &cli.global)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Route(args) => {
            run_route(args, &cli.global)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Pull(args) => {
            run_pull(args, &cli.global)?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Construye un runtime de Tokio para los subcomandos asíncronos.
fn runtime() -> Result<tokio::runtime::Runtime, AppError> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(AppError::from)
}

/// Ejecuta `import`: descarga de un repo GGUF o import nativo de safetensors.
///
/// La fuente decide el flujo:
/// - repo de HuggingFace (`org/model`) → descarga reanudable del GGUF;
/// - directorio local con `.safetensors` → creación nativa del modelo en Ollama;
/// - referencia del hub (`hf.co/...`) → se redirige al subcomando `pull`.
fn run_import(args: &ImportArgs, global: &GlobalArgs) -> Result<(), AppError> {
    match SourceKind::detect(&args.source) {
        SourceKind::GgufRepo(repo) => {
            reject_llamacpp_backend(args)?;
            reject_flags_for_gguf_import(args)?;
            import_gguf(&repo, args, global.json)
        }
        SourceKind::SafetensorsDir(dir) => {
            reject_flags_for_native_import(args)?;
            match args.backend {
                Backend::LlamaCpp => import_llamacpp(&dir, args, global),
                Backend::Ollama => import_native(&dir, args, global),
            }
        }
        SourceKind::HubRef(input) => {
            reject_llamacpp_backend(args)?;
            Err(AppError::UsePull { input })
        }
    }
}

/// Rechaza `--backend llama.cpp` para fuentes que no son un directorio de safetensors.
///
/// El backend de llama.cpp solo convierte snapshots locales de safetensors; un repo
/// GGUF o una referencia del hub ya son GGUF, así que el flag no aplica.
///
/// # Errors
///
/// Devuelve [`AppError::LlamaCppUnsupportedSource`] si se usó `--backend llama.cpp`.
fn reject_llamacpp_backend(args: &ImportArgs) -> Result<(), AppError> {
    if args.backend == Backend::LlamaCpp {
        Err(AppError::LlamaCppUnsupportedSource {
            input: args.source.clone(),
        })
    } else {
        Ok(())
    }
}

/// Rechaza los flags que solo tienen sentido en el import nativo de safetensors.
///
/// El import de un repo GGUF descarga ficheros ya convertidos y no crea el modelo
/// en Ollama, así que `--name` y `--quantize` no aplican; ignorarlos en silencio
/// confundiría al usuario (o, peor, iría a la red sin validar).
///
/// # Errors
///
/// Devuelve [`AppError::GgufImportUnsupportedFlags`] si se indicó alguno.
fn reject_flags_for_gguf_import(args: &ImportArgs) -> Result<(), AppError> {
    let mut flags = Vec::new();
    if args.name.is_some() {
        flags.push("--name".to_string());
    }
    if args.quantize.is_some() {
        flags.push("--quantize".to_string());
    }

    if flags.is_empty() {
        Ok(())
    } else {
        Err(AppError::GgufImportUnsupportedFlags { flags })
    }
}

/// Rechaza los flags que solo tienen sentido en el import de repos GGUF.
///
/// El import nativo parte de un snapshot local ya descargado, así que `--quant`,
/// `--mmproj` y `--dest` no aplican; ignorarlos en silencio confundiría al usuario.
///
/// # Errors
///
/// Devuelve [`AppError::NativeImportUnsupportedFlags`] si se indicó alguno.
fn reject_flags_for_native_import(args: &ImportArgs) -> Result<(), AppError> {
    let mut flags = Vec::new();
    if args.quant.is_some() {
        flags.push("--quant".to_string());
    }
    if args.mmproj {
        flags.push("--mmproj".to_string());
    }
    if args.dest.is_some() {
        flags.push("--dest".to_string());
    }

    if flags.is_empty() {
        Ok(())
    } else {
        Err(AppError::NativeImportUnsupportedFlags { flags })
    }
}

/// Descarga el GGUF (y su `mmproj`) de un repo de HuggingFace.
fn import_gguf(repo_input: &str, args: &ImportArgs, json: bool) -> Result<(), AppError> {
    let repo = hf::RepoId::parse(repo_input)?;
    let quant = args.quant.clone().ok_or(AppError::MissingQuant)?;
    let dest = args.dest.clone().unwrap_or_else(default_dest);

    let client = hf::HfHubClient::new()?;
    let options = import::ImportOptions {
        repo: &repo,
        quant: &quant,
        mmproj: args.mmproj,
        dest: &dest,
    };

    emit_stage(json, "descarga");

    let runtime = runtime()?;
    let report = runtime.block_on(import::run(&client, &options, !json))?;

    emit_import(&report, json)?;
    Ok(())
}

/// Importa un modelo nativo desde un directorio local con `.safetensors`.
///
/// Genera el Modelfile `FROM <directorio>`, sube los ficheros como blobs y crea
/// el modelo en Ollama, aplicando `--quantize` si se indicó.
fn import_native(
    dir: &std::path::Path,
    args: &ImportArgs,
    global: &GlobalArgs,
) -> Result<(), AppError> {
    let name = args.name.clone().ok_or(AppError::MissingName)?;
    let client = ollama::OllamaHttpClient::new(global.ollama.clone())?;
    let options = create::CreateOptions {
        name: &name,
        from: dir,
        mmproj: None,
        ctx: None,
        renderer: None,
        parser: None,
        quantize: args.quantize.as_deref(),
    };
    let progress = CliProgress { json: global.json };

    let runtime = runtime()?;
    let report = runtime.block_on(create::run(&client, &options, &progress))?;

    emit_create(&report, global.json)?;
    Ok(())
}

/// Importa un directorio de safetensors convirtiéndolo a GGUF con llama.cpp.
///
/// Convierte el snapshot a un GGUF FP16 en un directorio temporal, lo cuantiza
/// opcionalmente con `llama-quantize` y crea el modelo en Ollama a partir del GGUF
/// resultante. Requiere `convert_hf_to_gguf.py` y `llama-quantize` en `PATH`.
fn import_llamacpp(
    dir: &std::path::Path,
    args: &ImportArgs,
    global: &GlobalArgs,
) -> Result<(), AppError> {
    let name = args.name.clone().ok_or(AppError::MissingName)?;
    let client = ollama::OllamaHttpClient::new(global.ollama.clone())?;
    let converter = convert::LlamaCppConverter::new();
    let options = convert::ConvertOptions {
        name: &name,
        source_dir: dir,
        quantize: args.quantize.as_deref(),
        tmp_root: None,
    };
    let progress = CliProgress { json: global.json };

    let runtime = runtime()?;
    let report = runtime.block_on(convert::run(&client, &converter, &options, &progress))?;

    emit_create(&report, global.json)?;
    Ok(())
}

/// Adaptador de [`create::Progress`] que vuelca las etapas al mecanismo de [`emit_stage`].
struct CliProgress {
    json: bool,
}

impl create::Progress for CliProgress {
    fn stage(&self, stage: create::Stage) {
        emit_stage(self.json, stage.label());
    }

    fn stage_with_value(&self, stage: create::Stage, value: &str) {
        emit_stage_with_value(self.json, stage.label(), value);
    }
}

/// Ejecuta `pull`: descarga un GGUF del hub delegando en Ollama.
fn run_pull(args: &PullArgs, global: &GlobalArgs) -> Result<(), AppError> {
    let client = ollama::OllamaHttpClient::new(global.ollama.clone())?;
    let options = pull::PullOptions {
        reference: &args.reference,
    };

    emit_stage(global.json, "solicitando la descarga a Ollama");
    let runtime = runtime()?;
    let report = runtime.block_on(pull::run(&client, &options))?;

    emit_pull(&report, global.json)?;
    Ok(())
}

/// Presenta el resultado de `pull` en JSON o en texto.
///
/// # Errors
///
/// Devuelve [`AppError::Json`] si el reporte no se puede serializar.
fn emit_pull(report: &pull::PullReport, json: bool) -> Result<(), AppError> {
    if json {
        let data = serde_json::to_value(report)?;
        println!("{}", serde_json::json!({ "ok": true, "data": data }));
        return Ok(());
    }

    println!("modelo `{}` descargado en Ollama", report.reference);
    Ok(())
}

/// Emite un evento de progreso por etapa.
///
/// Va **siempre a `stderr`**: en modo texto por `tracing` y en modo `--json` como
/// una línea NDJSON `{"stage":...}`. En `--json` el logging humano del subscriber
/// está silenciado ([`init_tracing`]), de modo que `stderr` contiene **solo** estas
/// líneas y `stdout` conserva el contrato de un único objeto JSON (el sobre final).
fn emit_stage(json: bool, stage: &str) {
    if json {
        eprintln!("{}", serde_json::json!({ "stage": stage }));
    } else {
        tracing::info!(stage, "etapa en curso");
    }
}

/// Emite un evento de progreso con un valor asociado (p. ej. el `quantize`).
///
/// Igual que [`emit_stage`], escribe en `stderr` en ambos modos.
fn emit_stage_with_value(json: bool, stage: &str, value: &str) {
    if json {
        eprintln!("{}", serde_json::json!({ "stage": stage, "value": value }));
    } else {
        tracing::info!(stage, value, "etapa en curso");
    }
}

/// Directorio de destino por defecto para `import`.
fn default_dest() -> PathBuf {
    PathBuf::from("models")
}

/// Presenta el resultado de `import` en JSON o en texto.
///
/// # Errors
///
/// Devuelve [`AppError::Json`] si el reporte no se puede serializar.
fn emit_import(import_report: &import::ImportReport, json: bool) -> Result<(), AppError> {
    if json {
        let data = serde_json::to_value(import_report)?;
        println!("{}", serde_json::json!({ "ok": true, "data": data }));
        return Ok(());
    }

    for file in &import_report.files {
        let status = if file.cached {
            "ya estaba"
        } else {
            "descargado"
        };
        println!(
            "{} — {} ({status})",
            file.path,
            format::format_bytes(file.size)
        );
    }
    Ok(())
}

/// Ejecuta `create`: genera el Modelfile y crea el modelo en Ollama.
fn run_create(args: &CreateArgs, global: &GlobalArgs) -> Result<(), AppError> {
    let from = args.from.clone().ok_or(AppError::MissingFrom)?;
    let client = ollama::OllamaHttpClient::new(global.ollama.clone())?;
    let options = create::CreateOptions {
        name: &args.name,
        from: &from,
        mmproj: args.mmproj.as_deref(),
        ctx: args.ctx,
        renderer: args.renderer.as_deref(),
        parser: args.parser.as_deref(),
        quantize: args.quantize.as_deref(),
    };
    let progress = CliProgress { json: global.json };

    let runtime = runtime()?;
    let result = runtime.block_on(create::run(&client, &options, &progress))?;

    emit_create(&result, global.json)?;
    Ok(())
}

/// Presenta el resultado de `create` en JSON o en texto.
///
/// # Errors
///
/// Devuelve [`AppError::Json`] si el reporte no se puede serializar.
fn emit_create(result: &create::CreateReport, json: bool) -> Result<(), AppError> {
    if json {
        let data = serde_json::to_value(result)?;
        println!("{}", serde_json::json!({ "ok": true, "data": data }));
        return Ok(());
    }

    println!("modelo `{}` creado en Ollama", result.model);
    if let Some(quantize) = &result.quantize {
        println!("quantize: {quantize}");
    }
    let vision = if result.info.has_vision() {
        "sí"
    } else {
        "no"
    };
    println!("vision: {vision}");
    match result.info.num_ctx() {
        Some(ctx) => println!("num_ctx: {ctx}"),
        None => println!("num_ctx: (no reportado)"),
    }
    // El flujo de llama.cpp omite el Modelfile (apuntaría a un GGUF temporal ya
    // eliminado); solo se imprime cuando hay uno reproducible.
    if !result.modelfile.is_empty() {
        println!("Modelfile:\n{}", result.modelfile);
    }
    Ok(())
}

/// Ejecuta `test`: pruebas de humo contra el modelo indicado.
fn run_test(args: &TestArgs, global: &GlobalArgs) -> Result<ExitCode, AppError> {
    let client = ollama::OllamaHttpClient::new(global.ollama.clone())?;
    let options = smoke::SmokeOptions {
        model: &args.name,
        tools: args.tools,
        vision: args.vision,
        image: args.image.as_deref(),
        num_predict: args.num_predict,
    };

    let runtime = runtime()?;
    let report = runtime.block_on(smoke::run(&client, &options))?;

    emit_test(&report, global.json)?;
    Ok(if report.passed {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

/// Presenta el resultado de `test` en JSON o en texto.
///
/// # Errors
///
/// Devuelve [`AppError::Json`] si el reporte no se puede serializar.
fn emit_test(outcome: &smoke::SmokeReport, json: bool) -> Result<(), AppError> {
    if json {
        let data = serde_json::to_value(outcome)?;
        println!("{}", serde_json::json!({ "ok": true, "data": data }));
        return Ok(());
    }

    for check in &outcome.checks {
        let mark = if check.ok { "OK   " } else { "FALLA" };
        println!("[{mark}] {}: {}", check.mode, check.detail);
    }
    let verdict = if outcome.passed {
        "todas las pruebas pasaron"
    } else {
        "algunas pruebas fallaron"
    };
    println!("modelo `{}`: {verdict}", outcome.model);
    Ok(())
}

/// Ejecuta `bench`: mide el rendimiento de uno o dos modelos.
fn run_bench(args: &BenchArgs, global: &GlobalArgs) -> Result<(), AppError> {
    let client = ollama::OllamaHttpClient::new(global.ollama.clone())?;
    let prompt = args
        .prompt
        .clone()
        .unwrap_or_else(|| bench::DEFAULT_PROMPT.to_string());
    let options = bench::BenchOptions {
        model: &args.name,
        compare: args.compare.as_deref(),
        prompt: &prompt,
        num_predict: args.num_predict,
    };

    let runtime = runtime()?;
    let report = runtime.block_on(bench::run(&client, &options))?;

    emit_bench(&report, global.json)?;
    Ok(())
}

/// Presenta el resultado de `bench` en JSON o como tabla comparativa.
///
/// # Errors
///
/// Devuelve [`AppError::Json`] si el reporte no se puede serializar.
fn emit_bench(outcome: &bench::BenchReport, json: bool) -> Result<(), AppError> {
    if json {
        let data = serde_json::to_value(outcome)?;
        println!("{}", serde_json::json!({ "ok": true, "data": data }));
        return Ok(());
    }

    println!("prompt: {}", outcome.prompt);
    println!("{}", bench::comparison_table(&outcome.results));
    Ok(())
}

/// Ejecuta `list`: modelos instalados con su contexto y VRAM.
fn run_list(_args: &ListArgs, global: &GlobalArgs) -> Result<(), AppError> {
    let client = ollama::OllamaHttpClient::new(global.ollama.clone())?;
    let runtime = runtime()?;
    let report = runtime.block_on(list::run(&client))?;

    emit_list(&report, global.json)?;
    Ok(())
}

/// Presenta el listado en JSON o como tabla de texto.
///
/// # Errors
///
/// Devuelve [`AppError::Json`] si el listado no se puede serializar.
fn emit_list(listing: &list::ListReport, json: bool) -> Result<(), AppError> {
    if json {
        let data = serde_json::to_value(listing)?;
        println!("{}", serde_json::json!({ "ok": true, "data": data }));
        return Ok(());
    }

    println!("{}", list::table(listing));
    Ok(())
}

/// Ejecuta `info`: contexto, VRAM y processor de un modelo.
fn run_info(args: &InfoArgs, global: &GlobalArgs) -> Result<(), AppError> {
    let client = ollama::OllamaHttpClient::new(global.ollama.clone())?;
    let runtime = runtime()?;
    let details = runtime.block_on(list::run_info(&client, &args.name))?;

    emit_info(&details, global.json)?;
    Ok(())
}

/// Presenta los detalles de un modelo en JSON o en texto.
///
/// # Errors
///
/// Devuelve [`AppError::Json`] si los detalles no se pueden serializar.
fn emit_info(details: &list::ModelDetails, json: bool) -> Result<(), AppError> {
    if json {
        let data = serde_json::to_value(details)?;
        println!("{}", serde_json::json!({ "ok": true, "data": data }));
        return Ok(());
    }

    println!("model: {}", details.name);
    println!("context: {}", details.context);
    println!("size_vram: {}", format::format_bytes(details.size_vram));
    println!("processor: {}", details.processor);
    let capabilities = if details.capabilities.is_empty() {
        "(ninguna)".to_string()
    } else {
        details.capabilities.join(", ")
    };
    println!("capabilities: {capabilities}");
    Ok(())
}

/// Ejecuta `route`: añade una ruta de forma idempotente al `gateway.yaml`.
fn run_route(args: &RouteArgs, global: &GlobalArgs) -> Result<(), AppError> {
    let model = args.model.clone().ok_or_else(|| AppError::MissingModel {
        name: args.name.clone(),
    })?;
    let path = match &global.config {
        Some(path) => path.clone(),
        None => route::default_path()?,
    };

    let outcome = route::run(&path, &args.name, &model, args.dry_run)?;

    emit_route(&outcome, global.json, args.dry_run)?;
    Ok(())
}

/// Presenta el resultado de `route` en JSON o en texto.
///
/// # Errors
///
/// Devuelve [`AppError::Json`] si el resultado no se puede serializar.
fn emit_route(outcome: &route::RouteOutcome, json: bool, dry_run: bool) -> Result<(), AppError> {
    if json {
        let data = serde_json::to_value(outcome)?;
        println!("{}", serde_json::json!({ "ok": true, "data": data }));
        return Ok(());
    }

    let path = outcome.path.display();
    match outcome.status {
        route::RouteStatus::AlreadyExists => {
            println!(
                "la ruta `{}` ya existe en `{path}`; no se ha duplicado",
                outcome.name
            );
        }
        route::RouteStatus::Added => {
            if dry_run {
                println!(
                    "se añadiría la ruta `{}` (modelo `{}`) a `{path}`",
                    outcome.name, outcome.model
                );
                print!("{}", outcome.yaml);
            } else {
                println!(
                    "ruta `{}` añadida a `{path}` (modelo `{}`)",
                    outcome.name, outcome.model
                );
            }
        }
    }

    if let Some(backup) = &outcome.backup {
        println!("backup: {}", backup.display());
    }
    Ok(())
}

/// Presenta un error por la salida estándar (JSON) o por la de errores.
fn report(err: &AppError, json: bool) {
    if json {
        println!("{}", err.to_json());
    } else {
        eprintln!("error: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ImportArgs` de un directorio de safetensors con `--backend llama.cpp`.
    fn llamacpp_import() -> ImportArgs {
        ImportArgs {
            source: "./modelo".to_string(),
            backend: Backend::LlamaCpp,
            quant: None,
            name: Some("mi-modelo".to_string()),
            quantize: None,
            mmproj: false,
            dest: None,
        }
    }

    /// Aplica `set_flag` sobre un `ImportArgs` de llama.cpp y verifica que el flag se
    /// rechaza, nombrando `expected`.
    fn assert_flag_rejected(set_flag: impl FnOnce(&mut ImportArgs), expected: &str) {
        let mut args = llamacpp_import();
        set_flag(&mut args);

        let err = reject_flags_for_native_import(&args).expect_err("debe rechazar el flag");

        match err {
            AppError::NativeImportUnsupportedFlags { flags } => {
                assert!(
                    flags.iter().any(|flag| flag == expected),
                    "el error debe listar `{expected}`: {flags:?}"
                );
            }
            other => panic!("se esperaba NativeImportUnsupportedFlags, se obtuvo {other:?}"),
        }
    }

    /// `--backend llama.cpp` rechaza los flags que solo valen para el import de repos
    /// GGUF (`--quant`, `--dest`, `--mmproj`), en lugar de ignorarlos en silencio.
    #[test]
    fn llamacpp_backend_rejects_gguf_only_flags() {
        assert_flag_rejected(|args| args.quant = Some("Q4_K_M".to_string()), "--quant");
        assert_flag_rejected(|args| args.mmproj = true, "--mmproj");
        assert_flag_rejected(
            |args| args.dest = Some(PathBuf::from("/tmp/salida")),
            "--dest",
        );
    }

    /// Sin flags ajenos, el import nativo de safetensors no se rechaza.
    #[test]
    fn llamacpp_backend_accepts_pure_source() {
        let args = llamacpp_import();
        assert!(reject_flags_for_native_import(&args).is_ok());
    }
}
