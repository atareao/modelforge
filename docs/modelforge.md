# modelforge

`modelforge` es un CLI en Rust para **importar modelos GGUF desde HuggingFace**,
crearlos en Ollama, validarlos y enrutarlos en el gateway. Este documento cubre
los subcomandos `import` (incremento 2), `create` (incremento 3), `test` / `bench`
(incremento 4), `list` / `info` / `route` (incremento 5) y el import nativo de
safetensors / atajo `pull` del hub (incremento 6).

## `import` — descarga de un GGUF o import nativo de safetensors

La fuente decide el flujo:

- **Repo de HuggingFace** (`owner/model`): lista sus ficheros, selecciona el quant
  indicado y descarga el GGUF y, si se pide, el `mmproj` (proyector de visión).
  Muestra el tamaño y el progreso, y reanuda descargas interrumpidas.
- **Directorio local con `.safetensors`**: import **nativo** —genera un Modelfile
  `FROM <directorio>`, sube sus ficheros como blobs y crea el modelo en Ollama,
  sin conversión previa ni dependencias externas (ni Python ni llama.cpp).
- **Referencia del hub** (`hf.co/<repo>:<quant>`): se redirige al subcomando
  [`pull`](#pull--descarga-desde-el-hub-de-huggingface).

### Uso

```console
modelforge import <FUENTE> [--quant <QUANT>] [--name <MODELO>] [--quantize <QUANT>]
                          [--mmproj] [--dest <DIR>] [--json]
```

| Argumento / flag | Descripción |
|---|---|
| `<FUENTE>` | Repo de HuggingFace (`owner/model`), directorio local con `.safetensors`, o `hf.co/<repo>:<quant>` (obligatorio). |
| `-q, --quant <QUANT>` | Quant a descargar de un repo GGUF (p. ej. `Q6_K`, `Q4_K_M`, `q8_0`). **Obligatorio** para repos GGUF. |
| `--name <MODELO>` | Nombre del modelo a crear en Ollama. **Obligatorio** para el import nativo. |
| `--quantize <QUANT>` | Cuantización nativa de Ollama a aplicar al crear un snapshot de safetensors (p. ej. `nvfp4`). Solo en el import nativo. |
| `--mmproj` | Descarga también el `mmproj` (proyector de visión) de un repo GGUF. |
| `--dest <DIR>` | Directorio de destino de la descarga. Por defecto: `models/` en el directorio actual. |
| `--json` | Emite el resultado como JSON en `stdout`; `stderr` solo lleva los eventos de etapa NDJSON. |

### Ejemplos

Descargar un GGUF concreto:

```console
modelforge import ggml-org/tiny-llamas --quant q4_0 --dest ./models
```

Descargar un modelo de visión con su proyector:

```console
modelforge import ggml-org/SmolVLM-256M-Instruct-GGUF --quant Q8_0 --mmproj --dest ./models
```

Importar un modelo nativo desde un directorio de safetensors:

```console
modelforge import ./mi-modelo-safetensors --name mi-modelo
```

Importarlo además cuantizado a `nvfp4` en un único paso:

```console
modelforge import ./mi-modelo-safetensors --name mi-modelo --quantize nvfp4
```

Salida para agentes:

```console
modelforge import ggml-org/tiny-llamas --quant q4_0 --dest ./models --json
```

```json
{"ok":true,"data":{"repo":"ggml-org/tiny-llamas","quant":"q4_0","dest":"./models","files":[{"path":"stories15M-q4_0.gguf","size":19077344,"cached":false}]}}
```

### Import nativo (safetensors)

Ollama importa snapshots nativos de HuggingFace (`config.json`, tokenizer y
ficheros `.safetensors`) directamente a partir de un Modelfile `FROM <directorio>`:

1. Se valida que el directorio contiene al menos un `.safetensors` (si no, error
   claro y código de salida distinto de 0).
2. Se genera el Modelfile `FROM <directorio>`.
3. Se sube cada fichero del directorio como blob (`POST /api/blobs/:digest`) y se
   referencian en el mapa `files` de `POST /api/create`.
4. Se crea el modelo (`--name`) y se verifica con `/api/show`.

La **copia de pesos la gestiona Ollama**: `modelforge` no convierte ni copia. Si
la arquitectura del modelo no la soporta el conversor interno de Ollama, el error
de Ollama se propaga tal cual.

> **Limitación conocida — Ollama 0.40.8 en Linux:** la creación desde safetensors
> requiere el *runtime* **MLX**, que **no está disponible en Linux**. Ollama
> responde `503` con `MLX runtime is not available` y `modelforge` propaga ese
> error tal cual (código de salida ≠ 0). El import nativo (y `--quantize`) solo
> funcionan en plataformas soportadas por el conversor interno de Ollama (p. ej.
> **macOS con MLX**). En Linux, usa el flujo **GGUF** (`import --quant` o
> `pull`), que sí está soportado. Esta limitación está registrada como *Non-Goal*
> en `openspec/changes/native-model-import/design.md` y su cobertura real está
> diferida a los tests `#[ignore]` de `tests/native_import_real_ollama.rs`.

```console
$ modelforge import ./mi-modelo-safetensors --name mi-modelo
modelo `mi-modelo` creado en Ollama
vision: no
num_ctx: 262144
Modelfile:
FROM /ruta/absoluta/mi-modelo-safetensors
...
```

### Cuantización nativa (`--quantize`)

`--quantize <QUANT>` traduce al campo `quantize` de `POST /api/create` y hace que
Ollama **cuantice durante la creación** (sin `llama-quantize` ni Python). Solo vale
para un **directorio de safetensors**: Ollama rechaza cuantizar un GGUF ya
convertido (`create-time quantization is only supported for safetensors imports`),
así que con `create --from <gguf>` el CLI falla antes de llamar a la API.

El valor se **valida antes** de llamar a la API contra la lista soportada; un valor
inválido falla listando los válidos:

```console
$ modelforge import ./mi-modelo-safetensors --name mi-modelo --quantize NO_EXISTE
error: cuantización `NO_EXISTE` no soportada; valores válidos: int4, int8, nvfp4, mxfp4, mxfp8
$ echo $?
1
```

Valores soportados: `int4`, `int8`, `nvfp4`, `mxfp4`, `mxfp8` (sin distinguir
mayúsculas/minúsculas; el valor se envía en minúsculas).

> **Nota:** esta lista es la que acepta **la versión de Ollama instalada**. Los
> valores se verificaron contra **Ollama 0.40.8**, cuyo mensaje de la API es
> `unsupported quantize type "X": supported types are int4, int8, nvfp4, mxfp4,
> mxfp8`. Los quants de llama.cpp (`Q4_K_M`, `Q6_K`, `Q8_0`…) **no** son válidos
> aquí: esos son los que se eligen con `--quant` al descargar un GGUF del Hub, no
> los que Ollama aplica al cuantizar. Si se actualiza el servidor, revisa la lista
> con `ollama create --help`.

### Progreso por etapas

Las operaciones que pueden durar minutos indican la etapa en curso. El progreso va
**siempre a `stderr`**: en modo texto como mensajes de `tracing` y, con `--json`,
como líneas NDJSON (`{"stage":"..."}`). Con `--json`, `stderr` contiene **solo**
esas líneas (el logging humano se silencia) y **`stdout` conserva el contrato de un
único objeto JSON** (el sobre final `{"ok":true,...}`) en todos los subcomandos.

Las etapas se anuncian **cuando ocurren** y llevan el nombre de la spec: `descarga`
(import de un repo GGUF), `conversión interna` (preparación del snapshot y subida de
blobs), `cuantización` (con el valor) y `creación`. La etapa `conversión interna` se
anuncia **tras** confirmar que Ollama responde (`/api/version`), de modo que no se
emite ninguna etapa si el servidor está caído:

```console
$ modelforge import ./mi-modelo-safetensors --name mi-modelo --quantize nvfp4 --json
{"stage":"conversión interna"}                       # → stderr
{"stage":"cuantización","value":"nvfp4"}             # → stderr
{"stage":"creación"}                                 # → stderr
{"ok":true,"data":{...}}                             # → stdout (única línea)
```

### Selección del quant

El filtro compara **tokens** del nombre del fichero (separados por `-` y `.`)
con el quant pedido, **sin distinguir mayúsculas/minúsculas**. Así funcionan a la
vez `model-Q6_K.gguf`, `model.Q6_K.gguf` y `stories15M-q4_0.gguf`. Los ficheros de
proyector (`mmproj`, comparados sin distinguir mayúsculas) se excluyen de la
selección del GGUF. Si se pide `--mmproj` y el repo tiene varios, se prefiere el
que lleva el **mismo quant** que el GGUF; si ninguno coincide, se toma el primero
por ruta.

Si el quant no existe, el error indica los quants disponibles:

```console
$ modelforge import ggml-org/tiny-llamas --quant Q9_Z
error: el repo `ggml-org/tiny-llamas` no contiene el quant `Q9_Z`; disponibles: q4_0, q8_0
$ echo $?
1
```

### Descarga reanudable

La descarga se hace **fichero a fichero** y es reanudable mediante un fichero
temporal `<dest>/<ruta>.part`:

1. Si el fichero final ya existe con el tamaño remoto, se omite (no se vuelve a
   bajar).
2. Si existe un `.part`, se pide al Hub **solo el rango restante**
   (`Range: bytes=<descargado>-`) y se **anexa** al `.part`.
3. Al terminar, el `.part` se renombra al nombre definitivo (operación atómica en
   el mismo directorio).

La decisión de reanudación (`resume_plan`) es lógica pura y tiene tests unitarios;
el acceso a la red vive detrás del trait `hf::HfClient`, lo que permite probar la
orquestación con un cliente falso **sin red**.

**Decisión (ver `design.md`):** se usa **`hf-hub`** para resolver, listar y
descargar (xet, cache y reintentos los gestiona la librería; el rango de
reanudación se monta sobre `hf-hub`). Alternativa descartada: implementar el
protocolo a mano con `reqwest`.

### Salida y códigos de salida

- Texto (por defecto): una línea por fichero con ruta, tamaño y estado.
- `--json`: `{"ok":true,"data":{...}}` en `stdout` (un único objeto); en `stderr`
  van **solo** las líneas NDJSON de etapa (`{"stage":...}`), sin logging humano.
- Errores: mensaje claro en `stderr` (o `{"ok":false,"error":"..."}` con `--json`)
  y **código de salida distinto de 0**.

### Errores habituales

| Mensaje | Causa |
|---|---|
| `el repo `X` no contiene el quant `Y`; disponibles: ...` | El quant pedido no está en el repo. |
| `el repo `X` no contiene ningún fichero GGUF` | El repo no tiene GGUF. |
| `el repo `X` no contiene ningún mmproj ...` | Se pidió `--mmproj` y no hay proyector. |
| `falta `--quant <QUANT>` ...` | No se indicó el quant de un repo GGUF. |
| `falta `--name <MODELO>` ...` | No se indicó el nombre del modelo en el import nativo. |
| `el directorio `X` no contiene ningún fichero `.safetensors`` | El directorio indicado no es un snapshot nativo. |
| `cuantización `X` no soportada; valores válidos: int4, int8, nvfp4, mxfp4, mxfp8` | `--quantize` no es un valor soportado por Ollama. |
| ``--quantize` solo es válido para un directorio de safetensors ...` | Se pidió `--quantize` con un GGUF ya convertido (Ollama lo rechaza). |
| `el import nativo de safetensors no acepta `--quant, ...`` | Se pasaron flags de repo GGUF (`--quant`/`--mmproj`/`--dest`) al import nativo. |
| `el import de un repo GGUF no acepta `--name, --quantize`` | Se pasaron flags del import nativo (`--name`/`--quantize`) a un repo GGUF. |
| ``X` es una referencia del hub; usa `modelforge pull X`` | La fuente es `hf.co/...`; usa el subcomando `pull`. |
| `repo inválido `X`: se espera `owner/model`` | Formato de repo incorrecto. |
| `HuggingFace: ...` | Error del Hub (red, autenticación, repo inexistente). |

## `create` — creación del modelo en Ollama

Genera un **Modelfile reproducible** a partir de un GGUF local (y, opcionalmente,
su `mmproj`), materializa el modelo en Ollama y **verifica** el resultado con
`/api/show`.

### Uso

```console
modelforge create <NAME> --from <PATH> [--mmproj <MMPROJ>] [--ctx <N>] [--renderer <NAME>] [--parser <NAME>] [--quantize <QUANT>] [--json]
```

| Argumento / flag | Descripción |
|---|---|
| `<NAME>` | Nombre del modelo en Ollama, formato `model[:tag]` (obligatorio). |
| `--from <PATH>` | Fichero GGUF **o directorio de safetensors** de origen. **Obligatorio**. Se absolutiza antes de usarlo. |
| `--mmproj <MMPROJ>` | Proyector de visión. Añade una segunda línea `FROM` y activa `vision`. |
| `--ctx <N>` | Tamaño de contexto (`num_ctx`). Por defecto: `262144`. |
| `--renderer <NAME>` | `RENDERER` del Modelfile. Por defecto: `qwen3.5`. |
| `--parser <NAME>` | `PARSER` del Modelfile. Por defecto: `qwen3.5`. |
| `--quantize <QUANT>` | Cuantización nativa de Ollama a aplicar al crear un directorio de safetensors (p. ej. `nvfp4`). Se valida antes de llamar a la API y **no** vale con un GGUF. |
| `--json` | Emite el resultado como JSON en `stdout`; `stderr` solo lleva los eventos de etapa NDJSON. |

Si `--from` es un **directorio**, se comporta como el [import
nativo](#import-nativo-safetensors): exige al menos un `.safetensors`, sube como
blobs los ficheros **relevantes** del directorio (pesos `.safetensors`, `.json`,
tokenizers binarios y solo los `.txt` de tokenizer `vocab.txt`/`merges.txt`; se
ignoran `README.md`, `.gitattributes`, `README.txt`, `requirements.txt`…) y usa
`FROM <directorio>` en el Modelfile. `--quantize` **solo** se admite en este caso.

### Modelfile generado

El Modelfile se construye **de forma determinista**. Sin `--mmproj`:

```text
FROM /ruta/absoluta/model-Q6_K.gguf
RENDERER qwen3.5
PARSER qwen3.5
PARAMETER temperature 1
PARAMETER top_k 20
PARAMETER top_p 0.95
PARAMETER repeat_penalty 1
PARAMETER presence_penalty 1.5
PARAMETER num_ctx 262144
```

Con `--mmproj` se inserta una segunda línea `FROM` con el proyector, justo después
del GGUF:

```text
FROM /ruta/absoluta/model-Q6_K.gguf
FROM /ruta/absoluta/mmproj-f16.gguf
RENDERER qwen3.5
...
```

Los valores por defecto (`num_ctx 262144`, `renderer`/`parser qwen3.5` y el juego de
parámetros de muestreo) están alineados con el modelo de referencia del proyecto.
Se sobreescriben con `--ctx`, `--renderer` y `--parser`.

### Cómo se crea el modelo

El Modelfile se usa para el reporte y la reproducibilidad, pero la creación sigue
el flujo de la **API moderna de Ollama** para modelos GGUF locales:

1. Se calcula el **SHA-256** de cada fichero de origen.
2. Si el blob no existe en el servidor (`HEAD /api/blobs/:digest`), se sube
   (`POST /api/blobs/:digest`, por bloques y sin cargarlo entero en memoria).
3. Se crea el modelo con `POST /api/create` enviando el mapa `files`
   (nombre → digest), `renderer`, `parser` y `parameters` (valores tipados).
4. Se verifica con `POST /api/show` (capacidades y `num_ctx`).

> Nota: calcular el SHA-256 exige leer el GGUF completo, así que en modelos grandes
> la primera ejecución puede tardar unos minutos. Si el blob ya existe (mismo
> fichero subido antes), no se vuelve a subir.

### Ejemplos

Creación con visión y contexto de 256k:

```console
modelforge create mi-modelo:q6_K-256k \
  --from /data/models/model-Q6_K.gguf \
  --mmproj /data/models/mmproj-f16.gguf \
  --ctx 262144
```

Salida:

```text
modelo `mi-modelo:q6_K-256k` creado en Ollama
vision: sí
num_ctx: 262144
Modelfile:
FROM /data/models/model-Q6_K.gguf
FROM /data/models/mmproj-f16.gguf
RENDERER qwen3.5
PARSER qwen3.5
PARAMETER temperature 1
PARAMETER top_k 20
PARAMETER top_p 0.95
PARAMETER repeat_penalty 1
PARAMETER presence_penalty 1.5
PARAMETER num_ctx 262144
```

Salida para agentes:

```console
modelforge --json create mi-modelo --from /data/models/model-Q6_K.gguf --ctx 8192
```

```json
{"ok":true,"data":{"model":"mi-modelo","blobs":{"model-Q6_K.gguf":"sha256:f7af…"},"info":{"capabilities":["completion","tools","thinking"],"parameters":"num_ctx                        8192\n…"},"modelfile":"FROM /data/models/model-Q6_K.gguf\nRENDERER qwen3.5\n…"}}
```

### Errores habituales

| Mensaje | Causa |
|---|---|
| `falta `--from <GGUF>` ...` | No se indicó el GGUF de origen. |
| `no existe el fichero `X` ...` | La ruta de `--from` o `--mmproj` no existe. |
| `Ollama no responde en `X`: ...` | Ollama no está levantado en `--ollama`; código de salida ≠ 0. |
| `Ollama: ...` | La API de Ollama rechazó la operación (p. ej. GGUF inválido). |

## `test` — pruebas de humo del modelo

Ejecuta una batería de pruebas contra un modelo ya creado en Ollama y reporta el
resultado. Siempre se ejecuta **texto**; la prueba de **thinking** se añade
automáticamente si el modelo declara esa capacidad (pedir razonamiento a un modelo
que no lo soporta devuelve un 400 de Ollama, así que se omite). Con `--tools` y
`--vision` se añaden el *tool-calling* y la visión.

### Uso

```console
modelforge test <NAME> [--tools] [--vision] [--image <PATH>] [--num-predict <N>] [--json]
```

| Argumento / flag | Descripción |
|---|---|
| `<NAME>` | Modelo de Ollama a probar (obligatorio). |
| `--tools` | Añade la prueba de *tool-calling*: se envía una herramienta y se exige que el modelo emita `tool_calls`. |
| `--vision` | Añade la prueba de visión: se envía una imagen y se espera respuesta. |
| `--image <PATH>` | Imagen para la prueba de visión. Por defecto se usa una imagen PNG 8×8 embebida. |
| `--num-predict <N>` | Límite de tokens generados por prueba. |
| `--json` | Emite el resultado como JSON en `stdout`. |

### Qué comprueba cada modo

| Modo | Envío | Criterio de éxito |
|---|---|---|
| `text` | Prompt de texto, `think: false`. | La respuesta tiene contenido no vacío. |
| `thinking` | Prompt aritmético, `think: true`. Solo si el modelo declara `thinking`. | Hay bloque de razonamiento **o**, al menos, respuesta. |
| `tools` | Prompt + herramienta `get_weather`. | El modelo emite `tool_calls`. |
| `vision` | Prompt + imagen en base64. | El modelo responde a la imagen. |

### Ejemplos

```console
modelforge test mi-modelo:q6_K-256k --tools
```

```text
[OK   ] text: 177 caracteres de respuesta
[OK   ] thinking: el modelo emitió razonamiento
[OK   ] tools: tool_calls: get_weather
modelo `mi-modelo:q6_K-256k`: todas las pruebas pasaron
```

Salida para agentes:

```console
modelforge --json test mi-modelo:q6_K-256k --vision
```

```json
{"ok":true,"data":{"model":"mi-modelo:q6_K-256k","passed":true,"checks":[{"mode":"text","ok":true,"detail":"137 caracteres de respuesta"},{"mode":"thinking","ok":true,"detail":"el modelo emitió razonamiento"},{"mode":"vision","ok":true,"detail":"64 caracteres de respuesta a la imagen"}]}}
```

El sobre `ok` indica que el comando pudo ejecutarse; el campo `data.passed` es el
veredicto de las pruebas. Si **alguna** prueba falla, `passed` es `false` y el
**código de salida es distinto de 0** (los detalles siguen en `data.checks`).

> **Modelos pequeños:** un modelo de pocos parámetros puede ser incapaz de emitir
> `tool_calls` de forma fiable. En ese caso la prueba `tools` se marca `FALLA`
> (`el modelo no emitió \`tool_calls\``) y el código de salida es ≠0, aunque
> `text`, `thinking` y `vision` pasen: es una limitación del **modelo**, no del CLI.
>
> Si Ollama devuelve un **error** durante una prueba (p. ej. `prediction aborted`),
> el comando no reporta un `check` fallido sino que **aborta** con
> `{"ok":false,"error":"…"}` y código de salida ≠0.

## `bench` — benchmark del modelo

Mide el rendimiento de generación y reporta `eval_count`, `eval_duration`,
`load_duration`, `size_vram`, `context` y `processor`, además de los **tokens/s**.
Con `--compare <MODEL>` ejecuta el **mismo prompt** en ambos modelos y muestra
una tabla comparativa.

### Uso

```console
modelforge bench <NAME> [--compare <MODEL>] [--prompt <TEXT>] [--num-predict <N>] [--json]
```

| Argumento / flag | Descripción |
|---|---|
| `<NAME>` | Modelo a medir (obligatorio). |
| `--compare <MODEL>` | Segundo modelo; se mide con el mismo prompt y se compara. |
| `--prompt <TEXT>` | Prompt a ejecutar. Por defecto, un párrafo de ~50 palabras sobre Rust. |
| `--num-predict <N>` | Límite de tokens generados por ejecución. |
| `--json` | Emite el resultado como JSON en `stdout`. |

### Campos medidos

| Campo | Origen |
|---|---|
| `eval_count`, `eval_duration_ns`, `load_duration_ns` | Respuesta de `/api/generate`. |
| `size_vram`, `context` (`context_length`) | `/api/ps`, tras la generación. |
| `processor` | Derivado de `size` y `size_vram`: `100% GPU`, `100% CPU` o `NN%/MM% CPU/GPU`. |
| `tokens_per_second` | `eval_count / (eval_duration_ns / 1e9)`; `0` si la duración es 0. |

La tabla comparativa es una **función pura** (`bench::comparison_table`) sobre las
mediciones ya obtenidas, de modo que su formato es determinista y testeable.

### Ejemplos

```console
modelforge bench qwen3:8b --compare llama3.2:3b --num-predict 48
```

```text
prompt: Escribe un párrafo de unas 50 palabras explicando qué es Rust y para qué sirve.
model        tok/s   eval_count  eval_duration  load_duration  size_vram  context  processor
-----------  ------  ----------  -------------  -------------  ---------  -------  ---------
qwen3:8b     52.26   48          918.50ms       2.94s          9.6 GiB    32768    100% GPU
llama3.2:3b  119.05  48          403.20ms       6.47s          6.1 GiB    32768    100% GPU
```

Salida para agentes:

```console
modelforge --json bench llama3.2:3b --num-predict 16
```

```json
{"ok":true,"data":{"prompt":"Escribe un párrafo de unas 50 palabras explicando qué es Rust y para qué sirve.","results":[{"model":"llama3.2:3b","eval_count":16,"eval_duration_ns":128577000,"load_duration_ns":1223775,"size_vram":6533131795,"context":32768,"processor":"100% GPU","tokens_per_second":124.44}]}}
```

## `list` — modelos instalados

Lista los modelos de Ollama cruzando `/api/tags` (instalados) con `/api/ps`
(cargados en memoria), y muestra para cada uno su **contexto** y su **VRAM**.

### Uso

```console
modelforge list [--json]
```

Solo tiene flags globales (`--ollama`, `--config`, `--json`, `--verbose`).

### Campos mostrados

| Campo | Origen |
|---|---|
| `model` | `name`/`model` de `/api/tags`. |
| `context` | `context_length` de `/api/ps`; `0` si el modelo no está cargado. |
| `size_vram` | `size_vram` de `/api/ps`, formateado en unidades binarias. |
| `processor` | Derivado de `size` y `size_vram`: `100% GPU`, `100% CPU` o `NN%/MM% CPU/GPU`; `desconocido` si no está cargado. |

### Ejemplo

```text
model                                context  size_vram  processor
-----------------------------------  -------  ---------  -----------
deepseek-v4-pro-9b-heretic:q6_K-256k 262144   11.9 GiB   100% GPU
qwen3:8b                             0        0 B        desconocido
```

```json
{"ok":true,"data":{"models":[{"context":262144,"name":"deepseek-v4-pro-9b-heretic:q6_K-256k","processor":"100% GPU","size_vram":12799768001}]}}
```

> El **contexto** y la **VRAM** solo se conocen para modelos cargados (`/api/ps`).
> Un modelo instalado pero no cargado aparece con `0` / `desconocido`.

## `info` — detalles de un modelo

Muestra el **contexto**, la **VRAM** y el **processor** de un modelo, más sus
capacidades. Combina `/api/show` (contexto configurado y capacidades) con
`/api/ps` (VRAM y processor si está cargado).

### Uso

```console
modelforge info <NAME> [--json]
```

| Argumento | Descripción |
|---|---|
| `<NAME>` | Nombre del modelo a inspeccionar (obligatorio). |

### Comportamiento

- **Contexto:** `num_ctx` de `/api/show`; si no se reporta, el `context_length`
  cargado en `/api/ps`.
- **VRAM / processor:** de `/api/ps` si el modelo está cargado; si no, `0 B` y
  `desconocido`.

### Ejemplo

```console
modelforge info deepseek-v4-pro-9b-heretic:q6_K-256k
```

```text
model: deepseek-v4-pro-9b-heretic:q6_K-256k
context: 262144
size_vram: 11.9 GiB
processor: 100% GPU
capabilities: completion, vision, tools, thinking
```

```json
{"ok":true,"data":{"capabilities":["completion","vision","tools","thinking"],"context":262144,"name":"deepseek-v4-pro-9b-heretic:q6_K-256k","processor":"100% GPU","size_vram":12799768001}}
```

## `route` — añadir una ruta al gateway

Añade una ruta al `gateway.yaml` del gateway de OpenCode. La operación es
**idempotente** (no duplica), hace **backup** del fichero previo y **valida** el
YAML antes de escribir.

### Uso

```console
modelforge route <NAME> --model <MODEL> [--dry-run] [--config <PATH>] [--json]
```

| Argumento / flag | Descripción |
|---|---|
| `<NAME>` | Nombre lógico de la ruta a añadir (obligatorio). |
| `--model <MODEL>` | Modelo local (Ollama) asociado a la ruta. **Obligatorio**. |
| `--dry-run` | Muestra el YAML resultante sin escribir el fichero. |
| `--config <PATH>` | Ruta al `gateway.yaml`. Por defecto: `$HOME/.config/opencode/gateway.yaml`. |

### Esquema aplicado

El `gateway.yaml` es un documento con una lista `route` bajo la clave `route`.
Cada ruta tiene un `name` y una lista `backend`. `modelforge route` añade una
ruta con **dos backends**: el **local** de Ollama (el modelo indicado) y el de
**respaldo** (`BACKUP`) remoto:

```yaml
route:
  - name: <NAME>
    backend:
      - base_url: "http://localhost:11434/v1"        # backend local
        model: "<MODEL>"
        timeout_ms: 120000
      - base_url: "https://openrouter.ai/api/v1"     # BACKUP
        model: "deepseek/deepseek-v4.1-flash"
        api_key_env: "OPENROUTER_API_KEY"
        timeout_ms: 120000
```

Este esquema reproduce la estructura real de `~/.config/opencode/gateway.yaml`
(backend local + respaldo OpenRouter). Las rutas existentes se conservan.

### Comportamiento

- **Ruta nueva:** se añade y, si el fichero ya existía, se guarda una copia en
  `<fichero>.bak` antes de reescribirlo.
- **Ruta existente** (mismo `name`): no se duplica ni se modifica el fichero; se
  avisa al usuario y **no** se crea backup.
- **YAML inválido:** se aborta con un error claro y código de salida distinto de
  `0`, sin escribir ni crear backup.
- **`--dry-run`:** no escribe el fichero ni crea backup; imprime el YAML resultante.

### Backup

Antes de reescribir un `gateway.yaml` **existente** se copia a `gateway.yaml.bak`
(en el mismo directorio). El backup siempre refleja el contenido **inmediatamente
anterior** a la escritura.

```console
$ modelforge route local-heretic --model deepseek-v4-pro-9b-heretic:q6_K-256k
ruta `local-heretic` añadida a `/home/u/.config/opencode/gateway.yaml` (modelo `deepseek-v4-pro-9b-heretic:q6_K-256k`)
backup: /home/u/.config/opencode/gateway.yaml.bak

$ modelforge route local-heretic --model otro:modelo
la ruta `local-heretic` ya existe en `/home/u/.config/opencode/gateway.yaml`; no se ha duplicado
```

```json
{"ok":true,"data":{"backup":"/home/u/.config/opencode/gateway.yaml.bak","model":"deepseek-v4-pro-9b-heretic:q6_K-256k","name":"local-heretic","path":"/home/u/.config/opencode/gateway.yaml","status":"added"}}
```

### Errores habituales

| Situación | Mensaje |
|---|---|
| Sin `--model` | ``falta `--model <MODEL>`: indica el modelo de la ruta `<name>` del gateway`` |
| YAML inválido | ``el `gateway.yaml` en `<path>` no es válido: <detalle>`` |
| Sin `$HOME` ni `--config` | ``no se pudo determinar el directorio `$HOME`; usa `--config <PATH>` `` |

> Al añadir una ruta sobre un fichero **nuevo** no hay backup y el sobre devuelve
> `"backup":null`. Cuando el `gateway.yaml` ya existía, se guarda `<fichero>.bak` y
> el campo `backup` lleva su ruta.

## `pull` — descarga desde el hub de HuggingFace

Atajo remoto para modelos que **ya publican GGUF** en el hub: delega por completo
en Ollama (`POST /api/pull`), que resuelve y registra el modelo a partir de una
referencia `hf.co/<repo>:<quant>`. No descarga ni convierte nada `modelforge`.

### Uso

```console
modelforge pull <hf.co/<repo>:<quant>> [--json]
```

| Argumento / flag | Descripción |
|---|---|
| `<hf.co/<repo>:<quant>>` | Referencia del hub (obligatoria). Formato estricto: `hf.co/owner/modelo:QUANT`. |
| `--json` | Emite el resultado como JSON en `stdout`; `stderr` solo lleva los eventos de etapa NDJSON. |

### Ejemplos

```console
modelforge pull hf.co/usuario/modelo:Q4_K_M
modelo `hf.co/usuario/modelo:Q4_K_M` descargado en Ollama
```

Salida para agentes:

```json
{"ok":true,"data":{"reference":"hf.co/usuario/modelo:Q4_K_M","status":"success"}}
```

### Validación de la referencia

La referencia debe empezar por `hf.co/`, tener un repo `owner/modelo` y un quant no
vacío. Cualquier otro formato falla **antes** de llamar a Ollama, con código de
salida distinto de 0:

```console
$ modelforge pull usuario/modelo:Q4_K_M
error: referencia inválida `usuario/modelo:Q4_K_M`: se espera `hf.co/<repo>:<quant>` (p. ej. `hf.co/usuario/modelo:Q4_K_M`)
$ echo $?
1
```

### Errores habituales

| Mensaje | Causa |
|---|---|
| `referencia inválida `X`: se espera `hf.co/<repo>:<quant>`` | La referencia no tiene el formato esperado. |
| `Ollama no responde en `X`: ...` | Ollama no está accesible. |
| `Ollama: ...` | Ollama rechazó la descarga (repo/quant inexistente, etc.). |

## Flujo end-to-end (verificado)

Pipeline mínimo para llevar un GGUF desde HuggingFace hasta el gateway:
`import` → `create` → `test` → `bench` → `route` → limpieza. Ejemplo real con un
modelo pequeño (`ggml-org/SmolVLM-256M-Instruct-GGUF`, ~270 MB con proyector) y un
directorio temporal, verificado contra Ollama local:

```console
DEST=$(mktemp -d)

# 1) Import (GGUF + mmproj de visión) desde HuggingFace
modelforge --json import ggml-org/SmolVLM-256M-Instruct-GGUF \
  --quant Q8_0 --mmproj --dest "$DEST"

# 2) Crear el modelo en Ollama (verifica con /api/show)
modelforge --json create mfx-e2e:test \
  --from "$DEST/SmolVLM-256M-Instruct-Q8_0.gguf" \
  --mmproj "$DEST/mmproj-SmolVLM-256M-Instruct-Q8_0.gguf" \
  --ctx 4096

# 3) Pruebas de humo (texto + thinking; tools/vision bajo demanda)
modelforge --json test mfx-e2e:test --vision

# 4) Benchmark con comparación opcional
modelforge --json bench mfx-e2e:test --compare llama3.2:3b --num-predict 32

# 5) Enrutar en un gateway.yaml TEMPORAL (nunca el real del usuario)
modelforge --config "$DEST/gateway.yaml" --json route mfx-e2e --model mfx-e2e:test

# 6) Limpieza
ollama rm mfx-e2e:test
rm -rf "$DEST"
```

Salidas reales acotadas:

```json
// 1) import  → tamaño de GGUF y mmproj en bytes; `cached` indica si ya existía
{"data":{"dest":"/tmp/…","files":[{"cached":false,"path":"SmolVLM-256M-Instruct-Q8_0.gguf","size":175054528},{"cached":false,"path":"mmproj-SmolVLM-256M-Instruct-Q8_0.gguf","size":103769856}],"quant":"Q8_0","repo":"ggml-org/SmolVLM-256M-Instruct-GGUF"},"ok":true}
```

```json
// 3) test --vision  → sobre ok:true con el veredicto en data.passed
{"data":{"checks":[{"detail":"… caracteres de respuesta","mode":"text","ok":true},{"detail":"el modelo emitió razonamiento","mode":"thinking","ok":true},{"detail":"… caracteres de respuesta a la imagen","mode":"vision","ok":true}],"model":"mfx-e2e:test","passed":true},"ok":true}
```

```json
// 5) route  → status "added"; "backup" es null porque el fichero era nuevo
{"data":{"backup":null,"model":"mfx-e2e:test","name":"mfx-e2e","path":"/tmp/…/gateway.yaml","status":"added"},"ok":true}
```

Comportamiento confirmado en la ejecución:

- El sobre JSON es siempre `{"ok":true,"data":…}` o `{"ok":false,"error":"…"}`.
  `serde_json` ordena las claves alfabéticamente (`data` antes de `ok`); el orden
  **no es significativo**.
- `create` sube los blobs ausentes y verifica el resultado con `/api/show`.
- `test` devuelve código de salida `0` si todas las pruebas pasan y ≠0 si alguna
  falla, **aunque** el sobre siga siendo `ok:true` (`data.passed=false`).
- `route` sobre un fichero nuevo devuelve `"backup":null`; sobre uno existente
  guarda `<fichero>.bak` antes de reescribir. Con `--config` apunta siempre a un
  `gateway.yaml` concreto y **nunca** toca el `~/.config/opencode/gateway.yaml` real.
- `route` es **idempotente**: reejecutarlo avisa (`ya existe … no se ha duplicado`)
  sin modificar el fichero ni crear backup.

## Variables de entorno

| Variable | Uso |
|---|---|
| `HF_TOKEN` | Token de HuggingFace para repos privados o con *rate limit*. |
| `HF_ENDPOINT` | Endpoint alternativo del Hub. |
| `HF_HOME` / `HF_HUB_CACHE` | Ubicación del estado/cache de HuggingFace. |
| `RUST_LOG` | Nivel de logs (por defecto `info`; `-v` pone `debug`). |

## Verificación

```console
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
cargo run -- import --help
cargo run -- create --help
cargo run -- test --help
cargo run -- bench --help
cargo run -- list --help
cargo run -- info --help
cargo run -- route --help
cargo run -- pull --help
```

Pruebas de integración **ignoradas** por defecto (requieren red / Ollama real):

```console
# descarga real desde HuggingFace
cargo test -- --ignored downloads_real_gguf_from_hub

# creación real en Ollama (necesita un GGUF válido en disco)
MODELFORGE_TEST_GGUF=/ruta/al/model.gguf cargo test -- --ignored creates_model_with_real_ollama

# import nativo real (solo en un host soportado por MLX; en Linux Ollama 0.40.8 falla con 503)
MODELFORGE_TEST_SAFETENSORS=/ruta/al/snapshot cargo test -- --ignored imports_safetensors_with_real_ollama

# pruebas de humo y benchmark reales contra Ollama
cargo test -- --ignored --test test_bench_real_ollama
```

El test `test_bench_cli` comprueba, sin necesitar Ollama, que `test` y `bench`
fallan con un mensaje claro y código de salida distinto de 0 cuando el servidor no
responde, y que `test --vision` valida la imagen antes de llamar a la API.

El test `list_route_cli` comprueba, sin necesitar Ollama, que `list`/`info` fallan
con un mensaje claro cuando el servidor no responde, y que `route` sobre un
`gateway.yaml` temporal (vía `--config`) añade la ruta, no la duplica, crea el
backup `.bak` antes de reescribir y aborta ante un YAML inválido. **Nunca** toca el
`gateway.yaml` real del usuario.

El test `native_import_cli` comprueba, sin red ni Ollama, el import nativo de
safetensors: falla pidiendo `--name`, falla si el directorio no tiene
`.safetensors`, valida `--quantize` **antes** de llamar a Ollama y falla con error
claro si Ollama no responde. El test `pull_cli` comprueba lo propio para `pull`:
referencia válida + Ollama caído, y referencias inválidas con mensaje de formato.

Variables opcionales de los tests reales de `create`: `MODELFORGE_TEST_OLLAMA` (URL
base), `MODELFORGE_TEST_NAME` (nombre del modelo), `MODELFORGE_TEST_CTX` (`num_ctx`)
y `MODELFORGE_TEST_MMPROJ` (proyector de visión). En los de `test`/`bench`:
`MODELFORGE_TEST_MODEL` (modelo a probar) y `MODELFORGE_TEST_MODEL2` (segundo modelo
para la comparación).
