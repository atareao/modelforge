# AGENTS.md — modelforge

CLI en Rust (`modelforge`) para importar modelos GGUF de HuggingFace, crearlos en Ollama, validarlos y enrutarlos en el gateway de OpenCode.

## Stack

- Rust (binario `modelforge`, edición en `Cargo.toml`).
- Tests: `cargo test` (los de red/Ollama real van `#[ignore]`).
- Lint/format: `cargo clippy --all-targets -- -D warnings` y `cargo fmt --all -- --check`.
- Specs: OpenSpec en `openspec/`.

## Git Flow

This project follows strict gitflow. See [GIT_FLOW.md](./GIT_FLOW.md) for:
- Branch structure (main, development, feature/*, hotfix/*)
- Conventional commits with gitmoji
- How to create features, hotfixes, and releases

## Verificación antes de commit

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```
