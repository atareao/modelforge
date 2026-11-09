//! Tests de integración reales de `test` y `bench` contra un Ollama en marcha.
//!
//! Están **ignorados** por defecto. Para ejecutarlos:
//!
//! ```console
//! cargo test -- --ignored --test test_bench_real_ollama
//! ```
//!
//! Variables opcionales: `MODELFORGE_TEST_OLLAMA` (URL base),
//! `MODELFORGE_TEST_MODEL` (modelo a probar) y `MODELFORGE_TEST_MODEL2`
//! (segundo modelo para el benchmark comparativo).

use std::process::Command;

/// Ejecuta `modelforge` con los argumentos dados.
fn run(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_modelforge"))
        .args(args)
        .output()
        .expect("ejecutar modelforge")
}

fn ollama() -> String {
    std::env::var("MODELFORGE_TEST_OLLAMA").unwrap_or_else(|_| "http://localhost:11434".to_string())
}

fn model() -> String {
    std::env::var("MODELFORGE_TEST_MODEL").unwrap_or_else(|_| "qwen3:8b".to_string())
}

#[test]
#[ignore = "requiere Ollama en marcha y un modelo cargable"]
fn smoke_tests_a_real_model() {
    let output = run(&[
        "--ollama",
        &ollama(),
        "--json",
        "test",
        &model(),
        "--tools",
        "--num-predict",
        "64",
    ]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "test falló; stdout: {stdout}");
    assert!(stdout.contains("\"ok\":true"), "stdout: {stdout}");
    assert!(stdout.contains("\"passed\":true"), "stdout: {stdout}");
}

#[test]
#[ignore = "requiere Ollama en marcha y un modelo cargable"]
fn smoke_tests_vision_on_a_real_model() {
    let output = run(&[
        "--ollama",
        &ollama(),
        "--json",
        "test",
        &model(),
        "--vision",
        "--num-predict",
        "64",
    ]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "test --vision falló; stdout: {stdout}"
    );
    assert!(stdout.contains("\"mode\":\"vision\""), "stdout: {stdout}");
}

#[test]
#[ignore = "requiere Ollama en marcha y un modelo cargable"]
fn benchmarks_a_real_model() {
    let output = run(&[
        "--ollama",
        &ollama(),
        "--json",
        "bench",
        &model(),
        "--num-predict",
        "32",
    ]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "bench falló; stdout: {stdout}");
    assert!(stdout.contains("\"ok\":true"), "stdout: {stdout}");
    assert!(stdout.contains("\"eval_count\""), "stdout: {stdout}");
    assert!(stdout.contains("\"tokens_per_second\""), "stdout: {stdout}");
    assert!(stdout.contains("\"processor\""), "stdout: {stdout}");
}

#[test]
#[ignore = "requiere Ollama en marcha y dos modelos cargables"]
fn benchmarks_and_compares_two_models() {
    let second = std::env::var("MODELFORGE_TEST_MODEL2").unwrap_or_else(|_| model());

    let output = run(&[
        "--ollama",
        &ollama(),
        "bench",
        &model(),
        "--compare",
        &second,
        "--num-predict",
        "32",
    ]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "bench --compare falló; stdout: {stdout}"
    );
    assert!(
        stdout.contains("tok/s"),
        "la tabla debe tener cabecera; stdout: {stdout}"
    );
}
