# modelforge Specification

## Purpose
CLI `modelforge` para importar modelos GGUF desde HuggingFace, crear el modelo en Ollama, validarlo y, opcionalmente, añadir su ruta al gateway.

## Requirements

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
