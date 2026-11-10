# HANDOFF — modelforge

## Estado

- Repo independiente: `atareao/modelforge` (local: `/data/rust/modelforge`).
- Versión actual: **v0.3.0** (release publicada con binarios x86_64/aarch64). `main == development` a nivel de código; `development` va por delante en docs.
- **Issue #8** (import nativo de safetensors / HuggingFace hub): completado y cerrado.
- **Issue #24** (puente llama.cpp para safetensors→GGUF en Linux): completado y cerrado.
- **Issue #1**: cerrado.
- Ramas: `main` (producción, v0.3.0) · `development` (integración). `development` == `main` (solo docs por delante).
- **CI/CD configurado**: `.github/workflows/{ci,release-prepare,release}.yml`, `.vampus.yml`, `cliff.toml`, `GIT_FLOW.md`, `AGENTS.md`.
- Secretos `GH_PAT` y `CARGO_REGISTRY_TOKEN` configurados en el repo.
- El job `publish` a crates.io se eliminó (el nombre `modelforge` ya está ocupado en crates.io).
- Auditoría Rust `@rust-reviewer`: ✅ **APROBADA** en #8 y #24.
- Features entregadas:
  - **#8 (v0.2.0)** — import nativo de safetensors (solo macOS/MLX) + cuantización nativa + `pull hf.co/<repo>:<quant>`:
    1. `modelforge import <dir> --name <m>` — import nativo de un directorio con `.safetensors` (Modelfile `FROM <dir>`).
    2. `modelforge import <dir> --quantize <QUANT> --name <m>` — cuantización nativa en el import.
    3. `modelforge pull hf.co/<repo>:<quant>` — atajo remoto del hub.
  - **#24 (v0.3.0)** — puente **opcional** a llama.cpp para Linux:
    `modelforge import <dir> --backend llama.cpp [--quantize Q4_K_M] --name <m>`.
    Detecta `python3` + `convert_hf_to_gguf.py` + `llama-quantize` en `PATH`; si faltan, error accionable (sin auto-install). Backend por defecto `ollama` sin cambios.
  - **#29 (2026-11-10, docs)** — evaluación de modelos de código locales GGUF sobre RTX 4060 Ti 16 GB con Ollama 0.40.8; documentada en `docs/coder-models-evaluation.md`.
- Contrato `--json`: `stdout` = un único objeto JSON; `stderr` = eventos de etapa en NDJSON puro.
- Suite: **159 tests unitarios + integración passing** (los reales de red/Ollama/llama.cpp van `#[ignore]` y gateados por env: `MODELFORGE_TEST_SAFETENSORS`, `MODELFORGE_TEST_LLAMACPP`, etc.).
- `cargo clippy --all-targets -- -D warnings` limpio.
- `cargo fmt --check` limpio.
- OpenSpec: specs consolidadas en `openspec/specs/modelforge/spec.md`; changes archivados (`2026-11-09-modelforge`, `2026-11-09-native-model-import`, `2026-11-09-llamacpp-bridge`) (`openspec validate modelforge --strict`).
- Planes: `plans/PLAN-001-ARCHIVED.md`, `plans/PLAN-002-ARCHIVED.md` y `plans/PLAN-003-ARCHIVED.md` (todos completados).
- Docs: `docs/modelforge.md`, `docs/native-import.md`, `docs/llamacpp-bridge.md` y `docs/coder-models-evaluation.md`.
- Limitación conocida: Ollama en Linux no tiene MLX → el import nativo de safetensors da `503 MLX runtime is not available`; en Linux usar `--backend llama.cpp`. En macOS la vía nativa sigue válida. Documentado en `docs/native-import.md` y `docs/llamacpp-bridge.md`.

## Evaluación de modelos de código locales (2026-11-10)

- Objetivo: modelos GGUF de código **<10 GB**, rápidos y con soporte de **tools** para RTX 4060 Ti 16 GB + Ollama 0.40.8. Detalle en `docs/coder-models-evaluation.md` (PR #29).
- **Conservados** en Ollama:
  - `coder-qwopus-9b` (Qwen3.5-9B · 6,6 GB · 43 tok/s · tools ✅).
  - `coder-qwen35-4b-3d` (Qwen3.5-4B · 4,5 GB · 54 tok/s · tools ✅).
- **Eliminados** (no cumplían la spec de tools): `coder-qwen25-7b`, `coder-qwen25-14b`, `coder-strand-rust-14b` (tool-calls no nativas en Ollama 0.40.8) y `coder-dscoder-v2-lite` (sin tools).
- Hallazgo: Ollama 0.40.8 **no parsea `tool_calls`** de la familia qwen2/qwen2.5 (falla también el `qwen2.5-coder:14b` oficial); **qwen3.5 sí emite tool-calls nativas**.
- Nota operativa: para los benchmarks se detuvo temporalmente `llm-gateway.service`; **ya está restaurado (active)**.
- Follow-up abierto: posible issue en modelforge/Ollama por el tool-calling en qwen2/qwen2.5; reconsiderar Qwen2.5-Coder si se resuelve.

## Tarea pendiente

- Ninguna.

## Notas de tooling

- Bug corregido en v0.2.x `release-prepare.yml`: no detectaba `feat` con prefijo gitmoji (`^feat` no casaba con `✨ feat(...)`) → los `feat` se publicaban como patch. Regex corregida.
- `ci.yml`: `fmt --check`, `clippy --all-targets -D warnings`, `test` en PR a `main`/`development`.

## Verificación rápida

```bash
cd /data/rust/modelforge
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
openspec validate modelforge --strict
```

## Notas

- Sin `unwrap`/`panic` en producción.
- Red aislada tras los traits `HfClient` / `OllamaClient`.
- llama.cpp aislado tras el trait `Converter`.
- El `gateway.yaml` real del usuario permanece intacto.
- Proyecto independiente de `llm-gateway`; no hay workspace ni crate compartido.
- El `route` escribe en `~/.config/opencode/gateway.yaml` con backup.
