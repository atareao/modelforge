# Tasks

## 1. Import nativo desde safetensors

- [ ] 1.1 Detección del tipo de fuente (repo GGUF / directorio safetensors / `hf.co`) con un enum `SourceKind`, con test unitario.
- [ ] 1.2 Generación de Modelfile con `FROM <directorio>`, con test unitario.
- [ ] 1.3 Implementar `import <dir> --name <n>` de extremo a extremo y verificar con un modelo pequeño real.
- [ ] 1.4 Error claro si el directorio no contiene `.safetensors`, con test.
- [ ] 1.5 Documentar el flujo nativo en `docs/modelforge.md`.

## 2. Cuantización en el import

- [ ] 2.1 Flag `--quantize <QUANT>` con validación contra la lista soportada, con test.
- [ ] 2.2 Enviar el campo `quantize` en `/api/create`, con test.
- [ ] 2.3 Error claro si Ollama rechaza el quant o la arquitectura, con test de integración.
- [ ] 2.4 Documentar `--quantize` en `docs/modelforge.md`.

## 3. Atajo remoto `pull`

- [ ] 3.1 Subcomando `pull hf.co/<repo>:<quant>`, con parseo y validación, y test unitario.
- [ ] 3.2 Delegación en Ollama y reporte del resultado; test de integración (Ollama caído → error claro).
- [ ] 3.3 Documentar `pull` en `docs/modelforge.md`.

## 4. Progreso y salida

- [ ] 4.1 Progreso por etapas (descarga, conversión interna, cuantización, creación) con eventos JSON en `--json`.
- [ ] 4.2 Verificar `--json` y códigos de salida distintos de 0 en los nuevos flujos, con test.

## 5. Verificación

- [ ] 5.1 `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- [ ] 5.2 Flujo real: safetensors → import nativo; y safetensors → `--quantize Q4_K_M` → Ollama.
