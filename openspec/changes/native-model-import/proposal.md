# Proposal

## Why

`modelforge` solo trabaja hoy con GGUF ya compilados, pero muchos modelos de HuggingFace se publican únicamente en formato nativo (`.safetensors` o `.bin`). El usuario tiene que convertirlos y cuantizarlos a mano antes de crear el modelo en Ollama (issue #8).

## What Changes

- **Import nativo**: si la fuente es un directorio local con `.safetensors`, se genera un Modelfile con `FROM <directorio>` y se crea el modelo en Ollama sin conversión previa.
- **Conversión + cuantización** vía llama.cpp (`convert-hf-to-gguf.py` + `llama-quantize`) mediante el flag `--quantize <QUANT>`. Las herramientas se detectan en `PATH` y, si faltan, se falla con un error accionable.
- Nuevo subcomando **`pull hf.co/<repo>:<quant>`** como atajo para modelos que ya publican GGUF en el hub de HuggingFace.
- Feedback de progreso por etapas en operaciones de minutos (conversión/cuantización).
- Salida `--json` y códigos de salida coherentes con el resto del CLI.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `modelforge`: se añaden requisitos de import nativo desde safetensors, conversión/cuantización vía llama.cpp, atajo remoto del hub y feedback de progreso.

## Impact

- `src/import.rs` (detección de fuente y orquestación), nuevo módulo `src/convert.rs` (llama.cpp), `src/ollama.rs` (pull), `src/cli.rs`/`src/main.rs` (subcomando `pull` y flag `--quantize`).
- Dependencias externas en tiempo de ejecución (solo para `--quantize`): Python con `convert-hf-to-gguf.py` y `llama-quantize` en `PATH`.
- Cambio aditivo: no se modifica el contrato de los comandos existentes.
- Tests unitarios (detección, argumentos, errores) e integración; documentación en `docs/modelforge.md`.
