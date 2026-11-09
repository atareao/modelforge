# modelforge

CLI en Rust para importar modelos GGUF desde HuggingFace, crear el modelo en Ollama, probarlo, medir su rendimiento y, opcionalmente, añadir su ruta a un gateway OpenAI-compatible.

## Estado

Proyecto en planificación. El plan está en `plans/PLAN-001.md` y la especificación OpenSpec en `openspec/changes/modelforge/`.

## Comandos previstos

```text
modelforge import <repo> --quant Q6_K [--mmproj] [--dest <dir>]
modelforge create <name> --from <gguf> [--mmproj <mmproj>] [--ctx 262144] [--renderer ...] [--parser ...]
modelforge test <name> [--text] [--thinking] [--tools] [--vision] [--bench]
modelforge bench <name> [--compare <model>] [--tokens 256] [--json]
modelforge list
modelforge info <name>
modelforge route <name> --model <model> [--gateway-config <path>]
```

## Documentación

- Plan: `plans/PLAN-001.md`
- Spec: `openspec/changes/modelforge/specs/modelforge/spec.md`
- Documentación de uso: `docs/modelforge.md` (pendiente de implementación)