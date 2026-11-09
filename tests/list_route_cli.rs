//! Tests de integración de `list`, `info` y `route` sobre el binario real.
//!
//! Los de `list`/`info` apuntan a un puerto cerrado (Ollama caído) y comprueban el
//! error claro en JSON y el código de salida. Los de `route` operan sobre un
//! `gateway.yaml` temporal vía `--config`, sin tocar el fichero real del usuario.

use std::path::Path;
use std::process::{Command, Output};

/// Un puerto local cerrado que provoca un fallo de conexión inmediato.
const DEAD_OLLAMA: &str = "http://127.0.0.1:1";

/// Ejecuta `modelforge` con los argumentos dados.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_modelforge"))
        .args(args)
        .output()
        .expect("ejecutar modelforge")
}

/// Lee un fichero como texto.
fn read(path: &Path) -> String {
    std::fs::read_to_string(path).expect("leer fichero")
}

#[test]
fn list_fails_with_clear_error_when_ollama_is_down() {
    let output = run(&["--ollama", DEAD_OLLAMA, "--json", "list"]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(stdout.contains("Ollama no responde"), "stdout: {stdout}");
}

#[test]
fn info_fails_with_clear_error_when_ollama_is_down() {
    let output = run(&["--ollama", DEAD_OLLAMA, "--json", "info", "un-modelo"]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(stdout.contains("Ollama no responde"), "stdout: {stdout}");
}

#[test]
fn route_adds_new_route_and_writes_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("gateway.yaml");
    let config = config.to_str().expect("utf-8");

    let output = run(&[
        "--config", config, "--json", "route", "mi-ruta", "--model", "qwen3:8b",
    ]);

    assert!(output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":true"), "stdout: {stdout}");
    assert!(stdout.contains("\"status\":\"added\""), "stdout: {stdout}");

    let written = read(Path::new(config));
    assert!(written.contains("mi-ruta"), "yaml: {written}");
    assert!(written.contains("qwen3:8b"), "yaml: {written}");
    assert!(
        written.contains("api_key_env: OPENROUTER_API_KEY"),
        "debe incluir el backend de respaldo; yaml: {written}"
    );
    // El fichero no existía: no debe haber backup.
    assert!(!dir.path().join("gateway.yaml.bak").exists());
}

#[test]
fn route_is_idempotent_and_backs_up_before_rewriting() {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = dir.path().join("gateway.yaml");
    let config = config.to_str().expect("utf-8");

    // Primera ruta: crea el fichero (sin backup previo).
    run(&["--config", config, "route", "primera", "--model", "m1"]);

    // Misma ruta otra vez: no duplica, no crea backup ni cambia el fichero.
    let output = run(&[
        "--config", config, "--json", "route", "primera", "--model", "m2",
    ]);
    assert!(output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"status\":\"already_exists\""),
        "stdout: {stdout}"
    );
    assert!(
        !dir.path().join("gateway.yaml.bak").exists(),
        "sin cambios no debe haber backup"
    );
    let after_duplicate = read(Path::new(config));
    assert!(
        after_duplicate.contains("primera"),
        "yaml: {after_duplicate}"
    );
    assert!(
        !after_duplicate.contains("m2"),
        "no debe duplicar: {after_duplicate}"
    );

    // Ruta nueva sobre un fichero existente: sí crea backup del contenido anterior.
    let output = run(&[
        "--config", config, "--json", "route", "segunda", "--model", "m3",
    ]);
    assert!(output.status.success(), "status: {}", output.status);
    let backup = dir.path().join("gateway.yaml.bak");
    assert!(backup.exists(), "debe crear backup");
    assert_eq!(
        read(&backup),
        after_duplicate,
        "el backup es el contenido previo"
    );
    let rewritten = read(Path::new(config));
    assert!(rewritten.contains("segunda"), "yaml: {rewritten}");
    assert!(
        rewritten.contains("primera"),
        "conserva las anteriores: {rewritten}"
    );
}

#[test]
fn route_rejects_invalid_yaml_without_touching_the_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("gateway.yaml");
    std::fs::write(&path, "route: [no válido").expect("escribir");

    let output = run(&[
        "--config",
        path.to_str().expect("utf-8"),
        "--json",
        "route",
        "x",
        "--model",
        "m",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(stdout.contains("no es válido"), "stdout: {stdout}");
    assert!(!dir.path().join("gateway.yaml.bak").exists());
    assert_eq!(read(&path), "route: [no válido");
}

#[test]
fn route_without_model_fails_clearly() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("gateway.yaml");

    let output = run(&[
        "--config",
        path.to_str().expect("utf-8"),
        "--json",
        "route",
        "x",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(stdout.contains("falta `--model"), "stdout: {stdout}");
    assert!(!path.exists(), "no debe escribir nada");
}

#[test]
fn route_dry_run_does_not_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("gateway.yaml");

    let output = run(&[
        "--config",
        path.to_str().expect("utf-8"),
        "route",
        "x",
        "--model",
        "m",
        "--dry-run",
    ]);

    assert!(output.status.success(), "status: {}", output.status);
    assert!(!path.exists(), "dry-run no debe crear el fichero");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("se añadiría"), "stdout: {stdout}");
}
