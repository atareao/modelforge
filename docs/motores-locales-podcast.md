# Motores de inferencia locales: Ollama vs Blazar vs mistral.rs — material para un episodio de podcast

Fecha: 2026-11-11 · Autor: Lorenzo Carbonell

Este documento es material de origen para un episodio del podcast en el que se comparan **tres motores de inferencia locales** con el **mismo modelo y el mismo prompt** en una única GPU de 16 GB. Recoge el proceso real, los datos medidos y los hallazgos, incluido un **bug real de compatibilidad de driver** localizado y parcheado en un proyecto ajeno.

## El escenario

- **Hardware:** NVIDIA RTX 4060 Ti **16 GB** · AMD Ryzen 7 7700X · 61 GB RAM · driver **615.78.08** · CUDA toolkit **13.4** · compute capability **8.9**.
- **Modelos de prueba:** Qwen2.5-1.5B Q4_K_M (GGUF) y un modelo real **Qwen3.5-9B Q8_0** (9,5 GB, arquitectura `qwen35`, híbrida linear-attention + visión).
- **Motores:** **Ollama 0.40.8**, **Blazar 0.31.0** (gateway Rust que embarca llama.cpp b12254 y mistral.rs) y **mistral.rs 0.9.5** (motor sobre candle, con CUDA).
- **Método:** mismo prompt (`Write a Python function that reverses a linked list. Only code.`), 256 tokens de salida, `temperature: 0`, **VRAM liberada antes de cada medición**.

## La pregunta del episodio

1. ¿Qué motor sirve mejor un modelo real Qwen3.5 en 16 GB?
2. ¿Merece la pena **sustituir Ollama** por mistral.rs?
3. ¿Merece la pena sustituir el stack por **Blazar** (un gateway+orquestador Rust)?

## Benchmark a 1.5B — Qwen2.5-1.5B Q4_K_M (GGUF)

| Motor | Backend | tok/s (decode) | vs Ollama |
|---|---|---|---|
| **Ollama** 0.40.8 (llama.cpp embebido) | **CUDA** | **213,5** | — |
| **Blazar** 0.31.0 (llama.cpp b12254) | **Vulkan** | 207,3 | **-3,0 %** |
| **mistral.rs** 0.9.5 | **CUDA** | 200,9 | **-5,9 %** |

Cold start (cargar y primera respuesta): Blazar ~0,95 s · Ollama ~1,16 s · mistral.rs ~5,9 s (carga eager + CUDA graphs).

Con el perfil de Blazar **igualado a Ollama** (slots=1, ctx=4096, sin speculation) dio **207,6 tok/s** -> la diferencia **no es el tuning, es el wrapper**.

## Benchmark a 9B — Qwen3.5-9B Q8_0 (GGUF `qwen35`, 9,5 GB)

| Motor | ¿Carga? | tok/s | Capas en GPU | VRAM | Cold start |
|---|---|---|---|---|---|
| **Ollama** 0.40.8 | Sí | **30,29** | **100 %** | 8,5 GB | ~3,3 s |
| **Blazar** 0.31.0 (llama.cpp **Vulkan**) | Sí | **30,10** | Vulkan | 13,2 GB | **~47 s** |
| **mistral.rs** 0.9.5 directo (25/32 GPU) | Sí | 14,78 | **25/32 CUDA + 7/32 CPU** | 8,2 GB | ~8 s (listo) |
| **mistral.rs** 0.9.5 forzado (`-n 0:32`) | Sí | **31,08 / 31,11** | **32/32 CUDA** | **13,2 GB** | **~26 s** |

Lecturas clave:
- **Ollama sigue ganando o empatando** en la vía GGUF: estable, 100 % GPU, 8,5 GB y arranque en frío de ~3,3 s.
- **Blazar empata en velocidad** (-0,6 %) pero con **peor VRAM** (13,2 GB) y un **arranque en frío de ~47 s**.
- Los tres motores cargan y sirven el `qwen35` híbrido; **ninguno falló**.

## Hallazgo estrella 1 — el offload silencioso de mistral.rs

El primer test de mistral.rs a 9B dio **14,78 tok/s**, la mitad que Ollama. La tentación es culpar al motor. Pero el log lo delataba:

```
Layers 0-24: cuda[0]   (25 capas)
Layers 25-31: cpu      (7 capas)
```

mistral.rs había **repartido 7 de 32 capas a la CPU** por defecto. No era un defecto del motor, era **mapeo de dispositivo**. Forzando **todas** las capas a GPU (`mistralrs serve -f <gguf> -n "0:32"`):

```
Model has 32 repeating layers.
Layers 0-31: cuda[0] (16 GB)
```

-> **31,08 / 31,11 tok/s**, a la par de Ollama (30,29). **La lentitud era puro offload**, no el motor. Tool-calling a 9B correcto y **en paralelo** (`["read_file","ssh_exec"]`, `finish_reason: tool_calls`).

**Moraleja del episodio:** antes de descartar un motor por lentitud, mira el **reparto de capas GPU/CPU**. Un `14,78 tok/s` se convirtió en `31,1` cambiando una bandera.

### El matiz operativo (que decide la adopción)

mistral.rs iguala en velocidad, pero **no es un sustituto gratuito de Ollama**:

| Criterio | Ollama | mistral.rs (32/32 GPU) |
|---|---|---|
| tok/s | 30,3 | **31,1** |
| VRAM | ~8,5 GB | **13,2 GB** |
| Cold start | ~3,3 s | **~26 s** |
| Coexistencia de 2×9B | ✅ | ❌ (no caben) |
| Capa de gestión (store, keep-alive, evict) | ✅ | ❌ (hay que reconstruirla) |

