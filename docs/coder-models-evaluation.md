# Evaluación de modelos de código locales (GGUF)

Fecha: 2026-11-09/10 · Autor: Lorenzo Carbonell

## Objetivo

Seleccionar modelos **especializados en programación**, **rápidos** y que **manejen herramientas (tool-calling)**, con un tamaño **< 10 GB**, para uso local con Ollama.

## Entorno de pruebas

- GPU: **NVIDIA RTX 4060 Ti 16 GB** · CPU: AMD Ryzen 7 7700X (16 hilos) · RAM: 61 GB.
- Ollama **0.40.8** (servicio systemd, `OLLAMA_MAX_LOADED_MODELS=1`).
- Para benchmarks limpios se detuvo temporalmente `llm-gateway.service` (que mantenía `qwen3-coder:30b` en VRAM).

## Metodología

1. **Descarga:** `modelforge import <repo> --quant <QUANT> --dest /data/ollama-imports`
   (se usaron repos **bartowski** para los Qwen, porque `quant::select` no distingue ficheros partidos `-00001-of-00002`).
2. **Creación:** `ollama create <nombre> -f Modelfile` con `FROM <gguf>` + `PARAMETER num_ctx 8192` (plantilla embebida del GGUF).
3. **Tool-calling:** `POST /api/chat` con una herramienta `get_weather` y prompt que la exige; se comprueba `message.tool_calls`.
4. **Código:** `POST /api/generate` con `think:false`, `temperature:0`, prompt "Write only a Rust function is_prime".
5. **Velocidad:** `modelforge bench` (tokens/s) y stats `eval_count`/`eval_duration`.

## Resultados

| # | Modelo | Repo · quant | Tamaño | Arquitectura | Tools (nativo) | tok/s | VRAM | Código | Veredicto |
|---|---|---|---|---|---|---|---|---|---|
| 1 | Qwen2.5-Coder-7B | bartowski · Q5_K_M | 5.44 GB | qwen2 | ❌ (sale como texto) | 51 | 5.9 GB | ✅ | **Eliminado** |
| 2 | Qwen2.5-Coder-14B | bartowski · Q4_K_M | 9.35 GB | qwen2 | ❌ (sale como texto) | 30 | 10.4 GB | ✅ | **Eliminado** |
| 3 | Qwopus3.5-9B-MTP | Jackrong · Q5_K_M | 6.64 GB | qwen35 | ✅ `get_weather` | 43 | 6.2 GB | ✅ | **Conservado** |
| 4 | DeepSeek-Coder-V2-Lite | bartowski · IQ4_XS | 8.69 GB | deepseek2 | ❌ (sin tools) | n/a | 11.0 GB | ✅ | **Eliminado** |
| 5 | Qwen3.5-4B-3D-Coder | ccore · Q8_0 | 4.48 GB | qwen35 | ✅ `get_weather` | 54 | 4.8 GB | ✅ | **Conservado** |
| 6 | Strand-Rust-Coder-14B | Fortytwo · Q4_K_M | 9.04 GB | qwen2 | ❌ (sale como texto) | 30 | 10.5 GB | ✅ | **Eliminado** |

## Hallazgos

- **Ollama 0.40.8 no parsea `tool_calls` de la familia qwen2/qwen2.5**: los modelos devuelven la llamada como **texto** (`{"name":"get_weather","arguments":{...}}` o `<tool-calls>...`) y Ollama no la convierte en `message.tool_calls`. Se confirmó también con el **`qwen2.5-coder:14b` oficial de Ollama**: es una limitación de parseo de Ollama para qwen2/qwen2.5, no del modelo.
- La familia **qwen3.5** sí emite **tool_calls nativas** ✅ (validado además con `qwen3:8b` y `qwen3:14b`).
- **DeepSeek-Coder-V2-Lite-Instruct** no soporta tool-calling (contenido nulo con `tools`).
- Los modelos **Qwen3.5 divagan** si no se usa `think:false`; con razonamiento desactivado generan código correcto.

## Decisiones

