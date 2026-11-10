---
# Agentes locales en una RTX 4060 Ti — material para un episodio de podcast

Fecha: 2026-11-10 · Autor: Lorenzo Carbonell

Este documento es material de origen para un episodio del podcast sobre **dos agentes locales**: el **agente de codificación** y el **agente de tooling**. Recoge el proceso real, los datos medidos y los hallazgos.

## El escenario

- **Hardware:** NVIDIA RTX 4060 Ti **16 GB** · AMD Ryzen 7 7700X · 61 GB RAM.
- **Ollama 0.40.8** (systemd, `OLLAMA_MODELS=/var/lib/ollama`, `OLLAMA_MAX_LOADED_MODELS=1`).
- **`llm-gateway`** propio (Rust): proxy OpenAI-compatible con **failover local→OpenRouter** (`deepseek/deepseek-v4.1-flash`), escucha en `127.0.0.1:4010`.
- **`modelforge`**: CLI Rust para importar GGUFs de HuggingFace, crearlos en Ollama y enrutarlos en el gateway.

## La búsqueda: 6 modelos de código < 10 GB, rápidos y con tool-calling

Se probaron 6 modelos GGUF (todos < 10 GB) con tres pruebas: tool-calling (`message.tool_calls`), generación de código y velocidad (tok/s). Resumen del veredicto:

| Modelo | Tamaño | Arquitectura | Tools nativo | tok/s | Veredicto |
|---|---|---|---|---|---|
| Qwen2.5-Coder-7B (Q5_K_M) | 5,44 GB | qwen2 | ❌ (texto) | 51 | Eliminado |
| Qwen2.5-Coder-14B (Q4_K_M) | 9,35 GB | qwen2 | ❌ (texto) | 30 | Eliminado |
| Qwopus3.5-9B-MTP (Q5_K_M) | 6,64 GB | qwen3.5 | ✅ | 43 | **Conservado** |
| DeepSeek-Coder-V2-Lite (IQ4_XS) | 8,69 GB | deepseek2 | ❌ (sin tools) | n/a | Eliminado |
| Qwen3.5-4B-3D (Q8_0) | 4,48 GB | qwen3.5 | ✅ | 54 | Conservado |
| Strand-Rust-Coder-14B (Q4_K_M) | 9,04 GB | qwen2 | ❌ (texto) | 30 | Eliminado |

### Hallazgo estrella
**Ollama 0.40.8 no parsea los `tool_calls` de la familia qwen2/qwen2.5.** Los modelos devuelven la llamada como **texto** (`{"name":...,"arguments":...}` o `<tool-calls>...`) y Ollama no la convierte en `message.tool_calls`. Se confirmó incluso con el **`qwen2.5-coder:14b` oficial de Ollama**: es una limitación de **parseo de Ollama**, no del modelo. La familia **qwen3.5 sí emite tool-calls nativas**.

## El agente de codificación

- **Ruta del gateway:** `local-coder` → **`coder-qwopus-9b`** (Qwen3.5-9B, 6,64 GB, ~43 tok/s).
- Historia de la decisión: se partía de `qwen3-coder:30b-a3b-q4_K_M` (18–19 GB) que **no cabía** en 16 GB (forzaba offload y contaminaba benchmarks). Se consolidó a **una sola ruta de código**.
- Prueba de generación: escribe funciones Rust correctas (p. ej. `is_prime`, merge de intervalos) y **comandos de shell tersos**.
- Extra: es también el mejor para **generar comandos de shell** (uso de `dime`), mejor que `qwen2.5-coder:14b` (que añadía fences Markdown) y que `qwen3:8b` (que se desbocaba en repeticiones).

## El agente de tooling

- **Ruta del gateway:** `local-agent` → **`qwen3.5:9b-q4_K_M`** (9B, 6,6 GB, ~47,6 tok/s).
- Para **operaciones sobre servidores usando herramientas** (SSH, ficheros, email…), no para código.
- Comparativa de tool-calling (3 herramientas: `ssh_exec`, `read_file`, `send_email`; se evalúan selección, argumentos, **llamadas en paralelo** y multi-turno):

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

