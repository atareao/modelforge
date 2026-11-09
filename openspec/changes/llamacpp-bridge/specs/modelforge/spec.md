# Spec Delta

## Purpose

CLI `modelforge` para importar modelos GGUF desde HuggingFace, crear el modelo en Ollama, validarlo y, opcionalmente, añadir su ruta al gateway.

## ADDED Requirements

### Requirement: Selección de backend de import
El CLI SHALL aceptar un flag `--backend` con valores `ollama` (por defecto) y `llama.cpp`, determinando cómo se procesa un directorio de safetensors.

#### Scenario: Backend por defecto
- **WHEN** el usuario ejecuta `modelforge import <dir> --name <m>` sin `--backend`
- **THEN** el CLI usa el backend `ollama` (comportamiento nativo actual).

#### Scenario: Backend llama.cpp
- **WHEN** el usuario ejecuta `modelforge import <dir> --backend llama.cpp --name <m>`
- **THEN** el CLI procesa el directorio con el flujo de llama.cpp en lugar del nativo.

### Requirement: Conversión de safetensors a GGUF con llama.cpp
El CLI SHALL, con `--backend llama.cpp`, convertir un directorio de safetensors a GGUF usando `convert_hf_to_gguf.py` y, si se indica `--quantize`, cuantizar con `llama-quantize`, detectando ambas herramientas en `PATH`.

#### Scenario: Conversión y cuantización completas
- **WHEN** el usuario ejecuta `modelforge import <dir> --backend llama.cpp --quantize Q4_K_M --name <m>` y ambas herramientas están en `PATH`
- **THEN** el CLI genera un GGUF FP16 intermedio, lo cuantiza a `Q4_K_M` y crea el modelo `m` en Ollama a partir del GGUF resultante.

#### Scenario: Conversión sin cuantización
- **WHEN** el usuario ejecuta `modelforge import <dir> --backend llama.cpp --name <m>` sin `--quantize`
- **THEN** el CLI crea el modelo en Ollama a partir del GGUF FP16 convertido.

#### Scenario: Herramientas de llama.cpp ausentes
- **WHEN** se usa `--backend llama.cpp` y `convert_hf_to_gguf.py` o `llama-quantize` no están en `PATH`
- **THEN** el CLI falla con un error accionable que indica cómo instalarlas y con un código de salida distinto de 0.

#### Scenario: Quant de llama.cpp inválido
- **WHEN** el usuario pasa `--quantize` con `--backend llama.cpp` y un valor no soportado
- **THEN** el CLI falla antes de invocar llama.cpp, indicando la lista de quants válidos.

### Requirement: Limpieza de artefactos temporales
El CLI SHALL eliminar el GGUF intermedio y el directorio temporal tras la conversión, tanto en éxito como en error.

#### Scenario: Limpieza tras el éxito
- **WHEN** una conversión con `--backend llama.cpp` termina correctamente
- **THEN** el GGUF FP16 intermedio y el directorio temporal ya no existen.

#### Scenario: Limpieza tras el error
- **WHEN** una conversión con `--backend llama.cpp` falla
- **THEN** el GGUF intermedio y el directorio temporal se eliminan igualmente.

### Requirement: Feedback de progreso en la conversión
El CLI SHALL mostrar el progreso por etapas (conversión, cuantización, creación) durante el flujo de llama.cpp, con eventos JSON en `--json`.

#### Scenario: Progreso de conversión con llama.cpp
- **WHEN** el usuario ejecuta `import --backend llama.cpp --quantize <Q> --json`
- **THEN** `stderr` emite eventos NDJSON por etapa (conversión, cuantización, creación) y `stdout` contiene un único objeto JSON.
