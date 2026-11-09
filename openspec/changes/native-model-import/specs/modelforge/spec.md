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
El CLI SHALL, cuando se indique `--quantize <QUANT>`, aplicar la cuantización nativa de Ollama en la creación, sin invocar herramientas externas como llama.cpp o Python. El CLI SHALL aceptar únicamente los valores que soporta la versión de Ollama instalada para la cuantización en la creación (`int4`, `int8`, `nvfp4`, `mxfp4`, `mxfp8` en Ollama 0.40.8), y SHALL rechazar `--quantize` cuando el origen no sea un directorio de safetensors.

#### Scenario: Import con cuantización
- **WHEN** el usuario ejecuta `modelforge import ./modelo --quantize nvfp4 --name mi-modelo`
- **THEN** el CLI crea el modelo `mi-modelo` en Ollama cuantizado a `nvfp4` en un único paso.

#### Scenario: Quant no soportado
- **WHEN** el usuario pasa `--quantize` con un valor no soportado por Ollama (p. ej. `Q4_K_M`)
- **THEN** el CLI falla antes de invocar la creación, indicando los valores válidos.

#### Scenario: Cuantización sobre un GGUF ya convertido
- **WHEN** el usuario pasa `--quantize` con un origen que no es un directorio de safetensors (p. ej. `create --from model.gguf`)
- **THEN** el CLI falla antes de invocar la creación, explicando que Ollama solo cuantiza en la creación a partir de safetensors.

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
El CLI SHALL mostrar el progreso por etapas (descarga, conversión interna, cuantización, creación) durante operaciones que pueden durar varios minutos. Las etapas SHALL anunciarse cuando ocurren, con esos nombres; con `--json`, `stderr` SHALL contener **solo** las líneas NDJSON de etapa (sin logging humano).

#### Scenario: Progreso de creación
- **WHEN** el usuario ejecuta un import con `--quantize`
- **THEN** el CLI indica la etapa en curso (`conversión interna`, `cuantización` con el valor y `creación`) y, con `--json`, emite los eventos de etapa como líneas NDJSON en `stderr`, mientras que `stdout` sigue conteniendo un único objeto JSON (el sobre final `{"ok":true,"data":...}`).

#### Scenario: Import de un repo GGUF
- **WHEN** el usuario importa un repo GGUF (`modelforge import org/model --quant Q6_K`)
- **THEN** el CLI anuncia la etapa `descarga` antes de bajar los ficheros.

#### Scenario: Flags inaplicables en el import nativo
- **WHEN** el usuario pasa `--quant`, `--mmproj` o `--dest` con un directorio de safetensors
- **THEN** el CLI falla con un error claro que nombra los flags no admitidos, en lugar de ignorarlos.

#### Scenario: Flags inaplicables en el import de un repo GGUF
- **WHEN** el usuario pasa `--name` o `--quantize` con un repo GGUF (`modelforge import org/model --quant Q6_K --quantize nvfp4`)
- **THEN** el CLI falla con un error claro que nombra los flags no admitidos, sin llegar a la red.
