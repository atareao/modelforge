# HANDOFF — modelforge

## Estado

- Proyecto independiente creado en `atareao/modelforge` (local: `/data/rust/modelforge`).
- Issue #1 abierto y asignado.
- Artefactos OpenSpec completos y validados en `openspec/changes/modelforge/`.
- Plan en `plans/PLAN-001.md`.

## Tarea pendiente

- Esperar aprobación del spec y pasar a TDD (RED → GREEN → REFACTOR).
- Implementar `import`, `create`, `test`, `bench`, `list`, `info` y `route`.

## Notas

- No depende de `llm-gateway`; el `route` escribe en `~/.config/opencode/gateway.yaml` con backup.
- Modelo de referencia: `deepseek-v4-pro-9b-heretic:q6_K-256k` (256k, 12 GB VRAM, 100% GPU).