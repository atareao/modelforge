# Design

## Context

`modelforge` (v0.1.2) ya cubre el flujo GGUF: `import` descarga de HuggingFace, `create` genera el Modelfile y crea en Ollama. La red está aislada tras los traits `HfClient`/`OllamaClient`. Ollama soporta crear un modelo con `FROM <directorio>` cuando el directorio contiene safetensors y metadatos. llama.cpp provee `convert-hf-to-gguf.py` y `llama-quantize`. Ver `proposal.md` (issue #8).

## Goals / Non-Goals

**Goals:**
- Soportar repos en formato nativo (safetensors) con import directo y con conversión+cuantización.
- Soportar el atajo remoto de GGUF del hub.
- Detección de herramientas externas con errores accionables.
- Feedback de progreso y salida JSON.

**Non-Goals:**
- Reimplementar la conversión safetensors→GGUF en Rust.
- Descargar/instalar llama.cpp o Python automáticamente.
- Alterar el comportamiento actual de `import` sobre repos GGUF.

## Decisions

- **Detección de fuente**: un enum `SourceKind` (Repo GGUF / Directorio safetensors / `hf.co`) resuelve el camino a seguir a partir del argumento. Alternativa: flags separados; rechazada por multiplicar la superficie de CLI.
- **Import nativo**: Modelfile con `FROM <directorio>`; no se copia ni convierte el modelo. Alternativa: convertir siempre; rechazada por coste.
- **Conversión**: se invoca `convert-hf-to-gguf.py --outfile <tmp.gguf> <dir>` y luego `llama-quantize <tmp.gguf> <out.gguf> <QUANT>`. Se aísla tras un trait `Converter` para testear sin llama.cpp real.
- **Detección en PATH**: `which`/lookup de `convert-hf-to-gguf.py` y `llama-quantize`; si faltan, error con instrucciones de instalación. Sin auto-install.
- **`pull`**: delega en Ollama (`ollama pull hf.co/...` o la API), que ya soporta GGUF del hub. Validación estricta del formato `hf.co/<repo>:<quant>`.
- **Progreso**: etapas nombradas; con `--json`, eventos por etapa.

## Risks / Trade-offs

- [Herramientas ausentes] → error accionable, sin auto-install.
- [Conversión lenta / mucho CPU/RAM] → feedback por etapas y aviso de tamaño estimado.
- [Quants de llama.cpp vs Ollama] → validar el destino contra una lista conocida antes de invocar.
- [Espacio en disco] → avisar del tamaño estimado del GGUF intermedio y final.

## Migration Plan

Aditivo; sin migración de datos. Rollback: revertir el cambio (los comandos nuevos desaparecen).

## Open Questions

- (ninguna)
