# HANDOFF — modelforge

## Estado

- Repo independiente: `atareao/modelforge` (local: `/data/rust/modelforge`).
- Issue #1: https://github.com/atareao/modelforge/issues/1
- PR #2 abierto a `development`: https://github.com/atareao/modelforge/pull/2
- Rama actual: `feature/initial-plan`
- OpenSpec: `openspec/changes/modelforge/` — validado con `openspec validate modelforge --strict`.
- Plan: `plans/PLAN-001.md`.
- Scaffold Rust: `Cargo.toml` + `src/main.rs` (Hello, world!).

## Tarea pendiente

- Aprobar el spec (proposal + design + spec + tasks).
- Pasar a TDD: RED → GREEN → REFACTOR.
- Implementar `import`, `create`, `test`, `bench`, `list`, `info` y `route`.

## Verificación rápida

```bash
cd /data/rust/modelforge
openspec validate modelforge --strict
cargo check
gh pr view 2 --repo atareao/modelforge
```

## Notas

- Proyecto independiente de `llm-gateway`; no hay workspace ni crate compartido.
- El `route` escribe en `~/.config/opencode/gateway.yaml` con backup.
- Modelo de referencia: `deepseek-v4-pro-9b-heretic:q6_K-256k` (256k, 12 GB VRAM, 100% GPU).
