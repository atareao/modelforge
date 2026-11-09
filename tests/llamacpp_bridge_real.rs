//! Test de integración **real** del puente a llama.cpp.
//!
//! Está **ignorado** por defecto: requiere `llama.cpp` (`convert_hf_to_gguf.py` +
//! `llama-quantize`) en `PATH`, Python 3, un snapshot real de safetensors y un
//! Ollama accesible. Es la cobertura *diferida* de la tarea 5.2.
//!
//! ```console
//! MODELFORGE_TEST_LLAMACPP=1 MODELFORGE_TEST_SAFETENSORS=/ruta/al/snapshot \
//!   cargo test -- --ignored imports_safetensors_with_llamacpp
//! ```
//!
//! Variables opcionales: `MODELFORGE_TEST_OLLAMA`, `MODELFORGE_TEST_NAME` y
//! `MODELFORGE_TEST_QUANT` (p. ej. `Q4_K_M`).

use std::ffi::OsString;
use std::process::{Command, Output};

/// Ejecuta el binario `modelforge` con los argumentos dados.
fn run_modelforge(args: &[OsString]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_modelforge"))
        .args(args)
        .output()
        .expect("ejecutar modelforge")
}

/// Importa un snapshot de safetensors con `--backend llama.cpp` y verifica el
/// modelo con el propio CLI de Ollama; limpia el artefacto al terminar.
fn import_and_verify(name: &str, safetensors: &str, quant: Option<&str>) {
    let ollama = std::env::var("MODELFORGE_TEST_OLLAMA")
        .unwrap_or_else(|_| "http://localhost:11434".to_string());

    let mut args: Vec<OsString> = vec![
        "--ollama".into(),
        ollama.into(),
        "--json".into(),
        "import".into(),
        safetensors.to_string().into(),
        "--backend".into(),
        "llama.cpp".into(),
        "--name".into(),
        name.to_string().into(),
    ];
    if let Some(quant) = quant {
        args.push("--quantize".into());
        args.push(quant.to_string().into());
    }

    let output = run_modelforge(&args);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "import con llama.cpp falló; stdout: {stdout}"
    );
    assert!(stdout.contains("\"ok\":true"), "stdout: {stdout}");

    let show = Command::new("ollama")
        .args(["show", name])
        .output()
        .expect("ejecutar ollama show");
    assert!(
        show.status.success(),
        "`ollama show {name}` falló: {}",
        String::from_utf8_lossy(&show.stderr)
    );

    let _ = Command::new("ollama").args(["rm", name]).status();
}

/// Flujo real: safetensors → GGUF (llama.cpp) → modelo en Ollama (tarea 4.1/5.2).
#[test]
#[ignore = "requiere llama.cpp + Python 3 + Ollama y un snapshot real; define MODELFORGE_TEST_LLAMACPP=1 y MODELFORGE_TEST_SAFETENSORS"]
fn imports_safetensors_with_llamacpp() {
    assert_eq!(
        std::env::var("MODELFORGE_TEST_LLAMACPP").as_deref(),
        Ok("1"),
        "define MODELFORGE_TEST_LLAMACPP=1 para habilitar el test real"
    );
    let safetensors = std::env::var("MODELFORGE_TEST_SAFETENSORS")
        .expect("define MODELFORGE_TEST_SAFETENSORS con la ruta a un snapshot de safetensors");
    let name = std::env::var("MODELFORGE_TEST_NAME")
        .unwrap_or_else(|_| "modelforge-llamacpp-it:latest".to_string());

    import_and_verify(&name, &safetensors, None);
}

/// Flujo real con `--quantize`: safetensors → GGUF → quant → modelo en Ollama.
#[test]
#[ignore = "requiere llama.cpp + Python 3 + Ollama y un snapshot real; define MODELFORGE_TEST_LLAMACPP=1 y MODELFORGE_TEST_SAFETENSORS"]
fn imports_safetensors_quantized_with_llamacpp() {
    assert_eq!(
        std::env::var("MODELFORGE_TEST_LLAMACPP").as_deref(),
        Ok("1"),
        "define MODELFORGE_TEST_LLAMACPP=1 para habilitar el test real"
    );
    let safetensors = std::env::var("MODELFORGE_TEST_SAFETENSORS")
        .expect("define MODELFORGE_TEST_SAFETENSORS con la ruta a un snapshot de safetensors");
    let quant = std::env::var("MODELFORGE_TEST_QUANT").unwrap_or_else(|_| "Q4_K_M".to_string());
    let name = std::env::var("MODELFORGE_TEST_NAME")
        .unwrap_or_else(|_| "modelforge-llamacpp-it:Q4_K_M".to_string());

    import_and_verify(&name, &safetensors, Some(&quant));
}
