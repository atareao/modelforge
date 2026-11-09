# HANDOFF — modelforge

## Estado

- Repo independiente: `atareao/modelforge` (local: `/data/rust/modelforge`).
- Versión actual: **v0.2.0** (release publicada con binarios x86_64/aarch64). `main == development == v0.2.0`.
- **Issue #8** (import nativo de safetensors / HuggingFace hub): completado y cerrado.
- **Issue #1**: cerrado.
- Ramas: `main` (producción, v0.2.0) · `development` (integración). `development` == `main`.
- **CI/CD configurado**: `.github/workflows/{ci,release-prepare,release}.yml`, `.vampus.yml`, `cliff.toml`, `GIT_FLOW.md`, `AGENTS.md`.
- Secretos `GH_PAT` y `CARGO_REGISTRY_TOKEN` configurados en el repo.
- El job `publish` a crates.io se eliminó (el nombre `modelforge` ya está ocupado en crates.io).
- Auditoría Rust `@rust-reviewer`: ✅ **APROBADA** (2 rondas; corregido bloqueante de la whitelist de quants vs Ollama real).
- Feature #8 entregada en 3 flujos, sin dependencias externas (ni Python ni llama.cpp):
  1. `modelforge import <dir> --name <m>` — import nativo de un directorio con `.safetensors` (Modelfile `FROM <dir>`).
  2. `modelforge import <dir> --quantize <QUANT> --name <m>` — cuantización nativa en el import (valores reales de Ollama 0.40.8: `int4, int8, nvfp4, mxfp4, mxfp8`).
  3. `modelforge pull hf.co/<repo>:<quant>` — atajo remoto del hub.
- Contrato `--json`: `stdout` = un único objeto JSON; `stderr` = eventos de etapa en NDJSON puro.
- Suite: **135 tests unitarios + integración passing** (los de red/Ollama real van `#[ignore]` y gateados por env `MODELFORGE_TEST_SAFETENSORS`).
- `cargo clippy --all-targets -- -D warnings` limpio.
- `cargo fmt --check` limpio.
- OpenSpec: change `native-model-import` archivado en `openspec/changes/archive/2026-11-09-native-model-import/`; spec consolidada en `openspec/specs/modelforge/spec.md` (`openspec validate modelforge --strict`).
- Planes: `plans/PLAN-001-ARCHIVED.md` y `plans/PLAN-002-ARCHIVED.md` (ambos completados).
- Docs: `docs/modelforge.md` y `docs/native-import.md`.
- Limitación conocida: Ollama 0.40.8 en Linux no puede crear desde safetensors (`503 MLX runtime is not available`); requiere MLX (macOS). Documentado en `docs/native-import.md`.

## Tarea pendiente

- Ninguna.

## Notas de tooling

- Bug corregido en `release-prepare.yml`: no detectaba `feat` con prefijo gitmoji (`^feat` no casaba con `✨ feat(...)`) → los `feat` se publicaban como patch. Regex corregida; se descartó el v0.1.3 erróneo (release/tag borrados) y se re-publicó como v0.2.0.
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
- El `gateway.yaml` real del usuario permanece intacto.
- Proyecto independiente de `llm-gateway`; no hay workspace ni crate compartido.
- El `route` escribe en `~/.config/opencode/gateway.yaml` con backup.
