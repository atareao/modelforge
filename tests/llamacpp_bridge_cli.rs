//! Tests de integración del puente a **llama.cpp** (`--backend llama.cpp`).
//!
//! No invocan llama.cpp real: fuerzan `PATH=""` para provocar la detección de
//! herramientas ausentes de forma determinista, y validan el enrutado, la
//! validación de quants y el contrato `--json` (stdout = un objeto, stderr =
//! NDJSON puro). El flujo real va `#[ignore]` en `llamacpp_bridge_real.rs`.

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

/// Ejecuta `modelforge` con un `PATH` controlado (para simular tools ausentes).
fn run_with_path(args: &[&str], path: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_modelforge"))
        .args(args)
        .env("PATH", path)
        .output()
        .expect("ejecutar modelforge")
}

/// Directorio con un `.safetensors` ficticio y metadatos, como un snapshot nativo.
fn model_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("model.safetensors"), b"SAFETENSORS").expect("pesos");
    std::fs::write(dir.path().join("config.json"), b"{}").expect("config");
    dir
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn llamacpp_backend_reports_missing_tools_with_install_hint() {
    let dir = model_dir();
    let output = run_with_path(
        &[
            "--json",
            "import",
            dir.path().to_str().expect("utf-8"),
            "--backend",
            "llama.cpp",
            "--name",
            "mi-modelo",
        ],
        "",
    );

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = stdout(&output);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("convert_hf_to_gguf.py") || stdout.contains("llama.cpp"),
        "debe nombrar la herramienta ausente; stdout: {stdout}"
    );
    assert!(
        stdout.contains("PATH"),
        "debe indicar el `PATH`; stdout: {stdout}"
    );
}

#[test]
fn llamacpp_backend_rejects_invalid_quant_before_llamacpp_or_ollama() {
    let dir = model_dir();
    let output = run_with_path(
        &[
            "--json",
            "--ollama",
            DEAD_OLLAMA,
            "import",
            dir.path().to_str().expect("utf-8"),
            "--backend",
            "llama.cpp",
            "--name",
            "mi-modelo",
            "--quantize",
            "NO_EXISTE",
        ],
        "",
    );

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = stdout(&output);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("NO_EXISTE") && stdout.contains("Q4_K_M"),
        "debe listar los quants válidos de llama.cpp; stdout: {stdout}"
    );
    assert!(
        !stdout.contains("Ollama no responde"),
        "debe fallar antes de contactar con Ollama; stdout: {stdout}"
    );
}

#[test]
fn llamacpp_backend_rejects_gguf_repo() {
    let output = run(&[
        "--json",
        "import",
        "org/modelo",
        "--backend",
        "llama.cpp",
        "--quant",
        "Q4_K_M",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = stdout(&output);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("llama.cpp") && stdout.contains("safetensors"),
        "debe explicar que solo aplica a safetensors; stdout: {stdout}"
    );
}

#[test]
fn llamacpp_backend_rejects_hub_reference() {
    let output = run(&[
        "--json",
        "import",
        "hf.co/usuario/repo:Q4_K_M",
        "--backend",
        "llama.cpp",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = stdout(&output);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("llama.cpp"),
        "debe mencionar el backend; stdout: {stdout}"
    );
}

/// Con `--json`, `stdout` es un único objeto JSON y `stderr` solo NDJSON (vacío
/// aquí, porque el fallo ocurre antes de anunciar la primera etapa).
#[test]
fn llamacpp_backend_json_keeps_stdout_single_and_stderr_ndjson() {
    let dir = model_dir();
    let output = run_with_path(
        &[
            "--json",
            "import",
            dir.path().to_str().expect("utf-8"),
            "--backend",
            "llama.cpp",
            "--name",
            "mi-modelo",
        ],
        "",
    );

    assert!(!output.status.success(), "status: {}", output.status);

    let stdout = stdout(&output);
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(lines.len(), 1, "stdout debe ser un objeto JSON: {stdout}");
    let value: serde_json::Value =
        serde_json::from_str(lines[0]).expect("stdout debe ser JSON válido");
    assert_eq!(value["ok"], serde_json::json!(false), "stdout: {stdout}");

    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines().filter(|line| !line.trim().is_empty()) {
        let event: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|err| panic!("stderr no es NDJSON puro ({err}): {line}"));
        assert!(
            event.get("stage").is_some(),
            "cada línea de stderr debe ser un evento de etapa: {line}"
        );
    }
}

/// Escribe un script ejecutable (shebang `sh`) en `path`.
#[cfg(unix)]
fn write_executable(path: &std::path::Path, contents: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, contents).expect("escribir script");
    let mut perms = std::fs::metadata(path).expect("metadata").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms).expect("chmod +x");
}

