# Tasks

## 1. Import nativo desde safetensors

- [ ] 1.1 Detección del tipo de fuente (repo GGUF / directorio safetensors / `hf.co`), con test unitario.
- [ ] 1.2 Generación de Modelfile con `FROM <directorio>`, con test unitario.
- [ ] 1.3 Implementar `import <dir> --name <n>` de extremo a extremo y verificar con un modelo pequeño real.
- [ ] 1.4 Error claro si el directorio no contiene `.safetensors`, con test.
- [ ] 1.5 Documentar el flujo nativo en `docs/modelforge.md`.

## 2. Conversión y cuantización vía llama.cpp

- [ ] 2.1 Trait `Converter` + detección de `convert-hf-to-gguf.py` y `llama-quantize` en `PATH`, con test.
- [ ] 2.2 Invocación de la conversión a GGUF intermedio, con test (converter falso).
- [ ] 2.3 Cuantización con `llama-quantize` y validación del quant destino, con test.
- [ ] 2.4 Error accionable si faltan las herramientas, con test de integración.
- [ ] 2.5 Documentar `--quantize` en `docs/modelforge.md`.

## 3. Atajo remoto `pull`

- [ ] 3.1 Subcomando `pull hf.co/<repo>:<quant>`, con parseo y validación, y test unitario.
- [ ] 3.2 Delegación en Ollama y reporte del resultado; test de integración (Ollama caído → error claro).
- [ ] 3.3 Documentar `pull` en `docs/modelforge.md`.

## 4. Progreso y salida

- [ ] 4.1 Progreso por etapas (descarga, conversión, cuantización, creación) con eventos JSON en `--json`.
- [ ] 4.2 Verificar `--json` y códigos de salida distintos de 0 en los nuevos flujos, con test.

## 5. Verificación

- [ ] 5.1 `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- [ ] 5.2 Flujo real: safetensors → import nativo; y safetensors → `--quantize Q4_K_M` → Ollama.
