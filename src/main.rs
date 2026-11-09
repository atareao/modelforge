//! `modelforge`: CLI para importar modelos GGUF desde HuggingFace, crearlos en
//! Ollama, validarlos y enrutarlos en el gateway.

mod bench;
mod cli;
mod create;
mod error;
mod format;
mod hf;
mod import;
mod list;
mod modelfile;
mod ollama;
mod quant;
mod route;
mod smoke;
mod table;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::cli::{
    BenchArgs, Cli, Command, CreateArgs, GlobalArgs, ImportArgs, InfoArgs, ListArgs, RouteArgs,
    TestArgs,
};
use crate::error::AppError;

fn main() -> ExitCode {
    let cli = Cli::parse();

    init_tracing(cli.global.verbose);
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
fn init_tracing(verbose: bool) {
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
            run_import(args, cli.global.json)?;
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
    }
}

/// Construye un runtime de Tokio para los subcomandos asíncronos.
fn runtime() -> Result<tokio::runtime::Runtime, AppError> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(AppError::from)
}

/// Ejecuta `import`: resuelve el repo, selecciona el quant y descarga reanudable.
fn run_import(args: &ImportArgs, json: bool) -> Result<(), AppError> {
    let repo = hf::RepoId::parse(&args.repo)?;
    let quant = args.quant.clone().ok_or(AppError::MissingQuant)?;
    let dest = args.dest.clone().unwrap_or_else(default_dest);

    let client = hf::HfHubClient::new()?;
    let options = import::ImportOptions {
        repo: &repo,
        quant: &quant,
        mmproj: args.mmproj,
        dest: &dest,
    };

    let runtime = runtime()?;
    let report = runtime.block_on(import::run(&client, &options, !json))?;

    emit_import(&report, json)?;
    Ok(())
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
    };

    let runtime = runtime()?;
    let result = runtime.block_on(create::run(&client, &options))?;

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
    println!("Modelfile:\n{}", result.modelfile);
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