/// Progreso NDJSON de **éxito**: con herramientas fake que salen 0 y un Ollama
/// muerto, el CLI emite `conversión → cuantización → creación` por `stderr` (NDJSON)
/// y un único objeto JSON por `stdout`.
#[cfg(unix)]
#[test]
fn llamacpp_backend_emits_success_stage_events_in_order() {
    let fake = tempfile::tempdir().expect("tools dir");
    let model = model_dir();

    // `python3` fake: escribe el `--outfile` indicado y sale 0.
    write_executable(
        &fake.path().join("python3"),
        "#!/bin/sh\nout=''\nprev=''\nfor a in \"$@\"; do\n  if [ \"$prev\" = '--outfile' ]; then out=\"$a\"; fi\n  prev=\"$a\"\ndone\nprintf 'GGUF-F16' > \"$out\"\nexit 0\n",
    );
    // El script de conversión solo tiene que existir: lo ejecuta el `python3` fake.
    std::fs::write(fake.path().join("convert_hf_to_gguf.py"), b"# fake\n").expect("script");
    // `llama-quantize` fake: escribe el fichero de salida (`$2`) y sale 0.
    write_executable(
        &fake.path().join("llama-quantize"),
        "#!/bin/sh\nprintf 'GGUF-QUANT' > \"$2\"\nexit 0\n",
    );

    let output = run_with_path(
        &[
            "--json",
            "--ollama",
            DEAD_OLLAMA,
            "import",
            model.path().to_str().expect("utf-8"),
            "--backend",
            "llama.cpp",
            "--name",
            "mi-modelo",
            "--quantize",
            "Q4_K_M",
        ],
        fake.path().to_str().expect("utf-8"),
    );

    // Las etapas previas se emiten; la creación falla al no responder Ollama.
    assert!(!output.status.success(), "status: {}", output.status);

    // `stdout`: un único objeto JSON (el sobre de error).
    let stdout = stdout(&output);
    let lines: Vec<&str> = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    assert_eq!(
        lines.len(),
        1,
        "stdout debe ser un único objeto JSON: {stdout}"
    );
    let value: serde_json::Value = serde_json::from_str(lines[0]).expect("stdout JSON válido");
    assert_eq!(value["ok"], serde_json::json!(false), "stdout: {stdout}");

    // `stderr`: etapas NDJSON en orden, con el quant canonicalizado en `cuantización`.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let events: Vec<serde_json::Value> = stderr
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            serde_json::from_str(line)
                .unwrap_or_else(|err| panic!("stderr no es NDJSON puro ({err}): {line}"))
        })
        .collect();

    assert!(events.len() >= 3, "faltan etapas en stderr: {stderr}");
    assert_eq!(
        events[0]["stage"],
        serde_json::json!("conversión"),
        "{stderr}"
    );
    assert_eq!(
        events[1]["stage"],
        serde_json::json!("cuantización"),
        "{stderr}"
    );
    assert_eq!(events[1]["value"], serde_json::json!("Q4_K_M"), "{stderr}");
    assert_eq!(
        events[2]["stage"],
        serde_json::json!("creación"),
        "{stderr}"
    );
}
