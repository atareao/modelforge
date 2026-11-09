# Spec Delta

## Purpose

CLI `modelforge` para importar modelos GGUF desde HuggingFace, crear el modelo en Ollama, validarlo y, opcionalmente, añadir su ruta al gateway.

## ADDED Requirements

### Requirement: Importar un modelo nativo desde safetensors
El CLI SHALL aceptar como fuente un directorio local con ficheros `.safetensors` y crear el modelo en Ollama de forma nativa, generando un Modelfile con `FROM <directorio>` sin conversión previa.

#### Scenario: Directorio de safetensors válido
- **WHEN** el usuario ejecuta `modelforge import ./modelo --name mi-modelo` sobre un directorio con `.safetensors` y metadatos
- **THEN** el CLI genera un Modelfile con `FROM ./modelo` y crea el modelo `mi-modelo` en Ollama.

#### Scenario: Directorio sin safetensors
- **WHEN** el directorio indicado no contiene ficheros `.safetensors`
- **THEN** el CLI falla con un error claro y un código de salida distinto de 0.

### Requirement: Conversión y cuantización vía llama.cpp
El CLI SHALL, cuando se indique `--quantize <QUANT>`, convertir los safetensors a GGUF con `convert-hf-to-gguf.py` y cuantizar con `llama-quantize`, detectando ambas herramientas en `PATH` y fallando con un error accionable si no están disponibles.

#### Scenario: Conversión y cuantización completas
- **WHEN** el usuario ejecuta `modelforge import ./modelo --quantize Q4_K_M --name mi-modelo` y ambas herramientas están en `PATH`
- **THEN** el CLI genera un GGUF intermedio, lo cuantiza a `Q4_K_M`, crea el Modelfile y registra el modelo en Ollama.

#### Scenario: Herramientas de llama.cpp ausentes
- **WHEN** se usa `--quantize` y `convert-hf-to-gguf.py` o `llama-quantize` no están en `PATH`
- **THEN** el CLI falla con un error accionable que indica cómo instalarlas y con un código de salida distinto de 0.

#### Scenario: Quant de destino inválido
- **WHEN** el usuario pasa `--quantize` con un valor no soportado
- **THEN** el CLI falla antes de invocar llama.cpp, indicando la lista de quants válidos.

### Requirement: Atajo remoto de HuggingFace Hub
El CLI SHALL aceptar `modelforge pull hf.co/<repo>:<quant>` y delegar en Ollama la descarga y el registro del modelo GGUF alojado en el hub.

#### Scenario: Pull de un GGUF del hub
- **WHEN** el usuario ejecuta `modelforge pull hf.co/usuario/repo:Q4_K_M`
- **THEN** el CLI solicita a Ollama la descarga y muestra el resultado.

#### Scenario: Referencia de hub inválida
- **WHEN** la referencia no tiene el formato `hf.co/<repo>:<quant>`
- **THEN** el CLI falla con un error claro y un código de salida distinto de 0.

### Requirement: Feedback de progreso en operaciones largas
El CLI SHALL mostrar el progreso por etapas (descarga, conversión, cuantización, creación) durante operaciones que pueden durar varios minutos.

#### Scenario: Progreso de conversión
- **WHEN** el usuario ejecuta una conversión con `--quantize`
- **THEN** el CLI indica la etapa en curso y, con `--json`, emite eventos JSON por etapa.
