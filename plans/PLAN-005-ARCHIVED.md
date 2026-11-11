# PLAN-005 — Evaluación de un modelo ligero para tareas one-shot (dime, commits, resúmenes)

- **Issue:** [#38](https://github.com/atareao/modelforge/issues/38)
- **Fecha:** 2026-11-11
- **Estado:** En curso
- **Rama:** `development`
- **Ámbito:** evaluación / decisión de rutas del gateway (no toca el binario `modelforge`)

## 1. Objetivo

Elegir **un modelo mucho más ligero** que el actual (`dime` usa hoy `coder-qwopus-9b`, 6,6 GB) para tareas **one-shot de salida corta**, y enrutarlo en el gateway:

1. **`dime`** — generar un comando de shell en una línea, **sin fences markdown**.
2. **Commit messages** — conventional commit + gitmoji (formato del repo).
3. **Resumen ~50 palabras** — descripciones/textos para YouTube.
4. **Respuesta breve** — contestación corta y directa (con el role de `dime`).

Requisito clave: **< 3 GB** (idealmente ~1B) para quedar **residente** en 16 GB sin desalojar otros modelos.

## 2. Estado de partida

- `dime` → `local-coder` → **`coder-qwopus-9b`** (Qwen3.5-9B, 6,6 GB). Desproporcionado para un comando de shell.
- Gateway con 5 rutas: `local-fast` (`llama3.2:3b`), `local-writer` (`qwen3:8b`), `local-coder` (`coder-qwopus-9b`), `local-agent` (`qwen35-9b-agent`), `local-deepseek`.
- Hardware: **RTX 4060 Ti 16 GB** · Ollama **0.40.8** (`OLLAMA_MAX_LOADED_MODELS=2`).
- Modelos ya presentes: `llama3.2:3b`, `qwen3:8b`, `qwen35-9b-agent`, `coder-qwopus-9b` (+ otros).

## 3. Modelos a probar

| Tier | Modelo | Tamaño aprox. | Nota |
|---|---|---|---|
| ~0.5B | `qwen2.5:0.5b` | ~0,4 GB | mínimo |
| ~0.5B | `qwen3:0.6b` | ~0,5 GB | reasoning opcional |
| ~1B | `llama3.2:1b` | ~1,3 GB | — |
| ~1B | `gemma3:1b` | ~0,8 GB | multilingual |
| ~1B | `qwen2.5:1.5b` | ~1,0 GB | — |
| ~1B | `qwen2.5-coder:1.5b` | ~1,0 GB | especializado código/shell |
| ~1B | `qwen3:1.7b` | ~1,4 GB | reasoning |
| 3-4B | `llama3.2:3b` | ~2,0 GB | **referencia** (actual `local-fast`) |
| 3-4B | `qwen3:4b` | ~2,6 GB | techo calidad |
| 3-4B | `gemma3:4b` | ~3,3 GB | techo calidad |
| Ref. | `coder-qwopus-9b` | 6,6 GB | **referencia** (actual `dime`) |

## 4. Metodología

1. **Descarga:** `ollama pull <modelo>`.
2. **Contexto:** fijar `num_ctx 8192` en la llamada (o crear variante con `PARAMETER num_ctx 8192`).
3. **Ejecución:** `POST /api/chat` con `temperature: 0` (salvo el test de respuesta breve, que usa `temperature: 1` como `dime`), `think: false` cuando el modelo lo soporte.
4. **Medición:** una pasada en frío (tras liberar VRAM) y otra en caliente.

## 5. Batería de pruebas

### 5.1 Shell (`dime`) — 5 tareas, solo el comando, sin fences
1. `comando para ver el espacio que ocupa cada subdirectorio de /var ordenado de mayor a menor`
2. `buscar en /home todos los ficheros mayores de 100 MB`
3. `listar los puertos TCP en escucha`
4. `comprimir el directorio proyecto en un tar.gz`
5. `renombrar todos los .jpeg de una carpeta a .jpg`

*Se evalúa:* comando correcto, **una línea**, sin markdown/fences, sin prosa.

### 5.2 Commit — 3 diffs reales
Prompt: `Genera SOLO el mensaje de commit (formato: "emoji tipo(scope): descripción") para este diff:
<diff>`

*Se evalúa:* formato exacto (emoji + `tipo(scope):`), fidelidad al diff, longitud.

### 5.3 Resumen ~50 palabras — 3 textos/temas
Prompt: `Resume en un máximo de 50 palabras, en español, para la descripción de un vídeo de YouTube: <texto>`

*Se evalúa:* **≤ ~50 palabras**, español correcto, fidelidad.

### 5.4 Respuesta breve — 3 preguntas
Con el role de `dime` (asistente ácida, irónica, breve, con emoji inicial):
1. `¿qué es un contenedor?`
2. `¿merece la pena systemd-resolved?`
3. `dime en una frase para qué sirve tmux`

*Se evalúa:* concisión, tono, español.

## 6. Métricas

| Métrica | Fuente |
|---|---|
| tok/s (decode) | `eval_count` / `eval_duration` de `/api/chat` |
| latencia total (s) | cronómetro sobre la respuesta completa |
| VRAM (MB) | `ollama ps` |
| Residencia | ¿cabe junto a `coder-qwopus-9b` / `qwen35-9b-agent`? |

## 7. Resultados

Evaluados 11 modelos. Ganadores por prueba:

| Prueba | Ganador | Detalle |
|---|---|---|
| Shell (`dime`) | `qwen2.5-coder:1.5b` | 5/5, 986 MB (pero sin persona) |
| Commit | `qwen2.5:1.5b` / `gemma3:4b` | 2/3 (emoji incompleto) |
| Resumen ≤50w | `qwen2.5:1.5b` / `gemma3:4b` / `qwen3:1.7b` | 3/3 |
| Respuesta breve | **`gemma3:4b`** | 3/3, mejor tono |

Todoterreno: **`gemma3:4b`** (12/14). Por debajo de ~1B: roto (fidelidad, rechazos). `qwen3:4b`: inservible (filtra razonamiento con `think:false`).
Detalle completo en `docs/oneshot-models-evaluation.md`.

## 8. Decisión

**`gemma3:4b` unificado para `dime` y one-shot** (tras verificación en vivo por el gateway):

- `local-oneshot` → `gemma3:4b`.
- `local-dime` → `gemma3:4b` (con warmup); `dime.yml` apunta a `local-dime`.
- `qwen2.5:1.5b` descartado para `dime` (en vivo: contexto Windows, fences, erratas). `qwen2.5-coder:1.5b` queda como alternativa "seca".
- `dime.yml` con "sin bloques de código ni backticks".

## 9. Tareas

- [x] Descargar la shortlist.
- [x] Ejecutar 5.1 (shell) en todos los modelos.
- [x] Ejecutar 5.2 (commit) en todos los modelos.
- [x] Ejecutar 5.3 (resumen 50w) en todos los modelos.
- [x] Ejecutar 5.4 (respuesta breve) en todos los modelos.
- [x] Recoger métricas (tok/s, latencia, VRAM).
- [x] Decidir ganador y escribir §7/§8.
- [x] Crear `docs/oneshot-models-evaluation.md`.
- [x] Aplicar cambio en el gateway + `dime` (si procede).
- [x] Archivar el plan.

## 10. Artefactos

- Prompts y scripts de prueba en `/tmp/opencode/oneshot/`.
- Config afectada: `~/.config/opencode/gateway.yaml`, `~/.config/dime/dime.yml`.
- Doc final: `docs/oneshot-models-evaluation.md`.

## 11. Riesgos

- Modelos muy pequeños (≤1B) pueden fallar en español o en formato estricto; el plan los mide, no los asume.
- `num_ctx` mal fijado puede provocar evicción (lección de PLAN-004).
- No alterar el setup en producción hasta decidir; `dime` y el gateway se tocan solo al final.