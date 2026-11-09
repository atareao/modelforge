//! Tests de integración de `test` y `bench` cuando Ollama no está disponible.
//!
//! Ejecutan el binario real apuntando a un puerto cerrado y comprueban que ambos
//! subcomandos fallan con un error claro, salida JSON y código distinto de 0.

use std::process::Command;

/// Un puerto local cerrado que provoca un fallo de conexión inmediato.
const DEAD_OLLAMA: &str = "http://127.0.0.1:1";

/// Ejecuta `modelforge` con los argumentos dados.
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_modelforge"))
        .args(args)
        .output()
        .expect("ejecutar modelforge")
}

#[test]
fn test_fails_with_clear_error_when_ollama_is_down() {
    let output = run(&["--ollama", DEAD_OLLAMA, "--json", "test", "un-modelo"]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("Ollama no responde"),
        "el error debe indicar que Ollama no responde; stdout: {stdout}"
    );
}

#[test]
fn test_vision_with_missing_image_fails_before_calling_ollama() {
    let output = run(&[
        "--json",
        "test",
        "un-modelo",
        "--vision",
        "--image",
        "/ruta/que/no/existe.png",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(stdout.contains("no existe el fichero"), "stdout: {stdout}");
}

#[test]
fn bench_fails_with_clear_error_when_ollama_is_down() {
    let output = run(&["--ollama", DEAD_OLLAMA, "--json", "bench", "un-modelo"]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("Ollama no responde"),
        "el error debe indicar que Ollama no responde; stdout: {stdout}"
    );
}

#[test]
fn bench_compare_accepts_two_models() {
    // Solo comprueba el parseo del flag: el fallo debe ser de red, no de sintaxis.
    let output = run(&[
        "--ollama",
        DEAD_OLLAMA,
        "--json",
        "bench",
        "modelo-a",
        "--compare",
        "modelo-b",
    ]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Ollama no responde"),
        "debe fallar conectando, no parseando; stdout: {stdout}"
    );
}
