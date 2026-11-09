# Proposal

## Why

El flujo de probar un modelo de HuggingFace en Ollama es manual y repetitivo: descargar el GGUF y el mmproj, generar un Modelfile, ejecutar `ollama create`, hacer smoke test, medir velocidad y añadir la ruta al gateway. Se repite cada vez que se evalúa un modelo nuevo (issue #1).

## What Changes

- Nuevo CLI `modelforge` como proyecto independiente (`atareao/modelforge`), sin tocar el binario `llm-gateway`.
- Subcomandos: `import`, `create`, `test`, `bench`, `list`, `info`, `route`.
- Descarga reanudable desde HuggingFace con selección de quant y mmproj.
- Generación de Modelfile con `RENDERER`, `PARSER`, `num_ctx` y visión.
- Creación vía API de Ollama (`/api/create`).
- Smoke test de texto, thinking, tools y vision.
- Benchmark con `eval_count`, `eval_duration`, `load_duration`, `size_vram`, `context` y `processor`.
- `route` opcional: añade la ruta al `gateway.yaml` con backup y sin duplicar.

## Capabilities

### New Capabilities
- `modelforge`: CLI de importación, creación y validación de modelos GGUF de HuggingFace en Ollama.

### Modified Capabilities
- (ninguna)

## Impact

- Nuevo proyecto independiente en `/data/rust/modelforge` (repo `atareao/modelforge`), sin dependencia del paquete `llm-gateway`. Dependencias: `clap`, `tokio`, `reqwest`, `serde`, `serde_json`, `serde_yaml`, `hf-hub` o API HTTP, `indicatif`, `anyhow`, `tracing`.
- No cambia el gateway ni sus rutas. El `route` escribe en `~/.config/opencode/gateway.yaml` con backup.
- Tests unitarios e integración. Documentación en `docs/modelforge.md`.
- CI propio: `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build --verbose` y `cargo test --verbose`.