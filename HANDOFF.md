# HANDOFF — modelforge

## Estado

- Repo independiente: `atareao/modelforge` (local: `/data/rust/modelforge`).
- Versión actual: **v0.3.0** (release publicada con binarios x86_64/aarch64). `main == development == v0.3.0`.
- **Issue #8** (import nativo de safetensors / HuggingFace hub): completado y cerrado.
- **Issue #24** (puente llama.cpp para safetensors→GGUF en Linux): completado y cerrado.
- **Issue #1**: cerrado.
- Ramas: `main` (producción, v0.3.0) · `development` (integración). `development` == `main`.
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
- Contrato `--json`: `stdout` = un único objeto JSON; `stderr` = eventos de etapa en NDJSON puro.
- Suite: **159 tests unitarios + integración passing** (los reales de red/Ollama/llama.cpp van `#[ignore]` y gateados por env: `MODELFORGE_TEST_SAFETENSORS`, `MODELFORGE_TEST_LLAMACPP`, etc.).
- `cargo clippy --all-targets -- -D warnings` limpio.
- `cargo fmt --check` limpio.
- OpenSpec: specs consolidadas en `openspec/specs/modelforge/spec.md`; changes archivados (`2026-11-09-modelforge`, `2026-11-09-native-model-import`, `2026-11-09-llamacpp-bridge`) (`openspec validate modelforge --strict`).
- Planes: `plans/PLAN-001-ARCHIVED.md`, `plans/PLAN-002-ARCHIVED.md` y `plans/PLAN-003-ARCHIVED.md` (todos completados).
- Docs: `docs/modelforge.md`, `docs/native-import.md` y `docs/llamacpp-bridge.md`.
- Limitación conocida: Ollama en Linux no tiene MLX → el import nativo de safetensors da `503 MLX runtime is not available`; en Linux usar `--backend llama.cpp`. En macOS la vía nativa sigue válida. Documentado en `docs/native-import.md` y `docs/llamacpp-bridge.md`.

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