### Hallazgo estrella
**Solo la familia Qwen3.x emite tool-calls en paralelo** en Ollama 0.40.8. El resto (granite, gpt-oss, mistral, devstral, lfm) devuelve **una sola** llamada por turno. Los 24B ni siquiera caben limpios en 16 GB.

## El gateway consolidado

`~/.config/opencode/gateway.yaml`, seis rutas, cada una con fallback a OpenRouter:

| Ruta | Modelo local | Rol |
|---|---|---|
| `local-fast` | llama3.2:3b | rápido |
| `local-writer` | qwen3:8b | redacción |
| `local-coder` | **coder-qwopus-9b** | **codificación** (y los comandos de `dime`) |
| `local-agent` | **qwen35-9b-agent** (Qwen3.5-9B, ctx 8192) | **tooling/agente + 8 subagentes** |
| `local-deepseek` | deepseek-v4-pro-9b-heretic | — |

El **warmup/keep-alive** del gateway está activo **solo en `local-coder`** (Opción A: una sola ruta caliente). Se subió `OLLAMA_MAX_LOADED_MODELS` a **2**, de modo que `coder-qwopus-9b` y `qwen3.5:9b` pueden convivir sin que uno desaloje al otro.

## El muro: convertir safetensors a GGUF sin Python

`modelforge` **no puede** importar modelos en **safetensors** y convertirlos: en este sistema **no existen** `convert_hf_to_gguf.py` ni `llama-quantize`, y llama.cpp **no convierte AWQ**. Por eso se tiró de repos GGUF ya cuantizados (bartowski).

### ¿Hay crates Rust para hacerlo sin llama.cpp ni Ollama? Sí.

**Conversión safetensors → GGUF (puro Rust):**

