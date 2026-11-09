//! Test de integración del subcomando `create` cuando Ollama no está disponible.
//!
//! Ejecuta el binario real apuntando a un puerto cerrado y comprueba que falla con
//! un error claro y código de salida distinto de 0.

use std::process::Command;

/// Un puerto local cerrado que provoca un fallo de conexión inmediato.
const DEAD_OLLAMA: &str = "http://127.0.0.1:1";

#[test]
fn create_fails_with_clear_error_when_ollama_is_down() {
    let dir = tempfile::tempdir().expect("tempdir");
    let gguf = dir.path().join("model-Q6_K.gguf");
    std::fs::write(&gguf, b"GGUF").expect("escribir gguf ficticio");

    let output = Command::new(env!("CARGO_BIN_EXE_modelforge"))
        .args([
            "--ollama",
            DEAD_OLLAMA,
            "--json",
            "create",
            "test-model",
            "--from",
        ])
        .arg(&gguf)
        .output()
        .expect("ejecutar modelforge");

    assert!(
        !output.status.success(),
        "el comando debe fallar; status: {}",
        output.status
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("Ollama no responde"),
        "el error debe indicar que Ollama no responde; stdout: {stdout}"
    );
}
