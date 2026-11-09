# native-model-import

Documenta la feature **native-model-import** (issue #8): importar modelos
publicados en formato nativo (`.safetensors`) y GGUF del hub usando **solo
capacidades nativas de Ollama**, sin Python ni `llama.cpp`. Cubre el **import
nativo** de un directorio de safetensors, la **cuantización en el propio import**
y el **atajo remoto** del hub (`pull`).

## Qué es

Un directorio que sigue la convención de HuggingFace (`config.json`, tokenizer y
ficheros `.safetensors`) es un **snapshot nativo**. Ollama lo acepta directamente
a partir de un Modelfile `FROM <directorio>`: `modelforge` genera ese Modelfile,
sube los ficheros como blobs y crea el modelo.

La **conversión y la copia de pesos las gestiona Ollama**; `modelforge` no
convierte ni reimplementa nada. No se invoca Python (`transformers`,
`safetensors`) ni `llama.cpp` (`llama-quantize`). Si la arquitectura del modelo no
la soporta el conversor interno de Ollama, el error de Ollama se propaga tal cual.

## Los tres flujos

| Flujo | Comando | Qué hace |
|---|---|---|
| Import nativo | `modelforge import <dir> --name <modelo>` | Importa un directorio de `.safetensors` + metadatos. |
| Import + cuantización | `modelforge import <dir> --quantize <QUANT> --name <modelo>` | Cuantiza **durante** la creación, en un único paso. |
| Atajo remoto | `modelforge pull hf.co/<repo>:<quant>` | Delega en Ollama (`/api/pull`) para un GGUF ya publicado en el hub. |

### Import nativo de un directorio

```console
modelforge import ./mi-modelo-safetensors --name mi-modelo
```

```text
modelo `mi-modelo` creado en Ollama
vision: no
num_ctx: 262144
Modelfile:
FROM /ruta/absoluta/mi-modelo-safetensors
...
```

El pipeline es:

1. Se valida que el directorio contiene al menos un `.safetensors` (si no, error
   claro y código de salida distinto de 0).
2. Se genera el Modelfile `FROM <directorio>`.
3. Se sube cada fichero relevante como blob (`POST /api/blobs/:digest`) y se
   referencian en el mapa `files` de `POST /api/create`.
4. Se crea el modelo (`--name`) y se verifica con `/api/show`.

### Import con cuantización

Añadir `--quantize <QUANT>` hace que Ollama **cuantice al crear**, sin
`llama-quantize` ni Python:

```console
modelforge import ./mi-modelo-safetensors --quantize nvfp4 --name mi-modelo
```

### Atajo remoto del hub (`pull`)

Atajo para modelos que **ya publican GGUF** en el hub: delega por completo en
Ollama, que resuelve y registra el modelo a partir de la referencia. `modelforge`
no descarga ni convierte nada.

```console
modelforge pull hf.co/usuario/modelo:Q4_K_M
```

```text
modelo `hf.co/usuario/modelo:Q4_K_M` descargado en Ollama
```

## Cuantización

`--quantize <QUANT>` traduce al campo `quantize` de `POST /api/create`. El valor se
**valida antes** de llamar a la API contra la lista soportada; un valor inválido
falla listando los válidos:

```console
$ modelforge import ./mi-modelo-safetensors --quantize NO_EXISTE --name mi-modelo
error: cuantización `NO_EXISTE` no soportada; valores válidos: int4, int8, nvfp4, mxfp4, mxfp8
$ echo $?
1
```

Valores válidos reales de Ollama: `int4`, `int8`, `nvfp4`, `mxfp4`, `mxfp8` (sin
distinguir mayúsculas/minúsculas; el valor se envía en **minúsculas**).

> **Nota:** esta lista es la que acepta **la versión de Ollama instalada**. Los
> valores se verificaron contra **Ollama 0.40.8**, cuyo mensaje de la API es
> `unsupported quantize type "X": supported types are int4, int8, nvfp4, mxfp4,
> mxfp8`. Los quants de **llama.cpp** (`Q4_K_M`, `Q6_K`, `Q8_0`…) **no** son
> válidos aquí: esos son los que se eligen con `--quant` al descargar un **GGUF**
> del hub (`import --quant` o `pull`), **no** los que Ollama aplica al cuantizar un
> snapshot de safetensors. Si se actualiza el servidor, revisa la lista con
> `ollama create --help`.

## Contrato `--json`

- **`stdout`**: un **único objeto JSON** (el sobre final `{"ok":true,...}` o
  `{"ok":false,"error":"..."}`) en todos los subcomandos.
- **`stderr`**: **solo** los eventos de etapa en **NDJSON puro** (`{"stage":"..."}`).
  El logging humano se **silencia** con `--json`.

```console
$ modelforge import ./mi-modelo-safetensors --quantize nvfp4 --name mi-modelo --json
{"stage":"conversión interna"}                       # → stderr
{"stage":"cuantización","value":"nvfp4"}             # → stderr
{"stage":"creación"}                                 # → stderr
{"ok":true,"data":{...}}                             # → stdout (única línea)
```

## Ficheros y filtro

Del directorio se suben los ficheros **relevantes** del snapshot:

| Categoría | Selección |
|---|---|
| Pesos y metadatos | Extensiones de modelo (`.safetensors`, `.json`, tokenizers binarios). |
| Tokenizer de texto | Solo por **nombre**: `vocab.txt` y `merges.txt`. |
| Ignorados | `README.md`, `.gitattributes`, `README.txt`, `requirements.txt`. |

El filtro es determinista y tiene tests unitarios; lo comparten `import` nativo y
`create --from <directorio>`.

## Errores accionables

| Mensaje | Causa |
|---|---|
| `el directorio `X` no contiene ningún fichero `.safetensors`` | El directorio indicado no es un snapshot nativo. |
| `cuantización `X` no soportada; valores válidos: int4, int8, nvfp4, mxfp4, mxfp8` | `--quantize` no es un valor soportado por Ollama. |
| ``--quantize` solo es válido para un directorio de safetensors ...` | Se pidió `--quantize` con una fuente GGUF (Ollama lo rechaza). |
| `HuggingFace: ...` / error del conversor interno | Arquitectura no soportada por el conversor interno de Ollama; se propaga tal cual. |
| `referencia inválida `X`: se espera `hf.co/<repo>:<quant>`` | La referencia `hf.co` no tiene el formato esperado (subcomando `pull`). |
| `el import nativo de safetensors no acepta `--quant, ...`` | Se pasaron flags de repo GGUF (`--quant`/`--mmproj`/`--dest`) al import nativo. |
| `el import de un repo GGUF no acepta `--name, --quantize`` | Se pasaron flags del import nativo (`--name`/`--quantize`) a un repo GGUF. |

## Limitación conocida

**Ollama 0.40.8 en Linux:** la creación desde safetensors requiere el *runtime*
**MLX**, que **no está disponible en Linux**. Ollama responde `503` con
`MLX runtime is not available` y `modelforge` propaga ese error tal cual (código de
salida ≠ 0). El import nativo (y `--quantize`) solo funcionan en plataformas
soportadas por el conversor interno de Ollama (p. ej. **macOS con MLX**). En Linux,
usa el flujo **GGUF** (`import --quant` o `pull`), que sí está soportado.

La cobertura real está diferida a los tests e2e `#[ignore]`, gateados por la
variable `MODELFORGE_TEST_SAFETENSORS`:

```console
# import nativo real (solo en un host soportado por MLX; en Linux Ollama 0.40.8 falla con 503)
MODELFORGE_TEST_SAFETENSORS=/ruta/al/snapshot cargo test -- --ignored imports_safetensors_with_real_ollama
```

## Ficheros implicados

| Fichero | Responsabilidad |
|---|---|
| `src/source.rs` | Clasificación de la fuente (repo GGUF, directorio con `.safetensors`, referencia `hf.co`). |
| `src/pull.rs` | Atajo remoto del hub (`pull`): validación de la referencia y `POST /api/pull`. |
| `src/create.rs` | Generación del Modelfile, subida de blobs y `POST /api/create`. |
| `src/ollama.rs` | Cliente HTTP de Ollama (`/api/version`, `/api/blobs`, `/api/create`, `/api/show`, `/api/pull`). |
| `src/quant.rs` | Validación de los valores de `--quantize` soportados. |
| `src/main.rs` | Subcomandos `import` y `pull`, parseo de flags y contrato de salida. |
| `src/error.rs` | Tipos de error y mensajes accionables. |

## Referencias

- `plans/PLAN-002-ARCHIVED.md` — plan de la feature (issue #8).
- `openspec/specs/modelforge/spec.md` — especificación consolidada del CLI.
