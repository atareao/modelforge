# HANDOFF — modelforge

## Modelo ligero para tareas one-shot (issue #38, `plans/PLAN-005.md`) — 2026-11-11

**Evaluación (11 modelos, harness propio en `/tmp/opencode/oneshot/`):** ganador **`gemma3:4b`** como todoterreno (12/14); `qwen2.5-coder:1.5b` 5/5 en shell pero **sin persona**; `qwen2.5:1.5b` sirve para resúmenes pero **en vivo falla en `dime`** (contexto Windows, fences, erratas); por debajo de ~1B está roto; `qwen3:4b` **inservible** (filtra razonamiento con `think:false`). Doc: `docs/oneshot-models-evaluation.md`.

**Aplicado en el gateway (5 → 7 rutas):**
- **`local-oneshot` → `gemma3:4b`** — tareas one-shot de texto (resúmenes, respuestas).
- **`local-dime` → `gemma3:4b`** con warmup — comando de shell para `dime` **y** su persona ácida.
- **`dime` → `local-dime`** (`~/.config/dime/dime.yml`); `system_role` ampliado con "sin bloques de código ni backticks". Probado con `dime` real.
- Backups: `gateway.yaml.bak-oneshot-20261111070051`, `dime.yml.bak-oneshot-20261111070051`, `dime.yml.bak-fence-20261111070252`.

**Cerrado:** PR #39 mergeado en `development`; issue #38 cerrado; plan archivado en `plans/PLAN-005-ARCHIVED.md`.

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
  - **#31, #32 (2026-11-10, docs)** — consolidación de rutas de modelos y `local-agent` en `docs/coder-models-evaluation.md`; material de podcast en `docs/agentes-locales-podcast.md`.
  - **(2026-11-10, docs)** — redefinición a **5 rutas** en `llm-gateway` (nuevo `local-coder` con warmup y `local-agent` sobre `qwen35-9b-agent`; se elimina `local-gptoss`) + alineación de `opencode.jsonc`. Documentado en `docs/agentes-locales-podcast.md`.
- Contrato `--json`: `stdout` = un único objeto JSON; `stderr` = eventos de etapa en NDJSON puro.
- Suite: **159 tests unitarios + integración passing** (los reales de red/Ollama/llama.cpp van `#[ignore]` y gateados por env: `MODELFORGE_TEST_SAFETENSORS`, `MODELFORGE_TEST_LLAMACPP`, etc.).
- `cargo clippy --all-targets -- -D warnings` limpio.
- `cargo fmt --check` limpio.
- OpenSpec: specs consolidadas en `openspec/specs/modelforge/spec.md`; changes archivados (`2026-11-09-modelforge`, `2026-11-09-native-model-import`, `2026-11-09-llamacpp-bridge`) (`openspec validate modelforge --strict`).
- Planes: `plans/PLAN-001-ARCHIVED.md`, `plans/PLAN-002-ARCHIVED.md` y `plans/PLAN-003-ARCHIVED.md` (todos completados).
- Docs: `docs/modelforge.md`, `docs/native-import.md`, `docs/llamacpp-bridge.md`, `docs/coder-models-evaluation.md`, `docs/agentes-locales-podcast.md` y `docs/oneshot-models-evaluation.md`.
- `llm-gateway` (proxy OpenAI-compatible, `127.0.0.1:4010`) **activo**; **7 rutas** locales con failover a OpenRouter (ver sección de agentes locales).
- Limitación conocida: Ollama en Linux no tiene MLX → el import nativo de safetensors da `503 MLX runtime is not available`; en Linux usar `--backend llama.cpp`. En macOS la vía nativa sigue válida. Documentado en `docs/native-import.md` y `docs/llamacpp-bridge.md`.

## Evaluación de modelos de código locales (2026-11-10)

