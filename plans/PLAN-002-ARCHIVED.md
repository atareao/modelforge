# PLAN-002 — Import nativo de modelos (safetensors / HuggingFace hub)

- **Repositorio:** atareao/modelforge
- **Issue:** #8
- **Estado:** propuesto (pendiente de aprobación)
- **Objetivo:** permitir a `modelforge` importar modelos publicados en formato nativo (`.safetensors`) y GGUF del hub, apoyándose en las capacidades nativas de Ollama, **sin Python ni llama.cpp**.

## Alcance

Tres flujos de trabajo, todos nativos de Ollama:

1. **Import nativo (safetensors):** `modelforge import <dir> --name <modelo>` sobre un directorio con `.safetensors` y metadatos genera un Modelfile con `FROM <dir>` y crea el modelo.
2. **Import + cuantización:** `--quantize <QUANT>` aplica el `quantize` nativo de Ollama en `/api/create` (un solo paso, sin herramientas externas).
3. **Atajo remoto del hub:** `modelforge pull hf.co/<repo>:<quant>` delega en Ollama la descarga y el registro del GGUF del hub.

## Fuera de alcance

- Conversión/cuantización vía llama.cpp o Python.
- Arquitecturas no soportadas por el conversor interno de Ollama.
- Cuantizar GGUF existentes al importar (Ollama no lo permite).

## Decisión clave

La doc de Ollama confirma que importa safetensors nativamente (`Modelfile` con `FROM <directorio>`) y cuantiza modelos FP16/FP32 con `-q/--quantize` (API: campo `quantize`). Por tanto el flujo del issue #8 **no necesita dependencias externas**.

## Referencias

- OpenSpec change: `openspec/changes/native-model-import/`
- Propuesta: `openspec/changes/native-model-import/proposal.md`
- Diseño: `openspec/changes/native-model-import/design.md`
- Spec delta: `openspec/changes/native-model-import/specs/modelforge/spec.md`

## Tareas

### 1. Import nativo desde safetensors
- [x] 1.1 Detección del tipo de fuente (repo GGUF / directorio safetensors / `hf.co`) con un enum `SourceKind`, con test unitario.
- [x] 1.2 Generación de Modelfile con `FROM <directorio>`, con test unitario.
- [x] 1.3 `import <dir> --name <n>` de extremo a extremo, verificado con un modelo pequeño real. Verificación real **diferida**: test `#[ignore]` gateado `imports_safetensors_with_real_ollama` (`tests/native_import_real_ollama.rs`); no ejecutable en este host por falta de *runtime* MLX (Ollama 0.40.8 Linux → `503 MLX runtime is not available`).
- [x] 1.4 Error claro si el directorio no contiene `.safetensors`, con test.
- [x] 1.5 Documentar el flujo nativo en `docs/modelforge.md`.

### 2. Cuantización en el import
- [x] 2.1 Flag `--quantize <QUANT>` con validación contra la lista soportada, con test.
- [x] 2.2 Enviar el campo `quantize` en `/api/create`, con test.
- [x] 2.3 Error claro si Ollama rechaza el quant o la arquitectura, con test de integración.
- [x] 2.4 Documentar `--quantize` en `docs/modelforge.md`.

### 3. Atajo remoto `pull`
- [x] 3.1 Subcomando `pull hf.co/<repo>:<quant>`, con parseo y validación, y test unitario.
- [x] 3.2 Delegación en Ollama y reporte del resultado; test de integración (Ollama caído → error claro).
- [x] 3.3 Documentar `pull` en `docs/modelforge.md`.

### 4. Progreso y salida
- [x] 4.1 Progreso por etapas (descarga, conversión interna, cuantización, creación) con eventos JSON en `--json`.
- [x] 4.2 Verificar `--json` y códigos de salida distintos de 0 en los nuevos flujos, con test.

### 5. Verificación
- [x] 5.1 `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- [x] 5.2 Flujo real: safetensors → import nativo; y safetensors → `--quantize nvfp4` → Ollama. Cubierto de forma **diferida** por los tests `#[ignore]` gateados `imports_safetensors_with_real_ollama` y `imports_safetensors_quantized_with_real_ollama` (`tests/native_import_real_ollama.rs`); no ejecutables en este host por falta de *runtime* MLX.

## Criterios de aceptación

- Los tres flujos funcionan de extremo a extremo y están documentados.
- Sin dependencias externas (ni Python ni llama.cpp).
- Sin `unwrap`/`panic` en producción; red aislada tras traits.
- Suite verde y CI limpio.
