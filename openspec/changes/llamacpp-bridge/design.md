# Design

## Context

`modelforge` v0.2.0 cubre: import nativo de safetensors (macOS/MLX), repos GGUF y `pull`. La red está aislada tras `HfClient`/`OllamaClient`; `create.rs` orquesta `Stage`/`Progress`. En Linux, Ollama no puede crear desde safetensors (MLX); la vía oficial es llama.cpp. Ver `proposal.md` (issue #24).

## Goals / Non-Goals

**Goals:**
- Convertir e importar safetensors con llama.cpp en Linux, de extremo a extremo.
- Detección de herramientas externas con errores accionables.
- Mantener el backend nativo intacto y aditivo.

**Non-Goals:**
- Auto-descarga/instalación de llama.cpp o Python.
- Reimplementar la conversión en Rust.
- Cuantizar GGUF existentes al importar.

## Decisions

- **Backend seleccionable (`--backend`)**: explícito en vez de autodetección mágica, para que el usuario controle qué motor usa. Alternativa: autodetectar por SO; rechazada por opacidad.
- **Trait `Converter`**: aísla las llamadas a llama.cpp (`convert_hf_to_gguf.py`, `llama-quantize`) para testear con un doble sin depender del entorno.
- **Detección en `PATH`**: localizar el script y `llama-quantize`; si faltan, `AppError::ConverterToolMissing` con instrucciones. Sin auto-install.
- **GGUF intermedio**: FP16 en un directorio temporal, borrado al terminar (éxito o fallo).
- **Quants de llama.cpp**: lista propia (GGUF) validada antes de invocar; convive con la lista nativa de Ollama (backend `ollama`).
- **Progreso**: reutiliza `Stage`/`Progress`; nueva etapa de conversión/cuantización.

## Risks / Trade-offs

- [Herramientas ausentes] → error accionable, sin auto-install.
- [Conversión lenta / mucho CPU/RAM/disco] → aviso de tamaño y progreso por etapas.
- [Script Python con flags variables según versión de llama.cpp] → encapsular en `Converter` y documentar la versión probada.
- [Espacio en disco] → el GGUF intermedio se borra al terminar.

## Migration Plan

Aditivo; sin migración. Rollback: revertir (el flag `--backend llama.cpp` desaparece).

## Open Questions

- (ninguna)
