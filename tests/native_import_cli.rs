//! Tests de integración del *import nativo* (safetensors) sobre el binario real.
//!
//! No necesitan red ni Ollama: el import nativo valida la fuente y la
//! cuantización antes de contactar con Ollama, y el fallo de conexión se prueba
//! apuntando a un puerto cerrado.

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

/// Directorio con un `.safetensors` ficticio y metadatos, como un snapshot nativo.
fn model_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("model.safetensors"), b"SAFETENSORS").expect("escribir pesos");
    std::fs::write(dir.path().join("config.json"), b"{}").expect("escribir config");
    std::fs::write(dir.path().join("tokenizer.json"), b"{}").expect("escribir tokenizer");
    dir
}

#[test]
fn native_import_without_name_fails_with_clear_error() {
    let dir = model_dir();
    let output = run(&["--json", "import", dir.path().to_str().expect("utf-8")]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("--name"),
        "debe pedir `--name`; stdout: {stdout}"
    );
}

#[test]
fn native_import_directory_without_safetensors_fails() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("config.json"), b"{}").expect("escribir config");

    let output = run(&[
        "--json",
        "import",
        dir.path().to_str().expect("utf-8"),
        "--name",
        "mi-modelo",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("safetensors"),
        "debe indicar que faltan `.safetensors`; stdout: {stdout}"
    );
}

#[test]
fn native_import_rejects_unsupported_quantize_before_ollama() {
    let dir = model_dir();
    let output = run(&[
        "--json",
        "--ollama",
        DEAD_OLLAMA,
        "import",
        dir.path().to_str().expect("utf-8"),
        "--name",
        "mi-modelo",
        "--quantize",
        "NO_EXISTE",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("nvfp4"),
        "debe listar valores válidos de Ollama 0.40; stdout: {stdout}"
    );
    assert!(
        !stdout.contains("Ollama no responde"),
        "debe fallar antes de llamar a Ollama; stdout: {stdout}"
    );
}

/// `--quantize` solo vale con un directorio de safetensors; con un GGUF se rechaza.
#[test]
fn native_import_rejects_quantize_for_gguf() {
    let dir = tempfile::tempdir().expect("tempdir");
    let gguf = dir.path().join("model-Q6_K.gguf");
    std::fs::write(&gguf, b"GGUF").expect("escribir gguf");

    let output = run(&[
        "--json",
        "--ollama",
        DEAD_OLLAMA,
        "create",
        "mi-modelo",
        "--from",
        gguf.to_str().expect("utf-8"),
        "--quantize",
        "nvfp4",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("safetensors"),
        "debe explicar que solo vale para safetensors; stdout: {stdout}"
    );
}

/// El import nativo avisa si recibe flags que solo aplican al import de repos GGUF.
#[test]
fn native_import_rejects_gguf_only_flags() {
    let dir = model_dir();

    for (flag, value) in [
        ("--quant", Some("Q6_K")),
        ("--dest", Some("./models")),
        ("--mmproj", None),
    ] {
        let mut args = vec![
            "--json",
            "import",
            dir.path().to_str().expect("utf-8"),
            "--name",
            "mi-modelo",
            flag,
        ];
        if let Some(value) = value {
            args.push(value);
        }
        let output = run(&args);

        assert!(!output.status.success(), "{flag}: status {}", output.status);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains(flag.trim_start_matches('-')),
            "{flag}: stdout {stdout}"
        );
        assert!(stdout.contains("no acepta"), "{flag}: stdout {stdout}");
    }
}

/// Una fuente `hf.co/...` redirige a `pull`, incluso en `import`.
#[test]
fn import_hub_reference_suggests_pull() {
    let output = run(&["--json", "import", "hf.co/usuario/repo:nvfp4"]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("modelforge pull"),
        "debe sugerir `modelforge pull`; stdout: {stdout}"
    );
}

#[test]
fn native_import_fails_with_clear_error_when_ollama_is_down() {
    let dir = model_dir();
    let output = run(&[
        "--json",
        "--ollama",
        DEAD_OLLAMA,
        "import",
        dir.path().to_str().expect("utf-8"),
        "--name",
        "mi-modelo",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("Ollama no responde"),
        "debe indicar que Ollama no responde; stdout: {stdout}"
    );
}

/// Con `--json`, `stdout` debe ser **un único objeto JSON** (el sobre final) y
/// `stderr` no debe contener **ninguna** línea que no sea un evento NDJSON de
/// etapa. Con Ollama caído **no** se anuncia ninguna etapa: `conversión interna`
/// solo se emite tras validar la conexión (fix de la re-auditoría).
#[test]
fn json_keeps_stdout_as_a_single_object_and_stderr_as_pure_ndjson() {
    let dir = model_dir();
    let output = run(&[
        "--json",
        "--ollama",
        DEAD_OLLAMA,
        "import",
        dir.path().to_str().expect("utf-8"),
        "--name",
        "mi-modelo",
        "--quantize",
        "nvfp4",
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

    // `stderr` en modo `--json`: solo NDJSON de etapa, ninguna línea de logging humano.
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines().filter(|line| !line.trim().is_empty()) {
        let event: serde_json::Value = serde_json::from_str(line)
            .unwrap_or_else(|err| panic!("stderr no es NDJSON puro ({err}): {line}"));
        assert!(
            event.get("stage").is_some(),
            "cada línea de stderr debe ser un evento de etapa: {line}"
        );
    }
    // Con Ollama caído no se anuncia `conversión interna`: se emite tras conectar.
    assert!(
        !stderr.contains("conversión interna"),
        "no debe anunciar etapas antes de conectar con Ollama; stderr: {stderr}"
    );
}
