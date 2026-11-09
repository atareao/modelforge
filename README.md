# modelforge

CLI en Rust para importar modelos GGUF desde HuggingFace, crear el modelo en Ollama, probarlo, medir su rendimiento y, opcionalmente, añadir su ruta a un gateway OpenAI-compatible (OpenCode).

## Estado

**v0.1.0** — funcional. Comandos implementados y verificados de extremo a extremo: `import`, `create`, `test`, `bench`, `list`, `info`, `route`.

## Instalación

```bash
git clone https://github.com/atareao/modelforge
cd modelforge
cargo build --release
# binario en target/release/modelforge
```

## Uso

Flags globales, válidos para todos los comandos:

| Flag | Descripción |
|---|---|
| `--ollama <URL>` | Endpoint de Ollama (por defecto `http://localhost:11434`). |
| `--config <PATH>` | Ruta al `gateway.yaml` para `route`. |
| `--json` | Salida en JSON. |
| `--verbose` | Salida detallada. |

### `import` — descargar un GGUF de HuggingFace

Descarga reanudable con barra de progreso, filtro de quant y soporte de `mmproj` (modelos de visión).

```bash
modelforge import ggml-org/tiny-llamas --quant q4_0 --dest ./models
modelforge import ggml-org/SmolVLM-256M-Instruct-GGUF --quant Q8_0 --mmproj --dest ./models
```

### `create` — crear el modelo en Ollama

Genera un Modelfile reproducible y crea el modelo vía `/api/create`.

```bash
modelforge create mi-modelo:q6_K-256k \
  --from ./models/tiny-llamas-q6_K.gguf \
  --ctx 262144
```

### `test` — smoke test

Comprueba el modelo con texto, thinking, tools y visión.

```bash
modelforge test mi-modelo:q6_K-256k --text
modelforge test mi-modelo:q6_K-256k --vision
```

### `bench` — rendimiento

Mide métricas y permite comparar dos modelos.

```bash
modelforge bench mi-modelo:q6_K-256k --tokens 256 --json
modelforge bench modelo-a --compare modelo-b
```

### `list` / `info` — inventario

```bash
modelforge list
modelforge info mi-modelo:q6_K-256k
```

### `route` — añadir la ruta al gateway

Añade una ruta al `gateway.yaml` (por defecto `~/.config/opencode/gateway.yaml`) con backup automático y sin duplicados.

```bash
modelforge route local-heretic --model mi-modelo:q6_K-256k
```

## Flujo completo

```bash
modelforge import ggml-org/tiny-llamas --quant q4_0 --dest ./models
modelforge create mi-modelo:q4_0 --from ./models/tiny-llamas-q4_0.gguf --ctx 32768
modelforge test mi-modelo:q4_0 --text
modelforge bench mi-modelo:q4_0 --tokens 256
modelforge route local --model mi-modelo:q4_0 --config ./gateway.yaml
```

## Calidad

- Suite: 119 tests pasando (los que requieren red/Ollama real van `#[ignore]`).
- `cargo clippy --all-targets -- -D warnings` limpio.
- `cargo fmt --all -- --check` limpio.
- CI en GitHub Actions: formato, lint y tests en cada PR a `main`/`development`.

## Documentación

- Guía de uso: [`docs/modelforge.md`](docs/modelforge.md)
- Plan: [`plans/PLAN-001.md`](plans/PLAN-001.md)
- Especificación OpenSpec: [`openspec/specs/modelforge/spec.md`](openspec/specs/modelforge/spec.md)
- Flujo Git: [`GIT_FLOW.md`](GIT_FLOW.md)
