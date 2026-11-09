//! Tests de integración de `import` sobre el binario real.
//!
//! El primero no necesita red: falla antes de tocar el Hub (falta `--quant`). El
//! segundo levanta un servidor HTTP local que imita el listado del Hub y apunta
//! `HF_ENDPOINT` a él, de modo que el "quant inexistente" se prueba sin conexión.

use std::io::{Read as _, Write as _};
use std::net::TcpListener;
use std::process::{Command, Output};

/// Respuesta del Hub de prueba: un único GGUF que NO coincide con `Q6_K`.
const TREE_JSON: &str = r#"[{"type":"file","oid":"deadbeef","size":123,"path":"model-Q4_K_M.gguf","lfs":null,"lastCommit":null,"xetHash":null,"securityFileStatus":null}]"#;

/// Ejecuta `modelforge` con los argumentos dados.
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_modelforge"))
        .args(args)
        .output()
        .expect("ejecutar modelforge")
}

/// Levanta un servidor HTTP mínimo que responde `TREE_JSON` a cualquier petición.
///
/// Devuelve el endpoint (`http://127.0.0.1:PUERTO`) al que apuntar `HF_ENDPOINT`.
fn spawn_hf_mock() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
    let addr = listener.local_addr().expect("local_addr");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { break };
            let mut buffer = [0u8; 4096];
            let _ = stream.read(&mut buffer);
            let body = TREE_JSON.as_bytes();
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(body);
            let _ = stream.flush();
        }
    });
    format!("http://{addr}")
}

#[test]
fn import_without_quant_fails_with_json_error_and_nonzero_exit() {
    let output = run(&["--json", "import", "org/model"]);

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(stdout.contains("--quant"), "stdout: {stdout}");
}

#[test]
fn import_with_missing_quant_fails_with_nonzero_exit() {
    let endpoint = spawn_hf_mock();
    let home = tempfile::tempdir().expect("tempdir");
    let dest = home.path().join("models");

    let output = Command::new(env!("CARGO_BIN_EXE_modelforge"))
        .env("HF_ENDPOINT", &endpoint)
        .env("HF_HOME", home.path().join("hf-home"))
        .env("HF_HUB_DISABLE_IMPLICIT_TOKEN", "1")
        .env_remove("HF_TOKEN")
        .args([
            "--json",
            "import",
            "org/model",
            "--quant",
            "Q6_K",
            "--dest",
            dest.to_str().expect("utf-8"),
        ])
        .output()
        .expect("ejecutar modelforge");

    assert!(!output.status.success(), "status: {}", output.status);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\"ok\":false"), "stdout: {stdout}");
    assert!(
        stdout.contains("no contiene el quant"),
        "debe indicar que falta el quant; stdout: {stdout}"
    );
}

/// El import de un repo GGUF rechaza los flags que solo aplican al import nativo
/// (`--name`, `--quantize`) **antes** de salir a la red.
#[test]
fn import_gguf_rejects_native_only_flags_before_network() {
    for (flag, value) in [("--quantize", Some("nvfp4")), ("--name", Some("mi-modelo"))] {
        let mut args = vec!["--json", "import", "org/model", "--quant", "Q6_K", flag];
        if let Some(value) = value {
            args.push(value);
        }

        let output = run(&args);
        assert!(!output.status.success(), "{flag}: status {}", output.status);

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(stdout.contains("\"ok\":false"), "{flag}: stdout {stdout}");
        assert!(
            stdout.contains(flag.trim_start_matches('-')),
            "{flag}: el error debe nombrar el flag; stdout {stdout}"
        );
        assert!(stdout.contains("no acepta"), "{flag}: stdout {stdout}");
        // El rechazo ocurre antes de resolver el repo: no debe intentar el Hub.
        assert!(
            !stdout.contains("HuggingFace"),
            "{flag}: no debe llegar a la red; stdout {stdout}"
        );
    }
}
