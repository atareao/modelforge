# Design

## Context

`modelforge` (v0.1.2) ya cubre el flujo GGUF: `import` descarga de HuggingFace, `create` genera el Modelfile y crea en Ollama vía `/api/create`. La red está aislada tras los traits `HfClient`/`OllamaClient`. La documentación oficial de Ollama confirma que importa safetensors nativamente (`Modelfile` con `FROM <directorio>`) y que cuantiza modelos FP16/FP32 con `-q/--quantize` (API: campo `quantize`). Ver `proposal.md` (issue #8).

## Goals / Non-Goals

**Goals:**
- Importar repos en formato nativo (safetensors) con import directo y con cuantización, sin dependencias externas.
- Soportar el atajo remoto de GGUF del hub.
- Feedback de progreso y salida JSON.

**Non-Goals:**
- Usar llama.cpp o Python (`convert-hf-to-gguf.py`, `llama-quantize`).
- Soportar arquitecturas que el conversor interno de Ollama no entiende.
- Cuantizar GGUF existentes al importar (Ollama no lo permite).
- Alterar el comportamiento actual de `import` sobre repos GGUF.
- **Importar safetensors en plataformas donde Ollama no dispone de *runtime* MLX.**
  Ollama 0.40.8 en Linux responde `503 MLX runtime is not available` al crear desde
  safetensors; el import nativo y `--quantize` solo funcionan en hosts soportados
  (p. ej. macOS con MLX). En Linux se documenta como limitación conocida y se usa el
  flujo GGUF (`import --quant` / `pull`). La cobertura real queda diferida a tests
  `#[ignore]` gateados por entorno (`tests/native_import_real_ollama.rs`).

## Decisions

- **Apoyarse en Ollama, no en llama.cpp**: Ollama importa safetensors (`FROM <directorio>`) y cuantiza en el propio `create`. Evita dependencias externas y errores de entorno. Alternativa: orquestar `convert-hf-to-gguf.py` + `llama-quantize`; rechazada por fragilidad e innecesaria.
- **Detección de fuente**: un enum `SourceKind` (Repo GGUF / Directorio safetensors / `hf.co`) resuelve el camino a seguir a partir del argumento. Alternativa: flags separados; rechazada por multiplicar la superficie de CLI.
- **Cuantización**: se envía el campo `quantize` en `/api/create`; se valida el valor contra la lista soportada antes de llamar. Alternativa: `--quantize` solo informativo; rechazada.
- **`pull`**: delega en Ollama (`ollama pull hf.co/...` o el flujo equivalente por API), que ya soporta GGUF del hub. Validación estricta del formato `hf.co/<repo>:<quant>`.
- **Progreso**: etapas nombradas; con `--json`, eventos por etapa.

## Risks / Trade-offs

- [Arquitectura no soportada] → error claro desde Ollama, propagado como error accionable; documentado como limitación.
- [Versión de Ollama antigua sin `quantize`] → detectar/avisar si la creación falla por campo no soportado.
- [Nombres de quant de Ollama] → validar contra lista conocida antes de invocar.
- [Copia de pesos] → el import nativo no copia ni convierte; Ollama lee el directorio.
- [Runtime MLX ausente] → en Linux Ollama 0.40.8 falla con `503 MLX runtime is not
  available`; se propaga tal cual y se documenta como limitación (usar flujo GGUF).

## Migration Plan

Aditivo; sin migración de datos. Rollback: revertir el cambio (los comandos nuevos desaparecen).

## Open Questions

- (ninguna)
