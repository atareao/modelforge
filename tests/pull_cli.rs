//! Tests de integración del subcomando `pull` sobre el binario real.
//!
//! No necesitan Ollama: la referencia se valida antes de contactar con el
//! servidor y el fallo de conexión se prueba con un puerto cerrado.

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

#[test]
fn pull_with_valid_ref_fails_clearly_when_ollama_is_down() {
    let output = run(&[
        "--json",
        "--ollama",
        DEAD_OLLAMA,
        "pull",
        "hf.co/usuario/repo:Q4_K_M",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("Ollama no responde"),
        "debe indicar que Ollama no responde; stdout: {stdout}"
    );
}

/// Con `--json`, `stdout` debe ser **un único objeto JSON** y los eventos de etapa
/// (NDJSON) deben ir a `stderr`.
#[test]
fn pull_json_keeps_stdout_as_a_single_object_and_stages_on_stderr() {
    let output = run(&[
        "--json",
        "--ollama",
        DEAD_OLLAMA,
        "pull",
        "hf.co/usuario/repo:Q4_K_M",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "stdout debe ser un único objeto JSON; stdout: {stdout}"
    );
    let value: serde_json::Value =
        serde_json::from_str(lines[0]).expect("stdout debe ser JSON válido");
    assert_eq!(value["ok"], serde_json::json!(false), "stdout: {stdout}");
    assert!(
        !stdout.contains("\"stage\""),
        "los eventos de etapa no deben ir a stdout; stdout: {stdout}"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("\"stage\":\"solicitando la descarga a Ollama\""),
        "stderr debe llevar los eventos de etapa; stderr: {stderr}"
    );
}

#[test]
fn pull_with_invalid_ref_fails_with_clear_error() {
    for bad in [
        "usuario/repo:Q4_K_M",
        "hf.co/usuario/repo",
        "hf.co/usuario:Q4_K_M",
        "hf.co/usuario/repo:",
        "hf.co/:Q4_K_M",
    ] {
        let output = run(&["--json", "pull", bad]);

        assert!(
            !output.status.success(),
            "`{bad}` debe fallar; status: {}",
            output.status
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("\"ok\":false"), "`{bad}` stdout: {stdout}");
        assert!(
            stdout.contains("hf.co/<repo>:<quant>"),
            "`{bad}` debe indicar el formato esperado; stdout: {stdout}"
        );
    }
}