- **Conservados** (cumplen tools + rápido + < 10 GB + coding): `coder-qwopus-9b`, `coder-qwen35-4b-3d`.
- **Eliminados** (no cumplen): `coder-qwen25-7b`, `coder-qwen25-14b`, `coder-dscoder-v2-lite`, `coder-strand-rust-14b`.

## Uso

```bash
# Qwen3.5 4B (4.5 GB, 54 tok/s)
ollama run coder-qwen35-4b-3d
# Qwopus 9B (6.6 GB, 43 tok/s)
ollama run coder-qwopus-9b
```

## Follow-ups

- Issue potencial en `modelforge`/Ollama: soporte de tool-calling para qwen2/qwen2.5 (template/parser).
- Repetir la evaluación cuando Ollama añada un parser qwen2.5 con tools, para reconsiderar Qwen2.5-Coder.

---

## Consolidación de rutas del gateway (2026-11-10)

Tras la evaluación se consolidó `~/.config/opencode/gateway.yaml` a **una única ruta de código**:

- `local-coder` → su modelo local cambió de `qwen3-coder:30b-a3b-q4_K_M` (19 GB, no cabe en 16 GB, forzaba offload) a **`coder-qwopus-9b`** (Qwen3.5-9B, 6,6 GB, cabe limpio).
- Se eliminó la ruta `local-code` (gemma4:12b): era generalista, no especializada en código, y solapaba con `local-coder`.
- Se borraron de Ollama `qwen3-coder:30b-a3b-q4_K_M` y `coder-qwen35-4b-3d` (~23,5 GB liberados), al quedar sin ruta.

Rutas finales del gateway: `local-fast`, `local-writer`, `local-gptoss`, `local-coder`, `local-agent`, `local-deepseek`.

## Ruta de tooling/agente: `local-agent` (2026-11-10)

Para **operaciones sobre servidores usando herramientas** se añadió la ruta `local-agent` → **`qwen3.5:9b-q4_K_M`**. Se comparó el tool-calling de los modelos locales con 3 herramientas (`ssh_exec`, `read_file`, `send_email`): selección correcta, extracción de argumentos, **llamadas en paralelo** y respuesta multi-turno (resultado de tool → respuesta final sin re-llamadas).

| Modelo | Tamaño | Selección | Args | Paralelo | Multi-turno | tok/s |
|---|---|---|---|---|---|---|
| **qwen3.5:9b-q4_K_M** | 6,6 GB | ✅ | ✅ | ✅ (2) | ✅ | 47,6 |
| qwen3:14b | 9,3 GB | ✅ | ✅ | ✅ (2) | ✅ | 30,0 |
| qwen3:8b | 5,2 GB | ✅ | ✅ | ✅ (2) | ✅ | 52,2 |
| coder-qwopus-9b | 6,6 GB | ✅ | ✅ | ✅ (2) | ✅ | 43,9 |
| granite4.2:8b | 5,3 GB | ✅ | ✅ | ❌ (1) | — | — |
| gpt-oss:20b | 13 GB | ✅ | ✅ | ❌ (1) | — | — |
| mistral-small3.2:24b | 15 GB | ✅ | ✅ | ❌ (1) | — | — |
| devstral-small-2:24b | 15 GB | ✅ | ✅ | ❌ (1) | — | — |
| lfm2.5:24b-a2b | 14 GB | ✅ | ✅ | ❌ (1) | — | — |

**Hallazgo:** solo la familia **Qwen3.x** (qwen3, qwen3.5) emite **tool-calls en paralelo** en Ollama 0.40.8; el resto de modelos solo devuelve una llamada por turno. Los modelos de 24B (mistral, devstral, lfm) además no caben limpios en 16 GB (offload).

**Decisión:** `local-agent` → `qwen3.5:9b-q4_K_M` (9B, 6,6 GB, 47,6 tok/s, tools nativas + paralelo).

## Uso de la ruta de agente

```bash
# operaciones sobre servidores con herramientas
curl -s http://127.0.0.1:4010/v1/chat/completions -H 'Content-Type: application/json' \
  -d '{"model":"local-agent","messages":[{"role":"user","content":"..."}],"tools":[...]}'
```

---
