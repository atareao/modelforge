# llamacpp-bridge

Documenta la feature **llamacpp-bridge** (issue #24): convertir un directorio de
safetensors a **GGUF** con `llama.cpp` antes de crear el modelo en Ollama. Es un
puente **opcional y aditivo**: el backend nativo (`ollama`) no cambia y sigue
siendo el valor por defecto.

## Qué es

El import nativo de safetensors (documentado en
[`native-import.md`](./native-import.md)) solo funciona donde Ollama puede crear
desde un snapshot nativo. En **Linux** Ollama enruta los safetensors a su motor
MLX, que no tiene backend: responde `503 {"error":"MLX runtime is not available"}`
([ollama#17490](https://github.com/ollama/ollama/issues/17490)). Desde Ollama
v0.34.1 la vía soportada es pasar por GGUF.

`modelforge` **no reimplementa** la conversión: delega en las herramientas
oficiales de `llama.cpp` (`convert_hf_to_gguf.py` y `llama-quantize`) y crea el
modelo en Ollama a partir del GGUF resultante.

## Cómo se usa

```console
# Convertir e importar (sin cuantizar)
modelforge import ./mi-modelo-safetensors --backend llama.cpp --name mi-modelo

# Convertir, cuantizar e importar
modelforge import ./mi-modelo-safetensors --backend llama.cpp --quantize Q4_K_M --name mi-modelo
```

El flag `--backend` acepta `ollama` (por defecto) y `llama.cpp`, y **solo** aplica
a `import` con un **directorio local de safetensors**. Con un repo GGUF
(`org/modelo`) o una referencia del hub (`hf.co/...`) se rechaza con un error
claro (esos ya son GGUF: usa `--quant`).

## El pipeline

```text
safetensors ──convert_hf_to_gguf.py──▶ model-f16.gguf ──llama-quantize──▶ model-<QUANT>.gguf ──▶ ollama create
   (dir)                                   (FP16, temporal)                     (temporal)
```

1. **Validación del quant.** Si se pasa `--quantize`, se valida contra la lista
   propia de llama.cpp **antes** de invocar nada (error accionable con los valores
   válidos). La comparación ignora mayúsculas/minúsculas y se canonicaliza a
   mayúsculas (`q4_k_m` → `Q4_K_M`).
2. **Detección de herramientas en `PATH`.** Se localizan `python3`,
   `convert_hf_to_gguf.py` y `llama-quantize`. Los binarios (`python3`,
   `llama-quantize`) deben existir **y ser ejecutables**; el script `.py` solo tiene
   que existir (se invoca con `python3`). Si falta alguno, el CLI falla con un error
   accionable (cómo instalarlas), **sin auto-install**.
3. **Conversión.** En un directorio temporal:
   `python3 <convert_hf_to_gguf.py> <dir-safetensors> --outfile <tmp>/model-f16.gguf --outtype f16`.
4. **Cuantización (opcional).**
   `llama-quantize <tmp>/model-f16.gguf <tmp>/model-<QUANT>.gguf <QUANT>`.
5. **Creación en Ollama.** `ollama create` con `FROM <gguf>` (se reutiliza
   `create::run`).
6. **Limpieza.** El directorio temporal (y el GGUF intermedio) se eliminan
   **siempre**, tanto en éxito como en error.

## Quants soportados (`--backend llama.cpp`)

Lista propia de llama.cpp (GGUF), distinta de la nativa de Ollama
(`int4`/`int8`/`nvfp4`/`mxfp4`/`mxfp8`):

```text
Q2_K, Q3_K_S, Q3_K_M, Q3_K_L, Q4_0, Q4_1, Q4_K_S, Q4_K_M, Q5_0, Q5_1,
Q5_K_S, Q5_K_M, Q6_K, Q8_0, IQ4_NL, IQ4_XS, MXFP4
```

- `--backend ollama --quantize <Q>` valida contra la lista **nativa** de Ollama.
- `--backend llama.cpp --quantize <Q>` valida contra la lista de **llama.cpp**.

## Progreso y `--json`

Se reutiliza el mecanismo de etapas de `create`. Las etapas del flujo llama.cpp
son, en orden: `conversión`, `cuantización` (si se pidió) y `creación`.

Con `--json` el contrato se mantiene: **`stdout` = un único objeto JSON** (el
sobre final) y **`stderr` = eventos NDJSON por etapa** (`{"stage":...}`), con el
logging humano silenciado. Ejemplo de `stderr`:

```text
{"stage":"conversión"}
{"stage":"cuantización","value":"Q4_K_M"}
{"stage":"creación"}
```

## Requisitos en tiempo de ejecución

Solo para `--backend llama.cpp`:

- **Python 3** (`python3`) en `PATH`.
- **llama.cpp** con `convert_hf_to_gguf.py` y `llama-quantize` en `PATH`
  (<https://github.com/ggml-org/llama.cpp>). El script vive en la raíz del repo
  de llama.cpp; el binario se compila con CMake.

No se descarga ni se instala nada automáticamente.

## Ficheros implicados

| Fichero | Rol |
|---|---|
| `src/cli.rs` | Flag `--backend <ollama\|llama.cpp>` (enum `Backend`, `ValueEnum`). |
| `src/convert.rs` | Trait `Converter`, implementación real `LlamaCppConverter`, orquestación (`run`) y directorio temporal con limpieza (RAII). |
| `src/main.rs` | Enrutado (`SafetensorsDir` + `--backend llama.cpp` → `import_llamacpp`) y errores. |
| `src/quant.rs` | `SUPPORTED_LLAMACPP_QUANTS` + `canonical_llamacpp_quant` (lista propia, sin tocar la nativa). |
| `src/error.rs` | `ConverterToolMissing`, `ConverterFailed`, `LlamaCppUnsupportedSource`. |
| `src/create.rs` | Etapa `Stage::ConvertingSafetensors` (`conversión`). |

## Decisiones y follow-ups

- **Backend explícito** (no autodetección por SO): el usuario controla el motor.
- **Aislamiento con el trait `Converter`**: la orquestación, el enrutado y la
  limpieza se prueban con un doble, sin depender de que llama.cpp esté instalado.
  El flujo real va en tests `#[ignore]` gateados por `MODELFORGE_TEST_LLAMACPP=1`.
- **Modelfile transitorio**: el GGUF vive en un directorio temporal que se elimina
  al terminar, así que el reporte **omite** el `Modelfile` (no expone una ruta
  inexistente). El import nativo, cuyo origen persiste, sí lo incluye. El modelo
  queda creado en Ollama.
- **Herramientas asíncronas**: `LlamaCppConverter` invoca los procesos con
  `tokio::process::Command`, para no bloquear el reactor de Tokio durante la
  conversión/cuantización.
- **Follow-up**: revisar los flags de `convert_hf_to_gguf.py` si cambian entre
  versiones de llama.cpp (encapsulados en `LlamaCppConverter::convert`).
