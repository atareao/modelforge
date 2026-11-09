# Proposal

## Why

El import nativo de safetensors (issue #8) solo funciona en macOS: Ollama enruta safetensors a su motor MLX, que no tiene backend en Linux (`503 MLX runtime is not available`; ollama#17490). En Linux, la vía soportada por Ollama desde v0.34.1 es convertir a GGUF con llama.cpp. `modelforge` debe cubrir ese caso.

## What Changes

- Nuevo flag **`--backend <ollama|llama.cpp>`** en `import` (por defecto `ollama`).
- **Backend `llama.cpp`**: convierte safetensors → GGUF con `convert_hf_to_gguf.py` y, opcionalmente, cuantiza con `llama-quantize` antes de crear el modelo en Ollama.
- **Detección en `PATH`** de ambas herramientas; si faltan, error accionable (sin auto-install).
- **Quants propios** de llama.cpp (`Q4_K_M`, `Q5_K_M`, `Q6_K`, `Q8_0`, ...), distintos de los nativos de Ollama.
- Limpieza de artefactos temporales (GGUF FP16 intermedio).

## Capabilities

### New Capabilities
- (ninguna)

### Modified Capabilities
- `modelforge`: se añaden requisitos de conversión/import vía llama.cpp (backend alternativo) y detección de herramientas externas.

## Impact

- Nuevo módulo `src/convert.rs` (trait `Converter`), cambios en `src/cli.rs`, `src/main.rs`, `src/create.rs` (o `src/import.rs`), `src/error.rs`, `src/quant.rs` (lista de quants de llama.cpp).
- Dependencias en tiempo de ejecución **solo** para `--backend llama.cpp`: Python + llama.cpp (`convert_hf_to_gguf.py`, `llama-quantize`) en `PATH`.
- Cambio aditivo: el backend `ollama` por defecto no cambia.
- Tests unitarios e integración; documentación.
