# Spec Delta

## Purpose

CLI `modelforge` para importar modelos GGUF desde HuggingFace, crear el modelo en Ollama, validarlo y, opcionalmente, añadir su ruta al gateway.

## ADDED Requirements

### Requirement: Importar un modelo desde HuggingFace
El CLI SHALL resolver un repo de HuggingFace, listar sus archivos, seleccionar el quant indicado y descargar el GGUF y, si se pide, el mmproj, con progreso y reanudación.

#### Scenario: Descarga de un GGUF con mmproj
- **WHEN** el usuario ejecuta `modelforge import <repo> --quant Q6_K --mmproj`
- **THEN** el CLI descarga el GGUF y el mmproj a `--dest` y muestra progreso y tamaño.

#### Scenario: Descarga interrumpida
- **WHEN** una descarga se interrumpe y se relanza el mismo comando
- **THEN** el CLI reanuda desde el byte disponible sin volver a descargar lo ya bajado.

#### Scenario: Quant inexistente
- **WHEN** el repo no contiene el quant solicitado
- **THEN** el CLI falla con un error claro y un código de salida distinto de 0.

### Requirement: Crear un modelo en Ollama
El CLI SHALL generar un Modelfile con `FROM` del GGUF y del mmproj, `RENDERER`, `PARSER`, `num_ctx` y parámetros de muestreo, y crear el modelo vía `/api/create`.

#### Scenario: Creación con visión
- **WHEN** el usuario ejecuta `modelforge create <name> --from <gguf> --mmproj <mmproj> --ctx 262144`
- **THEN** el modelo se crea con `vision` y `num_ctx 262144`.

#### Scenario: Ollama no disponible
- **WHEN** Ollama no responde en `--ollama`
- **THEN** el CLI falla con un error claro y un código de salida distinto de 0.

### Requirement: Smoke test del modelo
El CLI SHALL ejecutar pruebas de texto, thinking, tools y vision, y reportar el resultado en JSON.

#### Scenario: Tool-calling
- **WHEN** el usuario ejecuta `modelforge test <name> --tools`
- **THEN** el CLI envía una herramienta y verifica que el modelo emite `tool_calls`.

#### Scenario: Vision
- **WHEN** el usuario ejecuta `modelforge test <name> --vision`
- **THEN** el CLI envía una imagen y verifica que el modelo responde.

### Requirement: Benchmark
El CLI SHALL medir `eval_count`, `eval_duration`, `load_duration`, `size_vram`, `context` y `processor`, y calcular tokens/s.

#### Scenario: Benchmark con comparación
- **WHEN** el usuario ejecuta `modelforge bench <name> --compare <model>`
- **THEN** el CLI ejecuta el mismo prompt en ambos y reporta una tabla comparativa.

### Requirement: Listar e inspeccionar modelos
El CLI SHALL listar los modelos de Ollama y mostrar el contexto, VRAM y processor de un modelo.

#### Scenario: Listado
- **WHEN** el usuario ejecuta `modelforge list`
- **THEN** el CLI muestra los modelos con su contexto y VRAM.

### Requirement: Añadir ruta al gateway
El CLI SHALL añadir una ruta al `gateway.yaml` con backup, sin duplicar y validando el YAML.

#### Scenario: Ruta nueva
- **WHEN** el usuario ejecuta `modelforge route <name> --model <model>`
- **THEN** el CLI añade la ruta y crea un backup del `gateway.yaml`.

#### Scenario: Ruta existente
- **WHEN** la ruta ya existe
- **THEN** el CLI no la duplica y avisa al usuario.

### Requirement: Salida JSON y códigos de salida
El CLI SHALL ofrecer `--json` en los comandos y usar códigos de salida distintos de 0 para errores.

#### Scenario: Error en JSON
- **WHEN** el usuario ejecuta `modelforge import ... --json` y falla
- **THEN** el CLI devuelve un JSON con el error y un código de salida distinto de 0.