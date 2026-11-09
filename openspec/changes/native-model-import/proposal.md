# Proposal

## Why

`modelforge` solo trabaja hoy con GGUF ya compilados, pero muchos modelos de HuggingFace se publican únicamente en formato nativo (`.safetensors`). Ollama ya sabe importar safetensors y cuantizarlos en un solo paso, pero `modelforge` no lo aprovecha (issue #8).

## What Changes

- **Import nativo**: si la fuente es un directorio local con `.safetensors`, se genera un Modelfile con `FROM <directorio>` y se crea el modelo en Ollama sin conversión previa.
- **Cuantización nativa en el import**: el flag `--quantize <QUANT>` se traduce al campo `quantize` de `POST /api/create`, sin herramientas externas (ni Python ni llama.cpp).
- Nuevo subcomando **`pull hf.co/<repo>:<quant>`** como atajo para modelos que ya publican GGUF en el hub de HuggingFace.
- Feedback de progreso por etapas en operaciones de minutos.
- Salida `--json` y códigos de salida coherentes con el resto del CLI.

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `modelforge`: se añaden requisitos de import nativo desde safetensors, cuantización nativa en el import, atajo remoto del hub y feedback de progreso.

## Impact

- `src/import.rs` (detección de fuente y orquestación), `src/modelfile.rs` (`FROM <directorio>`), `src/ollama.rs` (campo `quantize` y `pull`), `src/cli.rs`/`src/main.rs` (subcomando `pull` y flag `--quantize`).
- Sin dependencias externas en tiempo de ejecución.
- Cambio aditivo: no se modifica el contrato de los comandos existentes.
- Tests unitarios (detección, argumentos, errores) e integración; documentación en `docs/modelforge.md`.
