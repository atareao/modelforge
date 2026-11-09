# PLAN-002 — Import nativo de modelos (safetensors / HuggingFace hub)

- **Repositorio:** atareao/modelforge
- **Issue:** #8
- **Estado:** propuesto (pendiente de aprobación)
- **Objetivo:** permitir a `modelforge` trabajar con repos de HuggingFace publicados en formato nativo (`.safetensors`) y con GGUF del hub, sin conversión manual previa.

## Alcance

Tres flujos de trabajo:

1. **Import nativo (safetensors):** `modelforge import <dir> --name <modelo>` sobre un directorio con `.safetensors` genera un Modelfile con `FROM <dir>` y crea el modelo en Ollama sin conversión previa.
2. **Conversión + cuantización vía llama.cpp:** `--quantize <QUANT>` invoca `convert-hf-to-gguf.py` (FP16 GGUF intermedio) y `llama-quantize` (formato destino). Las herramientas se detectan en `PATH`; si faltan, error accionable.
3. **Atajo remoto del hub:** `modelforge pull hf.co/<repo>:<quant>` delega en Ollama la descarga y el registro del GGUF del hub.

## Fuera de alcance

- Reimplementar la conversión safetensors→GGUF en Rust.
- Descargar/instalar llama.cpp o Python automáticamente.
- Cambiar el contrato de los comandos existentes (el cambio es aditivo).

## Referencias

- OpenSpec change: `openspec/changes/native-model-import/`
- Propuesta: `openspec/changes/native-model-import/proposal.md`
- Diseño: `openspec/changes/native-model-import/design.md`
- Spec delta: `openspec/changes/native-model-import/specs/modelforge/spec.md`

## Tareas

### 1. Import nativo desde safetensors
- [ ] 1.1 Detección del tipo de fuente (repo GGUF / directorio safetensors / `hf.co`), con test unitario.
- [ ] 1.2 Generación de Modelfile con `FROM <directorio>`, con test unitario.
- [ ] 1.3 `import <dir> --name <n>` de extremo a extremo, verificado con un modelo pequeño real.
- [ ] 1.4 Error claro si el directorio no contiene `.safetensors`, con test.
- [ ] 1.5 Documentar el flujo nativo en `docs/modelforge.md`.

### 2. Conversión y cuantización vía llama.cpp
- [ ] 2.1 Trait `Converter` + detección de `convert-hf-to-gguf.py` y `llama-quantize` en `PATH`, con test.
- [ ] 2.2 Invocación de la conversión a GGUF intermedio, con test (converter falso).
- [ ] 2.3 Cuantización con `llama-quantize` y validación del quant destino, con test.
- [ ] 2.4 Error accionable si faltan las herramientas, con test de integración.
- [ ] 2.5 Documentar `--quantize` en `docs/modelforge.md`.

### 3. Atajo remoto `pull`
- [ ] 3.1 Subcomando `pull hf.co/<repo>:<quant>`, con parseo y validación, y test unitario.
- [ ] 3.2 Delegación en Ollama y reporte del resultado; test de integración (Ollama caído → error claro).
- [ ] 3.3 Documentar `pull` en `docs/modelforge.md`.

### 4. Progreso y salida
- [ ] 4.1 Progreso por etapas (descarga, conversión, cuantización, creación) con eventos JSON en `--json`.
- [ ] 4.2 Verificar `--json` y códigos de salida distintos de 0 en los nuevos flujos, con test.

### 5. Verificación
- [ ] 5.1 `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- [ ] 5.2 Flujo real: safetensors → import nativo; y safetensors → `--quantize Q4_K_M` → Ollama.

## Criterios de aceptación

- Los tres flujos funcionan de extremo a extremo y están documentados.
- Sin `unwrap`/`panic` en producción; red y llama.cpp aislados tras traits.
- Suite verde y CI limpio.