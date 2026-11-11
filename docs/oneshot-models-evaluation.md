# Evaluación de modelos ligeros para tareas one-shot (dime, commits, resúmenes)

Fecha: 2026-11-11 · Autor: Lorenzo Carbonell
Issue: [#38](https://github.com/atareao/modelforge/issues/38) · Plan: `plans/PLAN-005.md`

## Objetivo

Elegir **un modelo mucho más ligero** que el actual para tareas **one-shot de salida corta**, y enrutarlo en el gateway:

1. **`dime`** — generar un comando de shell en una línea, sin markdown.
2. **Commit messages** — conventional commit + gitmoji.
3. **Resumen ~50 palabras** — descripciones para YouTube.
4. **Respuesta breve** — contestación corta y con tono.

Hoy `dime` usa **`coder-qwopus-9b`** (Qwen3.5-9B, 6,6 GB), desproporcionado para estas tareas.

## Entorno de pruebas

- **GPU:** NVIDIA RTX 4060 Ti **16 GB** · AMD Ryzen 7 7700X · 61 GB RAM.
- **Ollama 0.40.8** (`OLLAMA_MAX_LOADED_MODELS=2`, `OLLAMA_CONTEXT_LENGTH=32768`).
- Cada prueba con `num_ctx: 8192`, `num_predict: 512`, `think: false`, `temperature: 0` (y `temperature: 1` en la prueba de respuesta breve, como `dime`).

## Shortlist (11 modelos)

| Tier | Modelo | Tamaño |
|---|---|---|
| ~0.5B | `qwen2.5:0.5b` | 397 MB |
| ~0.5B | `qwen3:0.6b` | 522 MB |
| ~1B | `gemma3:1b` | 806 MB |
| ~1B | `qwen2.5:1.5b` | 986 MB |
| ~1B | `qwen2.5-coder:1.5b` | 986 MB |
| ~1B | `llama3.2:1b` | 1,3 GB |
| ~1B | `qwen3:1.7b` | 1,4 GB |
| 3-4B | `llama3.2:3b` | 2,0 GB |
| 3-4B | `qwen3:4b` | 2,5 GB |
| 3-4B | `gemma3:4b` | 3,4 GB |
| Ref. | `coder-qwopus-9b` | 6,6 GB |

## Métricas (decode en caliente)

| Modelo | tok/s | Carga en frío (ms) |
|---|---|---|
| `qwen2.5:0.5b` | ~400-418 | 5 520 |
| `qwen3:0.6b` | ~360-410 | 5 775 |
| `gemma3:1b` | ~224-239 | 1 019 |
| `qwen2.5:1.5b` | ~204-227 | 1 025 |
| `qwen2.5-coder:1.5b` | ~201-226 | 1 026 |
| `llama3.2:1b` | ~175-200 | 1 029 |
| `qwen3:1.7b` | ~172-210 | 2 961 |
| `llama3.2:3b` | ~114-127 | 5 293 |
| `gemma3:4b` | ~88-96 | 3 160 |
| `qwen3:4b` | ~87-93 | 1 273 |
| `coder-qwopus-9b` | ~43-49 | 1 533 |

Residencia: con `OLLAMA_MAX_LOADED_MODELS=2` caben **dos** modelos ≤1,5 GB sobrados (p. ej. `qwen3:0.6b` a 8192 ctx ≈ 1,5 GB con buffers). Los candidatos < 1,5 GB **no desalojan** a los 9B si se respeta el contexto.

## Resultados por prueba

### 1. Shell (`dime`) — 5 tareas, comando correcto y sin fences

| Modelo | Aciertos /5 | Sin fences | Nota |
|---|---|---|---|
| `coder-qwopus-9b` (ref) | **5/5** | ✅ | impecable |
| `qwen2.5-coder:1.5b` | **5/5** | ⚠️ 4/5 con ```` ```bash ```` | el mejor <1 GB |
| `gemma3:4b` | 4/5 | ⚠️ parcial | buen `du`/`find` |
| `qwen2.5:1.5b` | 3/5 | ⚠️ | `ls -l | awk` no lista subdirs |
| `llama3.2:3b` | 3/5 | ❌ backticks | `netstat | grep ESTABLISHED` incorrecto |
| `qwen3:1.7b` | 2/5 | ✅ | `mv *.jpeg *.jpg` incorrecto |
| `gemma3:1b` | 2/5 | ⚠️ | `netstat` devolvió números ("2049,4000"); rename con `convert` |
| `qwen2.5:0.5b` | 2/5 | ❌ | `lsof -i:80` (puerto fijo); `mv {} {}.jpg` genera `.jpeg.jpg` |
| `qwen3:0.6b` | 2/5 | ✅ | `sort /var/` y `tar.gz` inválidos |
| `llama3.2:1b` | 1/5 | ❌ | alucinó `dirstat`; varias opciones |
| `qwen3:4b` | 0/5 | — | **filtra razonamiento** (ver hallazgos) |

Ejemplos (comando correcto):
- `/var` ordenado: `coder-qwopus-9b` → `du -sh /var/* | sort -rh` · `qwen2.5-coder:1.5b` → `du -sh /var/* | sort -rh`.
- Renombrar `.jpeg`→`.jpg`: `for f in *.jpeg; do mv "$f" "${f%.jpeg}.jpg"; done` (`qwen2.5-coder:1.5b`).

### 2. Commit (conventional commits + gitmoji) — 3 diffs reales

| Modelo | Formato OK /3 | Nota |
|---|---|---|
| `coder-qwopus-9b` (ref) | **3/3** | perfecto |
| `qwen2.5:1.5b` | 2/3 | sin emoji en el `fix` |
| `llama3.2:3b` | 2/3 | sin emoji en el 1º; backticks |
| `qwen2.5-coder:1.5b` | 1/3 | fences; sin emoji en el 1º; scope anidado `fix(docs(agentes))` |
| `qwen3:1.7b` | 2/3 | alucina contenido del diff; emoji `🟥` para docs |
| `gemma3:4b` | 2/3 | emoji erróneo (`🎉` en un `fix`) |
| `llama3.2:1b` | 1/3 | invierte "8 → 93 GB" |
| `gemma3:1b` | 0/3 | eco del diff |
| `qwen2.5:0.5b` | 0/3 | responde con prosa/instrucciones |
| `qwen3:0.6b` | 0/3 | eco de la cabecera `commit <sha>` |
| `qwen3:4b` | 0/3 | razonamiento filtrado |

### 3. Resumen ≤ 50 palabras — 3 textos

| Modelo | ≤50w OK /3 | Nota |
|---|---|---|
| `coder-qwopus-9b` (ref) | **3/3** | excelente y con gancho |
| `gemma3:4b` | **3/3** | bueno, estilo YouTube |
| `qwen3:1.7b` | **3/3** | correcto |
| `llama3.2:3b` | **3/3** | correcto |
| `qwen2.5:1.5b` | **3/3** | muy conciso |
| `gemma3:1b` | 3/3 | 51 palabras en 1 (borderline) |
| `qwen2.5-coder:1.5b` | 2/3 | 77 palabras en el 1º (se pasa) |
| `qwen3:0.6b` | 2/3 | 72 palabras en el 1º |
| `qwen2.5:0.5b` | 1/3 | **alucina** ("proyecto de videoconferencia") |
| `llama3.2:1b` | 0/3 | rechazos ("No puedo cumplir…") |
| `qwen3:4b` | 0/3 | 300+ palabras de razonamiento |

### 4. Respuesta breve (tono "Leslie", concisa) — 3 preguntas

| Modelo | OK /3 | Nota |
|---|---|---|
| `gemma3:4b` | **3/3** | **mejor tono** (ácido, breve) |
| `qwen3:1.7b` | 3/3 | tono correcto |
| `coder-qwopus-9b` (ref) | 3/3 | con gracia, pero **largo** (116/125 palabras) |
| `llama3.2:3b` | 2/3 | alucina "resolvemos" |
| `qwen2.5-coder:1.5b` | 2/3 | 1ª respuesta divaga; `:angry:` en texto |
| `qwen2.5:1.5b` | 0/3 | **error factual** ("contenedor HTML"); sin emoji; divaga |
| `gemma3:1b` | 1/3 | tono bien, contenido sin sentido ("pared digital para tmux") |
| `llama3.2:1b` | 1/3 | "Terminal Multi-User" (falso) |
| `qwen2.5:0.5b` | 0/3 | respuestas vacías o inconexas |
| `qwen3:0.6b` | 0/3 | "un buche de la computadora" |
| `qwen3:4b` | 0/3 | razonamiento filtrado |

## Tabla resumen

> La columna "Commit" cuenta **emoji + tipo(scope) válido**. El veredicto final (§Decisión) prevalece sobre esta tabla orientativa.

| Modelo | Tamaño | Shell /5 | Commit /3 | Resumen /3 | Breve /3 | **Total /14** |
|---|---|---|---|---|---|---|
| `coder-qwopus-9b` (ref) | 6,6 GB | 5 | 3 | 3 | 3 | **14** |
| `gemma3:4b` | 3,4 GB | 4 | 2 | 3 | 3 | **12** |
| `llama3.2:3b` | 2,0 GB | 3 | 2 | 3 | 2 | **10** |
| `qwen3:1.7b` | 1,4 GB | 2 | 2 | 3 | 3 | **10** |
| `qwen2.5-coder:1.5b` | 986 MB | 5 | 1 | 2 | 1 | **9** |
| `qwen2.5:1.5b` | 986 MB | 3 | 2 | 3 | 0 | **8** |
| `gemma3:1b` | 806 MB | 2 | 0 | 3 | 1 | **6** |
| `qwen3:0.6b` | 522 MB | 2 | 0 | 2 | 0 | **4** |
| `qwen2.5:0.5b` | 397 MB | 2 | 0 | 1 | 0 | **3** |
| `llama3.2:1b` | 1,3 GB | 1 | 1 | 0 | 1 | **3** |
| `qwen3:4b` | 2,5 GB | 0 | 0 | 0 | 0 | **0** |

## Hallazgos

1. **Por debajo de ~1B la fidelidad se rompe.** `llama3.2:1b`, `qwen3:0.6b` y `qwen2.5:0.5b` alucinan comandos (`dirstat`, `sort /var/`), rechazan resúmenes o devuelven frases vacías. No sirven para producción.
2. **`qwen3:4b` es inservible para one-shot.** Filtra su cadena de razonamiento al contenido ("Okay, the user wants…") **incluso con `think:false`**, con 5-11 s de latencia y respuestas de 300+ palabras. Verificado de forma directa contra `/api/chat`.
3. **Los *code fences* son un problema de prompt, no del modelo.** `qwen2.5-coder:1.5b` metía ```` ```bash ```` en 4/5 comandos; con un **system prompt duro** ("sin backticks, sin bloques de código, una línea") devuelve el comando **limpio**:
   > `du -sh /var/* | sort -rh`
   Se arregla **grabándolo en un Modelfile**.
4. **`gemma3:4b` es el mejor todoterreno "ligero".** 12/14, con el **mejor tono** para respuestas breves y buena calidad en todo; a 3,4 GB es la mitad del 9B actual y ~2× más rápido que él.
5. **`qwen2.5-coder:1.5b` es el rey del shell.** 5/5 comandos correctos con **986 MB** y ~210 tok/s; su punto débil es el resumen (se pasa de 50 palabras en 1 caso).
6. **El 9B sigue siendo el más fiable** (14/14) —pero a 45 tok/s, 6,6 GB, y **demasiado largo** para "respuesta breve" (116/125 palabras). Para one-shot, su calidad es un desperdicio.
7. **Un detalle de infraestructura:** con `num_ctx` mal fijado los modelos pequeños también rotan con `MAX_LOADED_MODELS=2`; con 8192 el footprint real de un ≤1,5B ronda 1-2 GB y **convive** con los 9B.

## Decisión (aplicada 2026-11-11)

Tras la **verificación en vivo** (más abajo), se **descarta `qwen2.5:1.5b` para `dime`** y se **unifica en `gemma3:4b`**:

- **`local-oneshot` → `gemma3:4b`**: tareas one-shot de texto (resúmenes, respuestas).
- **`local-dime` → `gemma3:4b`** (con warmup): comando de shell para `dime` **y** su persona ácida.
- **`dime` → `local-dime`**; `system_role` ampliado con "sin bloques de código ni backticks".
- **`qwen2.5:1.5b`**: descartado para `dime` (contexto Windows, fences, erratas). Solo válido como escritor de resúmenes barato.
- **`qwen2.5-coder:1.5b`**: el mejor en shell puro (5/5) pero **sin persona**; queda como alternativa "seca" a 986 MB.
- El 9B (`coder-qwopus-9b`) se mantiene para `rust-writer` y tareas que exijan fiabilidad.

## Verificación en vivo (2026-11-11)

Prueba real **por el gateway** (no en el harness), con la persona de Leslie:

| Prompt | `qwen2.5:1.5b` | `qwen2.5-coder:1.5b` | `gemma3:4b` |
|---|---|---|---|
| ficheros >100 MB | ✅ con prosa + fence | ✅ (fence) | ✅ **+ tono + emoji** |
| puertos TCP | ❌ **Windows** (`netstat -ano \| findstr`) | ✅ `sudo netstat -tuln` | ✅ `sudo netstat -tulpn` |
| tar.gz | ⚠️ errata (`nombre_de_votre_tar.gz`) | ✅ `🚀 tar -czvf ...` | ✅ `tar -czvf ...` |

**`qwen2.5:1.5b` falla como generador de comandos** (1/3, con contexto Windows y fences). `gemma3:4b` es el único que cumple **comando + persona + emoji**.

Comprobación tras aplicar la config (vía `dime` real, sin bloques de código):
```
🙄 ¡Qué desastre! Aquí tienes: `du -hsx /var | sort -rh | head -10`
🙄 ¿En serio? Necesitas que te diga eso?
netstat -tulnp
```
Latencia por el gateway: ~200-990 ms en caliente (cold ~6,7 s).

## Aplicación (hecha)

1. `~/.config/opencode/gateway.yaml`: rutas `local-dime` (`gemma3:4b` + warmup) y `local-oneshot` (`gemma3:4b`). Gateway reiniciado (`active`).
2. `~/.config/dime/dime.yml`: `model: local-dime` + `system_role` con "sin bloques de código ni backticks".
3. Backups: `gateway.yaml.bak-oneshot-20261111070051`, `dime.yml.bak-oneshot-20261111070051`, `dime.yml.bak-fence-20261111070252`.

## Ver también

- `plans/PLAN-005.md` — plan de esta evaluación.
- `docs/coder-models-evaluation.md` — evaluación de modelos de código (tool-calling, 2026-11-09).
- `docs/motores-locales-podcast.md` — evaluación de motores (Ollama/Blazar/mistral.rs).
- `docs/agentes-locales-podcast.md` — episodio hermano: agentes de código y tooling.
