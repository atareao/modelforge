# PLAN-004 — Evaluación de motores de inferencia locales (mistral.rs / Blazar / Ollama)

- **Issue:** [#36](https://github.com/atareao/modelforge/issues/36)
- **Fecha:** 2026-11-10
- **Estado:** En curso
- **Rama:** `development`
- **Ámbito:** investigación / decisión de arquitectura (no toca el binario `modelforge` todavía)

## 1. Objetivo

Decidir, **con datos medidos**, el rumbo del stack local de LLMs:

1. ¿Qué **motor** sirve mejor los modelos reales (Qwen3.5, arquitectura `qwen35`, híbrida linear-attention + visión)?
2. ¿Merece la pena sustituir `Ollama` por `mistral.rs`?
3. ¿Merece la pena sustituir `llm-gateway` y/o la capa de gestión por **Blazar**?
4. ¿Cuál es el destino de **`modelforge`**?

Requisitos de fondo del stack: modelos < 10 GB, especializados en código, rápidos, con **tool-calling (function calling)** funcionando en la cadena real (`opencode` → gateway → motor).

## 2. Estado de partida

- Stack validado y en producción: **Ollama 0.40.8** con 5 rutas en `~/.config/opencode/gateway.yaml`
  (`local-fast` llama3.2:3b, `local-writer` qwen3:8b, `local-coder` coder-qwopus-9b con warmup,
  `local-agent` qwen35-9b-agent, `local-deepseek`).
- 8 subagentes en `~/.config/opencode/agents/*.md` apuntando a `gateway/local-agent`.
- Todos los modelos reales del usuario son `architecture qwen35` (híbridos con torre de visión).
- `Ollama` corre como servicio systemd con `OLLAMA_MODELS=/var/lib/ollama`,
  `OLLAMA_MAX_LOADED_MODELS=2`, `OLLAMA_CONTEXT_LENGTH=32768`, `OLLAMA_FLASH_ATTENTION=1`,
  `OLLAMA_KV_CACHE_TYPE=q8_0`.
- Hardware: **NVIDIA RTX 4060 Ti, 16 GB**, driver **615.78.08**, CUDA toolkit **13.4**, compute capability **8.9**.

## 3. Decisiones tomadas

| # | Decisión | Motivo |
|---|---|---|
| D1 | **Ollama se queda como motor para GGUF.** | Es el más rápido o empata, es estable, y ya está integrado. |
| D2 | **`mistral.rs` es la vía para Qwen3.5** (safetensors/UQFF). | Ollama no puede con la vía tensorial nativa de Qwen3.5; mistral.rs sí reconoce `Qwen3_5ForConditionalGeneration` y la arch `qwen35` en GGUF. **Confirmado 2026-11-11**: con las 32 capas en GPU (`-n 0:32`) da **31,1 tok/s**, a la par de Ollama (30,3). El 14,78 previo era puro offload a CPU. |
| D3 | **Blazar NO se adopta como sustituto del stack** (por ahora). | No aporta ventaja de velocidad para los GGUF actuales, enruta GGUF a llama.cpp (no a mistral.rs), añade 47 s de arranque en frío y es un proyecto de 2 meses con bus factor 1. |
| D4 | **Decidida 2026-11-11** (ver 6.3): Ollama sigue como **motor primario GGUF**; mistral.rs queda como **motor complementario** para safetensors/UQFF; `llm-gateway` se mantiene; `modelforge` se reduce a **forja + batería comparativa**. | mistral.rs iguala en tok/s pero usa 13,2 GB de VRAM (no permite coexistir dos 9B) y ~26 s de cold start: no es un sustituto gratuito de Ollama. Blazar no aporta (D3). |
| D5 | **El fix de Blazar (bug CUDA) se prepara como PR** al repo externo `santanu20/blazar`. | Es un bug real de compatibilidad con drivers >= 615; afecta a cualquiera con hardware moderno. |

### Nota sobre el pivote descartado
Se evaluó y **se descarta** `mistral.rs` como sustituto total de Ollama: los formatos no se solapan
(GGUF -> Ollama/llama.cpp; safetensors/UQFF -> mistral.rs), y sustituir Ollama implicaría reconstruir
su capa de gestión (store, keep-alive, evict, coexistencia). Queda como **motor complementario**.

## 4. Evidencia — benchmarks

Todas las medidas con **el mismo modelo en cada motor**, mismo prompt
(`Write a Python function that reverses a linked list. Only code.`), 256 tokens de salida,
`temperature: 0`, y VRAM liberada antes de cada medición.

### 4.1 Benchmark a 1.5B — Qwen2.5-1.5B Q4_K_M (GGUF, `/tmp/opencode/gguf-spike2/qwen25-15b-q4km.gguf`)

| Motor | Backend | tok/s (decode) | vs Ollama |
|---|---|---|---|
| **Ollama** 0.40.8 (llama.cpp embebido) | **CUDA** | **213,5** | — |
| **Blazar** 0.31.0 (llama.cpp b12254) | **Vulkan** | 207,3 | **-3,0 %** |
| **mistral.rs** 0.9.5 | **CUDA** | 200,9 | **-5,9 %** |

Cold start (cargar y primera respuesta): Blazar ~0,95 s · Ollama ~1,16 s · mistral.rs ~5,9 s (carga eager + CUDA graphs).

Con perfil igualado a Ollama (slots=1, ctx=4096, sin speculation), Blazar dio **207,6 tok/s**
-> la diferencia **no es el tuning, es el wrapper**.

### 4.2 Benchmark a 9B — Qwen3.5-9B Q8_0 (GGUF `qwen35`, `/data/models/Qwen3.5-9b-Opus-Openclaw-Distilled-Q8_0.gguf`, 9,5 GB)

| Motor | ¿Carga? | tok/s | Capas en GPU | VRAM | Cold start |
|---|---|---|---|---|---|
| **Ollama** 0.40.8 | Sí | **30,29** | **100 %** | 8,5 GB | ~3,3 s |
| **Blazar** 0.31.0 (llama.cpp **Vulkan**) | Sí | **30,10** | Vulkan | 13,2 GB | **~47 s** |
| **mistral.rs** 0.9.5 directo | Sí | 14,78 | **25/32 CUDA + 7/32 CPU** | 8,2 GB | ~8 s (listo) |

Lecturas clave:
- **Ollama sigue ganando o empatando.** Blazar empata (-0,6 %) pero con peor VRAM y peor arranque en frío.
- **mistral.rs a 9B va 2x más lento, pero NO por el motor**: repartió 7 de 32 capas a CPU.
  Es un problema de **configuración de dispositivo**, no un defecto del motor -> test pendiente (sección 6.1).
- **Blazar y mistral.rs hacen tool-calling correctamente a 9B** (`["read_file","ssh_exec"]`).
- Los tres cargan y sirven el `qwen35` híbrido; **ninguno falló**.

### 4.3 Hallazgo clave sobre Blazar

Blazar **enruta por formato**, no por modelo:

> "GGUF files serve on the llama.cpp lane, HF-style safetensors directories serve on the mistral.rs / sglang lane"

Confirmado con `blazar plan` -> `no per-model lane resolved - the global active engine serves it`.

**Consecuencia:** la ventaja de Blazar + mistral.rs (que sí usa CUDA en esta máquina) **solo aplica
a safetensors/UQFF**. Para los GGUF actuales del usuario, Blazar no aporta nada sobre Ollama.

### 4.4 Resultado del test 6.1 — mistral.rs 9B con todas las capas en GPU (2026-11-11)

Reejecutado `mistralrs serve -f /data/models/Qwen3.5-9b-Opus-Openclaw-Distilled-Q8_0.gguf -n "0:32"` (las 32 capas a `cuda[0]`), mismo prompt, 256 tokens, `temperature: 0`:

| Métrica | mistral.rs 9B (32/32 GPU) | Ollama 0.40.8 | Nota |
|---|---|---|---|
| tok/s (decode) | **31,08 / 31,11** (2 runs) | 30,29 | Empata/supera |
| Reparto de capas | **Layers 0-31: cuda[0]** | 100 % GPU | — |
| VRAM | **13,2 GB** | ~8,5 GB | No caben 2×9B |
| Cold start | **~26 s** | ~3,3 s | Captura de CUDA graphs |
| Tool-calling | ✅ `read_file` + `ssh_exec` (paralelo) | ✅ | — |

**Conclusión:** el `14,78 tok/s` del primer test era **puro offload a CPU** (7/32 capas), no un defecto del motor. Con las 32 capas en GPU, mistral.rs **iguala y supera a Ollama** en velocidad. Pero **no es un sustituto gratuito**: 13,2 GB de VRAM (sin coexistencia), ~26 s de cold start y sin la capa de gestión de Ollama (store, keep-alive, evict). Queda como **motor complementario** para safetensors/UQFF, no como reemplazo de Ollama (D2/D4).

## 5. Evidencia — bug de CUDA en Blazar (localizado, parcheado y verificado)

### Causa raíz
En `crates/blazar-runtime/src/engine/build.rs`, `nvidia_gpu_facts()` lee el banner de `nvidia-smi`
buscando el literal `"CUDA Version:"`:

```rust
let line = text.lines().find(|l| l.contains("CUDA Version:"))?;
parse_version_pair(line.split("CUDA Version:").nth(1)?)
```

NVIDIA **renombró el campo** en drivers nuevos (615/CUDA 13.x) a **`CUDA UMD Version:`**.
`"CUDA UMD Version:"` no contiene la subcadena `"CUDA Version:"` -> `driver_cuda = None` ->
como el `match` del `doctor` exige las dos señales (driver CUDA + compute cap), cae a `warn`
-> **Blazar decide "no NVIDIA driver detected" -> Vulkan (llama.cpp) y CPU (mistral.rs)**.

Banner real de la máquina:
```
| NVIDIA-SMI 615.78.08   KMD Version: 615.78.08   CUDA UMD Version: 13.4   |
```

### Fix aplicado (local, sin commit)
Se extrae el parseo a una función testeable `parse_driver_cuda_from_banner()` que acepta
**las dos etiquetas** (la nueva primero), retrocompatible con drivers viejos. Se añaden **2 tests
unitarios** (banner nuevo -> `(13,4)`; banner viejo -> `(12,1)`).

### Verificación en vivo (antes / después)

| Check | Antes | Después |
|---|---|---|
| `doctor` -> `gpu driver` | `warn  no NVIDIA facts` | **`ok  driver CUDA 13.4, GPU sm 89`** |
| `engine update --check` (llama.cpp) | `no NVIDIA driver detected -> Vulkan` | **asset CUDA** (`llama-b12255-bin-ubuntu-cuda-12.8-x64`) |
| lane `mistral.rs` | asset **`cpu`** | asset **`cuda132-sm89`** |

Y corre en GPU de verdad: servidor arriba en 3 s, chat `"HOLA"` OK, tool-calling
`["read_file","ssh_exec"]` OK, delta de VRAM medido 2025 MiB (el modelo en la tarjeta).

### Fricciones adicionales detectadas (documentar en el PR / env)
- `blazar engine install` usa la **API de GitHub sin token** -> con 60 req/h agotadas da `403` y no
  instala el engine. Con `GH_TOKEN` (hay `gh` autenticado como `atareao`) funciona.
- El prebuilt CUDA de la lane **llama.cpp** de Blazar es solo **Vulkan** (`ubuntu-vulkan-x64`, sin
  `libggml-cuda.so`); su `engine build cuda` **falla** con el toolchain actual
  (`nvcc 13 exige gcc <= 13`, aquí hay gcc 16). El fix desbloquea CUDA **solo para la lane mistral.rs**.

## 6. Pendientes

### 6.1 Test `mistral.rs` 9B con todas las capas en GPU (decide D2/D4) — ✅ HECHO 2026-11-11
Ejecutado `mistralrs serve -f <gguf> -n "0:32"`. Resultado: **31,1 tok/s** (32/32 capas en GPU), a la par de Ollama. Detalle en §4.4.
- Resultado: **iguala ~30 tok/s** -> mistral.rs es **motor válido en GPU** para Qwen3.5 -> habilita la vía safetensors/UQFF sin Ollama.
- Matiz operativo: 13,2 GB VRAM + ~26 s cold start -> **complementario**, no sustituto de Ollama.

### 6.2 PR del fix de Blazar (repo externo, lo firma el usuario)
Flujo: `rust-writer` (dejar el parche pulido) -> `rust-reviewer` (auditar) -> conventional commit + PR con `gh`.
Debe incluir: banner real, línea culpable, antes/después, y la nota del rate-limit de la API.
**Sesión limpia (no mezclar con esta).**

### 6.3 Cerrar D4 — destino de `modelforge` y `llm-gateway` — ✅ DECIDIDO 2026-11-11
- mistral.rs funciona a 9B en GPU (31,1 tok/s) -> **`modelforge` se reduce a forja + batería comparativa**; **`llm-gateway` se mantiene** (Blazar no aporta y no se adopta).
- **Ollama sigue como motor primario GGUF** (coexistencia de 2×9B, cold start ~3,3 s). **mistral.rs** queda como **motor complementario** para safetensors/UQFF.

### 6.4 Crear `docs/<feature>.md` cuando el issue #36 se cierre.

## 7. Artefactos y rutas de referencia

- Clone de Blazar (con el fix, sin commit): `/tmp/opencode/blazar-src`
- Trial aislado de Blazar (wrapper `blz.sh`, XDG propio): `/tmp/opencode/blazar-trial`
- Binarios/engines de Blazar: `/tmp/opencode/blazar-trial/xdg-data/blazar/engines/`
  (`b12254` llama.cpp, `v0.9.5` mistralrs con asset `cuda132-sm89`)
- Motor mistral.rs del sistema: `/home/lorenzo/.local/bin/mistralrs` (v0.9.5, CUDA)
- Benchmarks: `/tmp/opencode/bench/` y `/tmp/opencode/bench9b/`
  (los artefactos de `/tmp` se limpian; el test 6.1 de 2026-11-11 usó `/tmp/opencode/bench9b/`)
- GGUF de prueba: `/tmp/opencode/gguf-spike2/qwen25-15b-q4km.gguf`,
  `/data/models/Qwen3.5-9b-Opus-Openclaw-Distilled-Q8_0.gguf`
- Config del stack actual: `~/.config/opencode/gateway.yaml`, `~/.config/opencode/opencode.jsonc`,
  `~/.config/opencode/agents/*.md`
- Documentación previa: `docs/agentes-locales-podcast.md`, `docs/coder-models-evaluation.md`

## 8. Riesgos y avisos

- **Blazar:** 2 meses de vida, **bus factor 1** (`santanu20`, 997/1013 commits), **0 stars**, 158 descargas
  en crates.io, versionado **0.x con churn fuerte** (config/API inestables). No es base para el stack.
- **mistral.rs:** su formato **UQFF** es exclusivo -> lock-in. `--paged-attn` tuvo que ir a `false`
  para cargar safetensors (memoria).
- **Ollama:** sigue siendo el punto único para GGUF; nada probado lo mejora hoy.
- Ninguna medición alteró el setup del usuario (Ollama 11434 intacto, `/var/lib/ollama` sin tocar,
  config de opencode sin tocar, VRAM restaurada en cada prueba).

## 9. Criterio de cierre del plan

- [x] 6.1 ejecutado y decisión D2/D4 tomada por escrito.
- [ ] 6.2 PR del fix abierto (o descartado explícitamente).
- [x] Decisión D4 documentada (destino de `modelforge` y `llm-gateway`).
- [ ] `docs/<feature>.md` creado; este plan renombrado a `plans/PLAN-004-ARCHIVED.md`.
