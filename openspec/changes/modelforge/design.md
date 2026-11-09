# Design

## Context

El flujo actual está documentado en `HANDOFF.md` (modelo `deepseek-v4-pro-9b-heretic`). El gateway es un proxy OpenAI-compatible; no debe gestionar modelos. El proyecto es independiente: `atareao/modelforge` en `/data/rust/modelforge`, con su propio repo, CI y release. El equipo tiene 16 GB de VRAM y `OLLAMA_MAX_LOADED_MODELS=1`, así que el CLI debe probar un modelo a la vez y reportar VRAM. Ver `proposal.md` para la motivación.

## Goals / Non-Goals

**Goals:**
- Automatizar el flujo HF → Oll, test → benchmark → ruta.
- Descarga reanudable, con progreso y selección de quant/mmproj.
- Modelfile reproducible con vision, renderer, parser y ctx.
- Salida JSON para agentes.
- Tests unitarios e integración.

**Non-Goals:**
- No reimplementar el protocolo de HF; usar `hf-hub` o la API HTTP.
- No meter el gestor en el binario `llm-gateway`; es un proyecto independiente.
- No gestionar el ciclo de vida de la GPU más allá de reportar VRAM.
- No borrar modelos automáticamente sin confirmación.

## Decisions

- **Proyecto independiente** `atareao/modelforge` en `/data/rust/modelforge`: mantiene el gateway enfocado y permite versionar, testear y publicar el CLI por separado. Alternativa: crate dentro del workspace de `llm-gateway`; rechazada por mezclar dominios y releases. Alternativa: binario extra en el paquete raíz; rechazada por mezclar dependencias y alargar el build del gateway.
- **`hf-hub`** para resolver y descargar: maneja xet, reanudación y cache. Alternativa: `reqwest` manual; más código y frágil.
- **API HTTP de Ollama** (`/api/create`, `/api/chat`, `/api/generate`, `/api/tags`, `/api/ps`) en vez de invocar el CLI: evita parsing de salida y permite JSON.
- **`serde_yaml`** para `route`: parsea, valida y evita duplicados. Alternativa: append de texto; rechazada.
- **`clap` + `tokio` + `reqwest` + `indicatif` + `anyhow`/`tracing`**: stack estándar y consistente con el gateway.
- **`test`/`bench`** devuelven JSON con `eval_count`, `eval_duration`, `load_duration`, `size_vram`, `context`, `processor`.

## Risks / Trade-offs

- [HF rate limit o repo gated] → token `HF_TOKEN` opcional y reintentos con backoff.
- [Descarga interrumpida] → `.part` + `Range` + reanudación.
- [Modelo no cabe en VRAM] → `info` reporta `size_vram` y `processor`; el CLI no fuerza carga si no cabe.
- [Ollama API cambia] → encapsular en un módulo `ollama.rs` con tests.
- [Sobrescribir `gateway.yaml`] → backup previo y `--dry-run`.
- [Proyecto nuevo con CI propio] → el CI se configura en este repo; no afecta al gateway.

## Migration Plan

- El proyecto es nuevo; no hay migración de datos. El CLI es aditivo. Rollback: borrar el proyecto y el binario.
- El `route` es opcional; si se usa, se puede restaurar el backup.

## Open Questions

- (ninguna)