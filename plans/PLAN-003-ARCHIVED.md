# PLAN-003 — Puente llama.cpp para safetensors → GGUF (Linux)

- **Repositorio:** atareao/modelforge
- **Issue:** #24
- **Estado:** implementado (task 5.2 real pendiente de entorno con llama.cpp)
- **Objetivo:** permitir a `modelforge` importar safetensors en Linux (y donde el backend nativo MLX no esté disponible) convirtiendo a GGUF con llama.cpp, de forma opcional y sin auto-instalar dependencias.

## Contexto

El import nativo de safetensors (v0.2.0, issue #8) solo funciona en macOS: Ollama enruta safetensors a MLX, sin backend en Linux (`503 MLX runtime is not available`; ollama#17490). Desde Ollama v0.34.1 la creación de GGUF desde safetensors se delega a llama.cpp. La vía oficial en Linux es `convert_hf_to_gguf.py` + `llama-quantize`.

## Alcance

1. **Flag `--backend`** con valores `ollama` (por defecto; nativo, quants `int4/int8/nvfp4/mxfp4/mxfp8`) y `llama.cpp` (GGUF).
2. **Detección en `PATH`** de `convert_hf_to_gguf.py` y `llama-quantize`; si faltan, error accionable (sin auto-install).
3. **Flujo de conversión:** safetensors → GGUF FP16 intermedio (directorio temporal) → (opcional) `llama-quantize` al quant destino → `ollama create` con `FROM <gguf>`.
4. **Quants de llama.cpp** para `--backend llama.cpp`: `Q4_K_M`, `Q5_K_M`, `Q6_K`, `Q8_0`, ... (lista propia, distinta de la nativa).
5. **Progreso por etapas** y `--json` coherentes con el CLI actual.

## Fuera de alcance

- Auto-descarga/instalación de llama.cpp o Python.
- Cambiar el comportamiento del backend nativo (macOS).
- Cuantizar GGUF ya existentes al importar.

## Referencias

- OpenSpec change: `openspec/changes/llamacpp-bridge/`
- Propuesta: `openspec/changes/llamacpp-bridge/proposal.md`
- Diseño: `openspec/changes/llamacpp-bridge/design.md`
- Spec delta: `openspec/changes/llamacpp-bridge/specs/modelforge/spec.md`

## Tareas

### 1. Selección de backend
- [x] 1.1 Flag `--backend <ollama|llama.cpp>` en `import` (clap), con valor por defecto `ollama`, y test unitario de parseo.
- [x] 1.2 Enrutado: safetensors + `--backend llama.cpp` → flujo de conversión; safetensors + `--backend ollama` → flujo nativo actual. Test unitario.

### 2. Detección de herramientas
- [x] 2.1 Trait `Converter` + implementación real que localiza `convert_hf_to_gguf.py` y `llama-quantize` en `PATH`. Test unitario (con doble).
- [x] 2.2 Error accionable `ConverterToolMissing` con instrucciones si falta alguna herramienta. Test.

### 3. Conversión y cuantización
- [x] 3.1 Conversión safetensors → GGUF FP16 (directorio temporal) y test con converter falso.
- [x] 3.2 Cuantización con `llama-quantize` y validación del quant destino contra la lista de llama.cpp. Test.
- [x] 3.3 `ollama create` con `FROM <gguf>` del resultado. Test de integración.
- [x] 3.4 Limpieza del GGUF intermedio y del directorio temporal. Test.

### 4. Integración con el CLI
- [x] 4.1 `import <dir> --backend llama.cpp [--quantize <Q>] --name <m>` de extremo a extremo. Test de integración CLI.
- [x] 4.2 `--json` con eventos por etapa (NDJSON en stderr, objeto único en stdout). Test.
- [x] 4.3 Códigos de salida distintos de 0 en los errores. Test.
- [x] 4.4 Documentar el puente en `docs/native-import.md` (o nuevo `docs/llamacpp-bridge.md`).

### 5. Verificación
- [x] 5.1 `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- [ ] 5.2 Flujo real con llama.cpp instalado: safetensors → `--backend llama.cpp --quantize Q4_K_M` → Ollama (`#[ignore]`, gateado por env).

## Criterios de aceptación

- `--backend llama.cpp` convierte y cuantiza safetensors en Linux de extremo a extremo.
- Si faltan herramientas, error accionable claro, sin auto-install.
- El backend `ollama` (por defecto) mantiene el comportamiento actual.
- Sin `unwrap`/`panic` en producción; llama.cpp aislado tras el trait `Converter`.
- Suite verde y CI limpio.
