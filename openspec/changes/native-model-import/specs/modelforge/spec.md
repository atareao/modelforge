# Spec Delta

## Purpose

CLI `modelforge` para importar modelos GGUF desde HuggingFace, crear el modelo en Ollama, validarlo y, opcionalmente, añadir su ruta al gateway.

## ADDED Requirements

### Requirement: Importar un modelo nativo desde safetensors
El CLI SHALL aceptar como fuente un directorio local con ficheros `.safetensors` y crear el modelo en Ollama de forma nativa, generando un Modelfile con `FROM <directorio>` sin conversión previa ni dependencias externas.

#### Scenario: Directorio de safetensors válido
- **WHEN** el usuario ejecuta `modelforge import ./modelo --name mi-modelo` sobre un directorio con `.safetensors` y metadatos (`config.json`, tokenizer)
- **THEN** el CLI genera un Modelfile con `FROM ./modelo` y crea el modelo `mi-modelo` en Ollama.

#### Scenario: Directorio sin safetensors
- **WHEN** el directorio indicado no contiene ficheros `.safetensors`
- **THEN** el CLI falla con un error claro y un código de salida distinto de 0.

### Requirement: Cuantizar un modelo nativo durante el import
El CLI SHALL, cuando se indique `--quantize <QUANT>`, aplicar la cuantización nativa de Ollama en la creación, sin invocar herramientas externas como llama.cpp o Python.

#### Scenario: Import con cuantización
- **WHEN** el usuario ejecuta `modelforge import ./modelo --quantize Q4_K_M --name mi-modelo`
- **THEN** el CLI crea el modelo `mi-modelo` en Ollama cuantizado a `Q4_K_M` en un único paso.

#### Scenario: Quant no soportado
- **WHEN** el usuario pasa `--quantize` con un valor no soportado por Ollama
- **THEN** el CLI falla antes de invocar la creación, indicando los valores válidos.

#### Scenario: Arquitectura no soportada
- **WHEN** el directorio contiene safetensors de una arquitectura que el conversor interno de Ollama no soporta
- **THEN** el CLI falla con un error claro que lo indica y un código de salida distinto de 0.

### Requirement: Atajo remoto de HuggingFace Hub
El CLI SHALL aceptar `modelforge pull hf.co/<repo>:<quant>` y delegar en Ollama la descarga y el registro del modelo GGUF alojado en el hub.

#### Scenario: Pull de un GGUF del hub
- **WHEN** el usuario ejecuta `modelforge pull hf.co/usuario/repo:Q4_K_M`
- **THEN** el CLI solicita a Ollama la descarga y muestra el resultado.

#### Scenario: Referencia de hub inválida
- **WHEN** la referencia no tiene el formato `hf.co/<repo>:<quant>`
- **THEN** el CLI falla con un error claro y un código de salida distinto de 0.

### Requirement: Feedback de progreso en operaciones largas
El CLI SHALL mostrar el progreso por etapas (descarga, conversión interna, cuantización, creación) durante operaciones que pueden durar varios minutos.

#### Scenario: Progreso de creación
- **WHEN** el usuario ejecuta un import con `--quantize`
- **THEN** el CLI indica la etapa en curso y, con `--json`, emite eventos JSON por etapa.
