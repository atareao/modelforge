# Tasks

## 1. Import nativo desde safetensors

- [x] 1.1 Detección del tipo de fuente (repo GGUF / directorio safetensors / `hf.co`) con un enum `SourceKind`, con test unitario.
- [x] 1.2 Generación de Modelfile con `FROM <directorio>`, con test unitario.
- [x] 1.3 Implementar `import <dir> --name <n>` de extremo a extremo. Verificación real **diferida**: cubierta por el test `#[ignore]` gateado `imports_safetensors_with_real_ollama` (`tests/native_import_real_ollama.rs`); no ejecutable en este host por falta de *runtime* MLX (Ollama 0.40.8 Linux responde `503 MLX runtime is not available`).
- [x] 1.4 Error claro si el directorio no contiene `.safetensors`, con test.
- [x] 1.5 Documentar el flujo nativo en `docs/modelforge.md`.

## 2. Cuantización en el import

- [x] 2.1 Flag `--quantize <QUANT>` con validación contra la lista soportada, con test.
- [x] 2.2 Enviar el campo `quantize` en `/api/create`, con test.
- [x] 2.3 Error claro si Ollama rechaza el quant o la arquitectura, con test de integración.
- [x] 2.4 Documentar `--quantize` en `docs/modelforge.md`.

## 3. Atajo remoto `pull`

- [x] 3.1 Subcomando `pull hf.co/<repo>:<quant>`, con parseo y validación, y test unitario.
- [x] 3.2 Delegación en Ollama y reporte del resultado; test de integración (Ollama caído → error claro).
- [x] 3.3 Documentar `pull` en `docs/modelforge.md`.

## 4. Progreso y salida

- [x] 4.1 Progreso por etapas (descarga, conversión interna, cuantización, creación) con eventos JSON en `--json`.
- [x] 4.2 Verificar `--json` y códigos de salida distintos de 0 en los nuevos flujos, con test.

## 5. Verificación

- [x] 5.1 `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- [x] 5.2 Flujo real: safetensors → import nativo; y safetensors → `--quantize nvfp4` → Ollama. Cubierto de forma **diferida** por los tests `#[ignore]` gateados `imports_safetensors_with_real_ollama` y `imports_safetensors_quantized_with_real_ollama` (`tests/native_import_real_ollama.rs`); no ejecutables en este host por falta de *runtime* MLX.
