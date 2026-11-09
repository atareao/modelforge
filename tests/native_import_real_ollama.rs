//! Test de integración **real** del import nativo (safetensors) contra Ollama.
//!
//! Está **ignorado** por defecto: crea un modelo a partir de un snapshot nativo
//! real y requiere un host soportado por el conversor interno de Ollama. Ollama
//! 0.40.8 en Linux **no** puede crear desde safetensors: responde
//! `503 {"error":"MLX runtime is not available"}`, así que este test solo corre en
//! un host soportado (p. ej. macOS con MLX). Es la cobertura *diferida* de las
//! tareas 1.3 y 5.2 de `openspec/changes/native-model-import/tasks.md`.
//!
//! Para ejecutarlo:
//!
//! ```console
//! MODELFORGE_TEST_SAFETENSORS=/ruta/al/snapshot \
//!   cargo test -- --ignored imports_safetensors_with_real_ollama
//! ```
//!
//! Variables opcionales: `MODELFORGE_TEST_OLLAMA` (URL base), `MODELFORGE_TEST_NAME`
//! (nombre del modelo) y `MODELFORGE_TEST_QUANTIZE` (p. ej. `nvfp4`).

use std::ffi::OsString;
use std::process::{Command, Output};

/// Ejecuta el binario `modelforge` con los argumentos dados.
fn run_modelforge(args: &[OsString]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_modelforge"))
        .args(args)
        .output()
        .expect("ejecutar modelforge")
}

/// Importa `safetensors` como `name` (opcionalmente con `quantize`) y verifica el
/// resultado con el propio CLI de Ollama; limpia el artefacto al terminar.
fn import_and_verify(name: &str, safetensors: &str, quantize: Option<&str>) {
    let ollama = std::env::var("MODELFORGE_TEST_OLLAMA")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());

    let mut args: Vec<OsString> = vec![
        "--ollama".into(),
        ollama.into(),
        "--json".into(),
        "import".into(),
        safetensors.to_string().into(),
        "--name".into(),
        name.to_string().into(),
    ];
    if let Some(quantize) = quantize {
        args.push("--quantize".into());
        args.push(quantize.to_string().into());
    }

    let output = run_modelforge(&args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "import nativo falló; stdout: {stdout}"
    );
    assert!(stdout.contains("\"ok\":true"), "stdout: {stdout}");

    // Verificación independiente con el propio CLI de Ollama.
    let show = Command::new("ollama")
        .args(["show", name])
        .output()
        .expect("ejecutar ollama show");
    assert!(
        show.status.success(),
        "`ollama show {name}` falló: {}",
        String::from_utf8_lossy(&show.stderr)
    );

    // Limpieza del artefacto creado por el propio test.
    let _ = Command::new("ollama").args(["rm", name]).status();
}

/// Flujo real: safetensors → import nativo → modelo en Ollama (tarea 1.3/5.2).
#[test]
#[ignore = "requiere Ollama en un host soportado (MLX) y un snapshot real; define MODELFORGE_TEST_SAFETENSORS"]
fn imports_safetensors_with_real_ollama() {
    let safetensors = std::env::var("MODELFORGE_TEST_SAFETENSORS")
        .expect("define MODELFORGE_TEST_SAFETENSORS con la ruta a un snapshot de safetensors");
    let name = std::env::var("MODELFORGE_TEST_NAME")
        .unwrap_or_else(|_| "modelforge-native-it:latest".to_string());

    import_and_verify(&name, &safetensors, None);
}

/// Flujo real: safetensors → import nativo con `--quantize` → modelo cuantizado
/// en Ollama (tarea 5.2).
#[test]
#[ignore = "requiere Ollama en un host soportado (MLX) y un snapshot real; define MODELFORGE_TEST_SAFETENSORS"]
fn imports_safetensors_quantized_with_real_ollama() {
    let safetensors = std::env::var("MODELFORGE_TEST_SAFETENSORS")
        .expect("define MODELFORGE_TEST_SAFETENSORS con la ruta a un snapshot de safetensors");
    let quantize =
        std::env::var("MODELFORGE_TEST_QUANTIZE").unwrap_or_else(|_| "nvfp4".to_string());
    let name = std::env::var("MODELFORGE_TEST_NAME")
        .unwrap_or_else(|_| "modelforge-native-it:nvfp4".to_string());

    import_and_verify(&name, &safetensors, Some(&quantize));
}