| Crate | Qué hace | Notas |
|---|---|---|
| **candle** (HuggingFace) | Binario `tensor-tools`: `quantize --quantization q4k/q5k/q6k … in.safetensors --out-file out.gguf` (+ `dequantize`) | Puro Rust; cuantiza k-quants. Escribe **metadata mínima** → puede no cargar en llama.cpp/Ollama. |
| **gguf-rs** (zackshen) / **gguf-rs-lib** (ThreatFlux) | Lectura **y escritura** de GGUF v1/v2/v3, puro Rust | Base para un conversor propio. |
| **compressr** | safetensors→GGUF con Q4_0/Q8_0/**Q4_K/Q5_K/Q6_K**, presets mixtos **Q4_K_M/Q5_K_M** e **imatrix** | Reproduce `llama_tensor_get_type`. No soporta GPTQ/AWQ/EXL2. |
| **rlx-gguf-convert** | safetensors/ONNX→GGUF con cuantización por tensor | — |
| **mlmf** | Load/save/convert unificado safetensors/GGUF/ONNX/PyTorch/AWQ | Sobre `candlelight`. |
| ironvault::hf_gguf | HF→GGUF **solo llama**, a F16/BF16/F32 | **Sin k-quants** (requiere `llama-quantize`). |
| hf2q | Convert **+ serve** 100 % puro Rust ("candle divorce") | ⚠️ **Solo Apple Silicon (Metal)**. |

**Servir/inferir sin Ollama ni llama.cpp:**

| Motor | Qué aporta |
|---|---|
| **mistral.rs** (sobre candle) | Carga **safetensors directo (sin convertir)** y GGUF; **ISQ** (cuantiza al vuelo); sirve **OpenAI `/v1` + Anthropic `/v1/messages`**; **tool-calling**, MCP, web search; 45+ arquitecturas. |
| **ferrox** | Motor GGUF **puro Rust** (CPU/Metal/**CUDA**), server OpenAI-compatible, sin bindings a llama.cpp/ggml. |

### Conclusión del muro
- **`mistral.rs` resuelve el problema por la raíz**: al cargar **safetensors directamente** con ISQ, `modelforge` **no necesitaría convertir** — y además sustituye a Ollama (serving) y a llama.cpp (inferencia).
- **Lo difícil no es la inferencia, es la capa Ollama**: **plantillas de chat y parseo de tool-calls**. Es justo lo que nos falló con qwen2.5. Un motor Rust puro obliga a asumirlo.
- El verdadero coste de un conversor propio es el **mapeo de nombres por arquitectura** (miles de líneas en el script Python de llama.cpp). Los conversores Rust existentes cubren pocas arquitecturas o emiten metadata mínima; los presets mixtos/imatrix solo los reproduce `compressr`.

## Guion sugerido (esqueleto del episodio)

1. **Gancho:** "monté dos agentes locales en una GPU de 16 GB y descubrí que Ollama no sabe parsear herramientas de medio ecosistema de modelos".
2. **El caso del agente de código:** de un 30B que no cabe a un 9B que vuela; por qué Qwen2.5-Coder se cayó (parseo de Ollama).
3. **El caso del agente de tooling:** el experimento de las 3 herramientas y el hallazgo de las **llamadas en paralelo** (solo Qwen3.x).
4. **El gateway:** una ruta por rol, failover a la nube; por qué el warmup y `MAX_LOADED_MODELS=1` se pelean.
5. **El muro y la salida:** no hay conversor en el sistema → ¿crates Rust? → `candle`, `gguf-rs`, `compressr`… y sobre todo **`mistral.rs`**: cargar safetensors directo y olvidarse de Ollama/llama.cpp.
6. **Cierre/follow-ups:** portar `modelforge` a un backend `mistral.rs` (sin conversión), reimplementar plantillas y parseo de tools, y revisar Qwen2.5 cuando Ollama arregle el parser.

## Fuentes

- candle (HuggingFace): https://github.com/huggingface/candle (bin `tensor-tools`: safetensors→GGUF con k-quants)
- gguf-rs: https://github.com/zackshen/gguf · gguf-rs-lib: https://github.com/ThreatFlux/gguf
- compressr: https://docs.rs/compressr
- rlx-gguf-convert: https://docs.rs/rlx-gguf-convert
- mlmf: https://github.com/ciresnave/mlmf
- mistral.rs: https://github.com/EricLBuehler/mistral.rs
- ferrox: https://medium.com/@antonellofratepietro/ferrox-v0-9-1-a-rust-gguf-engine-measured-against-llama-cpp-251684de71d2
- hf2q (Apple Silicon, puro Rust convert+serve): https://github.com/robertelee78/hf2q

## Ver también

- `docs/coder-models-evaluation.md` — detalle de las pruebas de los 6 modelos coder (mismas pruebas, formato técnico).
---

## Configuración operativa aplicada (2026-11-10)

### Warmup solo en `local-coder` (Opción A)

Para evitar el *cold start* de `dime`, se activa el keep-alive **solo** en la ruta de código. El gateway envía un `POST /chat/completions` mínimo al arrancar y cada 4 min, reiniciando el `keep_alive` de Ollama:

```yaml
  - name: local-coder
    backend:
      - base_url: "http://localhost:11434/v1"
        model: "coder-qwopus-9b"
        timeout_ms: 120000
        warmup:
          interval_ms: 240000   # 4 min < keep_alive de Ollama (5 min)
          timeout_ms: 300000    # cold load de un 9B
```

Verificado: `llm_gateway_warmup_total{route="local-coder",result="success"} 1` y `coder-qwopus-9b` residente (~6,18 GB VRAM).

### `dime` → `local-coder`

`dime` ya apuntaba al gateway; solo cambió el modelo:

```yaml
# ~/.config/dime/dime.yml
provider: openai
base_url: 127.0.0.1:4010
use_https: false
endpoint: v1/chat/completions
token: "sk-local"
model: local-coder
```

Cadena: `dime → 127.0.0.1:4010/v1/chat/completions → ruta local-coder → coder-qwopus-9b (caliente)`.
Prueba real: `dime "…comando para uso de disco de cada subdirectorio de /var…"` → `du -sh /var/* | sort -h` (sin fences; journal: `route=local-coder`).

### `OLLAMA_MAX_LOADED_MODELS=2`

`/etc/systemd/system/ollama.service.d/override.conf`:

```ini
Environment="OLLAMA_MAX_LOADED_MODELS=2"
```

Medición con **ambos** modelos cargados simultáneamente:

| Modelo | Total | VRAM | GPU% | tok/s |
|---|---|---|---|---|
| coder-qwopus-9b | 5,65 GB | 5,65 GB | 100 % | 43 |
| qwen3.5:9b-q4_K_M | 5,83 GB | 5,83 GB | 100 % | 47 |

- VRAM: **12 978 MiB / 16 380 MiB** → ~3 GB libres. **Sin offload**, y **sin degradación** de tok/s.
- Beneficio: alternar `local-coder` y `local-agent` **ya no evicta** (fin del *thrashing*).
- Límite: solo caben **dos** 9B; un tercer modelo (o un 14B) provoca evicción.

### Backups

- `~/.config/opencode/gateway.yaml.bak-warmup-20261110091239`
- `~/.config/dime/dime.yml.bak-20261110091239`
- `/etc/systemd/system/ollama.service.d/override.conf.bak-20261110091638`

---

## Sustitución de `gpt-oss:20b` por Qwen3.5-9B (2026-11-10)

### El problema

`gpt-oss:20b` (12 GB) respaldaba **8 subagentes** (`code-verifier`, `docker-expert`, `sql-expert`, `sqlite-expert`, `postgresql-expert`, `data-persistence`, `typst-expert`, `planificador-podcast-srt`) vía la ruta `local-gptoss`. Pero **no cabe junto a `coder-qwopus-9b`**:

```
gpt-oss:20b (12 GB) + coder-qwopus-9b (5,76 GB) = 17,8 GB > 15,8 GB usables
```

Con el warmup de `local-coder`, cada 4 min se producía *thrashing*.

### Candidatos que caben junto al coder

Prueba de convivencia real (con `num_ctx=8192`):

| Candidato | VRAM junto al coder | ¿Caben? | Tools paralelo | tok/s |
|---|---|---|---|---|
| **qwen3.5:9b** | 5,34 + 5,76 = **11,1 GB** | ✅ | ✅ | 47 |
| qwen3:8b | 5,84 + 5,76 = **11,6 GB** | ✅ | ✅ | 52 |
| qwen3:14b | 9,85 GB | ❌ evicta | ✅ | 30 |
| gpt-oss:20b | 12 GB | ❌ | ❌ (1 llamada) | 65 |

### La causa raíz (hallazgo)

Con `OLLAMA_CONTEXT_LENGTH=32768`, el segundo modelo se cargaba con `num_ctx=32768` y pedía **9,2 GB**. El log de Ollama lo cantó:

```
llama-server model predicted to exceed available memory, evicting
predicted="9.2 GiB"  predicted_num_ctx=32768  available="9.8 GiB"
```

Con `num_ctx=8192`: `model fits alongside existing models` → **coexistencia**. Como el gateway no envía `num_ctx`, hay que **grabarlo en el modelo** (`PARAMETER num_ctx 8192`).

### Prueba de calidad (`think:false`, tareas de los subagentes)

| Tarea | gpt-oss:20b | **qwen3.5:9b** | qwen3:8b |
|---|---|---|---|
| SQL (top 5 clientes 2024) | ✅ (cortado) | ✅ impecable | ✅ |
| Dockerfile multi-stage Rust | ❌ **vacío** | ✅ correcto | ✅ |
| Lógica ("all but 9") | 9 | 9 | 9 |

### Decisión aplicada

- Los **8 subagentes** pasan de `gateway/local-gptoss` a **`gateway/local-agent`**.
- Se crea el modelo **`qwen35-9b-agent`** (`FROM qwen3.5:9b-q4_K_M` + `PARAMETER num_ctx 8192`); `local-agent` apunta a él.
- Se **elimina** la ruta `local-gptoss` y se **borra `gpt-oss:20b`** (~12 GB liberados).
- `opencode.jsonc` (provider `gateway`) alineado: `local-fast`, `local-writer`, `local-coder`, `local-agent`, `local-deepseek`.

**Resultado verificado:** `qwen35-9b-agent` (5,34 GB) + `coder-qwopus-9b` (5,76 GB) conviven al 100 % en GPU (12,6 GB / 16 GB); `local-agent` devuelve **tool-calls en paralelo**.

**Caveat:** en razonamiento multi-paso *duro* con el pensamiento activado, `gpt-oss:20b` (20B MoE) aún puede ser superior; se sacrifica algo de techo por convivencia y tools en paralelo.

### Backups

- `~/.config/opencode/gateway.yaml.bak-qwen35-20261110141536`
- `~/.config/opencode/opencode.jsonc.bak-agentes-20261110141602`

---