- Objetivo: modelos GGUF de código **<10 GB**, rápidos y con soporte de **tools** para RTX 4060 Ti 16 GB + Ollama 0.40.8. Detalle en `docs/coder-models-evaluation.md` (PRs #29, #31, #32).
- **Conservados** en Ollama:
  - `coder-qwopus-9b` (Qwen3.5-9B · 5,76 GB · 43 tok/s · tools ✅) → agente `local-coder`.
  - `qwen35-9b-agent` (`FROM qwen3.5:9b-q4_K_M` + `PARAMETER num_ctx 8192` · 5,34 GB · tools ✅) → agente `local-agent`.
- **Eliminados** (no cumplían la spec de tools): `coder-qwen25-7b`, `coder-qwen25-14b`, `coder-strand-rust-14b` (tool-calls no nativas en Ollama 0.40.8), `coder-dscoder-v2-lite` (sin tools), `qwen3-coder:30b-a3b-q4_K_M` + `coder-qwen35-4b-3d` (~23,5 GB liberados) y hoy `gpt-oss:20b` (~12 GB liberados, no cabía junto a `coder-qwopus-9b` en 16 GB).
- **Hallazgo central:** Ollama 0.40.8 **no parsea `tool_calls`** de la familia qwen2/qwen2.5 (falla también el `qwen2.5-coder:14b` oficial); solo la familia **Qwen3.x** emite tool-calls nativas **y en paralelo**.
- Nota operativa: para los benchmarks se detuvo temporalmente `llm-gateway.service`; **ya está restaurado (active)**.
- Follow-up abierto: posible issue en modelforge/Ollama por el tool-calling en qwen2/qwen2.5; reconsiderar Qwen2.5-Coder si se resuelve.

## Limpieza del almacén de modelos Ollama (2026-11-10)

- Almacén reducido de **19 → 8 modelos**, ~**93 GB** liberados (leftovers de evaluación).
- **Borrados**: `deepseek-v4-pro-9b-heretic:q6_K`, `qwen3:14b`, `mistral-small3.2:24b`, `gemma4:12b`, `mannix/jackod-9b-coder-dflash`, `granite4.2:8b`, `qwen3.5:9b-q4_K_M` (base de `qwen35-9b-agent`; los blobs están compartidos → borrarlo **no** rompe el modelo derivado), `qwen2.5-coder:14b`, `lfm2.5:24b-a2b-q4_K_M`, `devstral-small-2:24b`, `mannix/qwen3.6-27b-a3b-coder`.
- **Conservados** (8): `qwen35-9b-agent`, `coder-qwopus-9b`, `deepseek-v4-pro-9b-heretic:q6_K-256k` (ruta `local-deepseek`), `llama3.2:3b`, `qwen3:8b` (rutas), `bge-m3` (embeddings), y `llama3.1:8b` + `qwen2.5:7b-instruct` (referenciados por otros proyectos: `mcp-recipes` y tests de `llm-gateway`).
- Documentado en `docs/agentes-locales-podcast.md` (sección "Limpieza del almacén de modelos").
- Nota viva: la ruta `local-deepseek` sigue **sin que ningún agente la use**.

## Agentes locales en llm-gateway (2026-11-10)

- **5 rutas** locales sobre el proxy OpenAI-compatible (`127.0.0.1:4010`), con **failover a OpenRouter**:
  - `local-fast` → **`llama3.2:3b`**.
  - `local-writer` → **`qwen3:8b`**.
  - `local-coder` → **`coder-qwopus-9b`** (Qwen3.5-9B) — agente de **codificación** (y comandos de shell para `dime`), **con warmup**.
  - `local-agent` → **`qwen35-9b-agent`** (Qwen3.5-9B · `num_ctx` 8192) — agente de **tooling** y de los **8 subagentes**.
  - `local-deepseek` → **DeepSeek** (sigue sin que ningún agente la use).
  - Se **eliminó** la ruta `local-gptoss`.
- **Warmup/keep-alive activo SOLO en `local-coder`** (Opción A): `warmup.interval_ms=240000`, `timeout_ms=300000`. Mantiene `coder-qwopus-9b` residente.
- **8 subagentes** (`code-verifier`, `data-persistence`, `docker-expert`, `planificador-podcast-srt`, `postgresql-expert`, `sql-expert`, `sqlite-expert`, `typst-expert`) usan `model: gateway/local-agent`.
- **`dime` apunta a `local-coder`**: `~/.config/dime/dime.yml` (`model: local-coder`, `base_url: 127.0.0.1:4010`). Probado.
- **`qwen35-9b-agent`** = `FROM qwen3.5:9b-q4_K_M` + `PARAMETER num_ctx 8192`. Clave: el gateway **no envía `num_ctx`**; sin grabarlo, el segundo modelo pide 32k y **evicta** (hallazgo: log con `predicted="9.2 GiB" predicted_num_ctx=32768 ... evicting`; con 8192 → `fits alongside existing models`).
- **`OLLAMA_MAX_LOADED_MODELS=2`** (antes 1) en `/etc/systemd/system/ollama.service.d/override.conf`. **Coexistencia verificada**: `qwen35-9b-agent` (5,34 GB) + `coder-qwopus-9b` (5,76 GB), 100 % GPU, ~12,6/16 GB, tok/s intactos. **Solo caben 2 modelos 9B**.
- **`opencode.jsonc`** (provider `gateway`) alineado: `local-fast`, `local-writer`, `local-coder`, `local-agent` y `local-deepseek` (se quitaron `local-gptoss` y `local-code`).
- Documentación: `docs/agentes-locales-podcast.md` (material de podcast: agente de codificación y de tooling).

## Follow-up abierto: backend mistral.rs (2026-11-10)

- Evaluar un backend **`mistral.rs`** para `modelforge`: carga safetensors directo con ISQ, sin convertir a GGUF; **sustituiría a Ollama y llama.cpp**.
- Crates de conversión puro Rust a revisar: `candle`, `gguf-rs`, `compressr`, `rlx-gguf-convert`, `mlmf`.

## Tarea pendiente

- PR del fix de Blazar (6.2, issue #36) en sesión limpia. Cerrar #36 + archivar PLAN-004.

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
