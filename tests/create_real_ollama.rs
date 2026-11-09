//! Test de integración real de `create` contra un Ollama en marcha.
//!
//! Está **ignorado** por defecto porque requiere un servidor Ollama y un GGUF real.
//! Para ejecutarlo:
//!
//! ```console
//! MODELFORGE_TEST_GGUF=/ruta/al/model.gguf cargo test -- --ignored creates_model_with_real_ollama
//! ```
//!
//! Variables opcionales: `MODELFORGE_TEST_OLLAMA` (URL base), `MODELFORGE_TEST_NAME`
//! (nombre del modelo), `MODELFORGE_TEST_CTX` (`num_ctx`), `MODELFORGE_TEST_MMPROJ`.

use std::process::Command;

/// Ejecuta el binario `modelforge` con los argumentos dados.
fn run_modelforge(args: &[std::ffi::OsString]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_modelforge"))
        .args(args)
        .output()
        .expect("ejecutar modelforge")
}

#[test]
#[ignore = "requiere Ollama en marcha y un GGUF real; define MODELFORGE_TEST_GGUF"]
fn creates_model_with_real_ollama() {
    let gguf = std::env::var("MODELFORGE_TEST_GGUF")
        .expect("define MODELFORGE_TEST_GGUF con la ruta a un GGUF real");
    let ollama = std::env::var("MODELFORGE_TEST_OLLAMA")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());
    let name = std::env::var("MODELFORGE_TEST_NAME")
        .unwrap_or_else(|_| "modelforge-it:latest".to_string());
    let ctx = std::env::var("MODELFORGE_TEST_CTX").unwrap_or_else(|_| "512".to_string());

    let mut args: Vec<std::ffi::OsString> = vec![
        "--ollama".into(),
        ollama.into(),
        "--json".into(),
        "create".into(),
        name.clone().into(),
        "--from".into(),
        gguf.into(),
        "--ctx".into(),
        ctx.into(),
    ];
    if let Ok(mmproj) = std::env::var("MODELFORGE_TEST_MMPROJ") {
        args.push("--mmproj".into());
        args.push(mmproj.into());
    }

    let output = run_modelforge(&args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "create falló; stdout: {stdout}");
    assert!(stdout.contains("\"ok\":true"), "stdout: {stdout}");

    // Verificación independiente con el propio CLI de Ollama.
    let show = Command::new("ollama")
        .args(["show", &name])
        .output()
        .expect("ejecutar ollama show");
    assert!(
        show.status.success(),
        "`ollama show {name}` falló: {}",
        String::from_utf8_lossy(&show.stderr)
    );

    // Limpieza del artefacto creado por el propio test.
    let _ = Command::new("ollama").args(["rm", &name]).status();
}