## Hallazgo estrella 2 — Blazar enruta por formato, no por modelo

Blazar decide el motor **según el formato del fichero**, no según el modelo:

> "GGUF files serve on the llama.cpp lane, HF-style safetensors directories serve on the mistral.rs / sglang lane"

Confirmado con `blazar plan` -> `no per-model lane resolved - the global active engine serves it`.

**Consecuencia:** la ventaja de Blazar + mistral.rs (que sí usa CUDA en esta máquina) **solo aplica a safetensors/UQFF**. Para los GGUF actuales, Blazar **no aporta nada** sobre Ollama.

## Hallazgo estrella 3 — un bug real de driver en Blazar (nvidia-smi)

Blazar decidía "no NVIDIA driver detected" y caía a **Vulkan (llama.cpp) y CPU (mistral.rs)**, dejando la GPU sin CUDA. La causa estaba en `crates/blazar-runtime/src/engine/build.rs`, que lee el banner de `nvidia-smi` buscando el literal `"CUDA Version:"`:

```rust
let line = text.lines().find(|l| l.contains("CUDA Version:"))?;
parse_version_pair(line.split("CUDA Version:").nth(1)?)
```

Pero **NVIDIA renombró el campo** en drivers nuevos (615 / CUDA 13.x) a **`CUDA UMD Version:`**. El literal viejo no casa -> `driver_cuda = None` -> Blazar concluye que no hay GPU NVIDIA.

Banner real de la máquina:

```
| NVIDIA-SMI 615.78.08   KMD Version: 615.78.08   CUDA UMD Version: 13.4   |
```

**Fix:** extraer el parseo a una función testeable `parse_driver_cuda_from_banner()` que acepta **las dos etiquetas** (la nueva primero), retrocompatible, con 2 tests unitarios. Verificación antes/después:

| Check | Antes | Después |
|---|---|---|
| `doctor` -> `gpu driver` | `warn  no NVIDIA facts` | **`ok  driver CUDA 13.4, GPU sm 89`** |
| `engine update --check` (llama.cpp) | `no NVIDIA driver detected -> Vulkan` | **asset CUDA** (`llama-b12255-bin-ubuntu-cuda-12.8-x64`) |
| lane `mistral.rs` | asset **`cpu`** | asset **`cuda132-sm89`** |

Y corre en GPU de verdad: servidor arriba en 3 s, chat OK, tool-calling OK, delta de VRAM 2025 MiB.

### Fricciones adicionales detectadas (para el episodio y para el PR)

- `blazar engine install` usa la **API de GitHub sin token** -> con 60 req/h agotadas da **403** y no instala. Con `GH_TOKEN` funciona.
- El prebuilt CUDA de la lane **llama.cpp** de Blazar es solo **Vulkan** (sin `libggml-cuda.so`); su `engine build cuda` **falla** con el toolchain actual (`nvcc 13` exige `gcc <= 13`, aquí hay gcc 16). El fix desbloquea CUDA **solo para la lane mistral.rs**.

## ¿Y merece la pena cambiar el stack? (la decisión)

| Pregunta | Respuesta | Motivo |
|---|---|---|
| ¿Ollama fuera? | **No** | Sigue ganando o empatando; estable e integrado. |
| ¿mistral.rs? | **Sí, como motor complementario** | Única vía para safetensors/UQFF nativo (Ollama no puede). Iguala en tok/s, pero 13,2 GB y ~26 s cold start -> **no reemplaza** a Ollama. |
| ¿Blazar? | **No se adopta** | No aporta velocidad en GGUF, enruta GGUF a llama.cpp (no a mistral.rs), 47 s cold start, proyecto de 2 meses con bus factor 1. |

**Resultado:** Ollama se queda como **motor primario GGUF**; mistral.rs queda como **motor complementario** para safetensors/UQFF (habilita cargar Qwen3.5 sin convertir a GGUF); el `llm-gateway` propio se mantiene; `modelforge` se reduce a **forja + batería comparativa**.

## Guion sugerido (esqueleto del episodio)

1. **Gancho:** "puse tres motores de inferencia a correr el mismo modelo en una GPU de 16 GB, y el más lento tenía un bug que cabía en una línea".
2. **El método:** mismo prompt, temperatura 0, VRAM limpia antes de cada medición. Por qué medir importa más que opinar.
3. **Los tres candidatos:** Ollama (el de siempre), Blazar (el nuevo gateway Rust), mistral.rs (el motor sobre candle).
4. **El caso mistral.rs:** de 14,78 a 31,1 tok/s por **7 capas mal repartidas**. El offload silencioso.
5. **El caso Blazar:** enruta por formato, no por modelo; y el **bug de `nvidia-smi`** (`CUDA Version:` -> `CUDA UMD Version:`). Cómo se localiza, se parchea y se verifica en vivo.
6. **La letra pequeña:** por qué "igualar en tok/s" no basta — VRAM, cold start y capa de gestión mandan.
7. **Cierre:** Ollama primario, mistral.rs complementario, Blazar descartado; y el **PR del fix de driver** al repo externo.

## Fuentes

- Blazar: https://github.com/santanu20/blazar
- mistral.rs: https://github.com/EricLBuehler/mistral.rs
- Ollama: https://github.com/ollama/ollama
- llama.cpp: https://github.com/ggml-org/llama.cpp
- candle (base de mistral.rs): https://github.com/huggingface/candle

## Ver también

- `plans/PLAN-004.md` — el plan de evaluación con todas las tablas y decisiones (D1–D5).
- `docs/coder-models-evaluation.md` — evaluación de modelos de código locales GGUF sobre Ollama.
- `docs/agentes-locales-podcast.md` — material del episodio hermano: agentes local de codificación y de tooling.
