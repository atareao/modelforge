# HANDOFF — modelforge

## Estado

- Repo independiente: `atareao/modelforge` (local: `/data/rust/modelforge`).
- Issue #1: https://github.com/atareao/modelforge/issues/1
- PR #2 **fusionado** a `development`. Rama `feature/initial-plan` eliminada.
- **Release v0.1.0 publicada**: PR #4 (`development` → `main`) fusionado; tag `v0.1.0` en `main`; release en GitHub.
- Ramas: `main` (producción, v0.1.0) · `development` (integración).
- **CI/CD configurado** (PR #6 → development): `.github/workflows/{ci,release-prepare,release}.yml`, `.vampus.yml`, `cliff.toml`, `GIT_FLOW.md`, `AGENTS.md`.
- Incrementos 1–6: implementación funcional completa y verificada.
- Auditoría Rust: ✅ aprobada.
- Flujo e2e real verificado: import → create → test → bench → route → limpieza.
- Suite: 119 tests passing (los de red/Ollama real van `#[ignore]`).
- `cargo clippy --all-targets -- -D warnings` limpio.
- `cargo fmt --check` limpio.
- OpenSpec: change `modelforge` archivado en `openspec/changes/archive/2026-11-09-modelforge/`; spec fusionada en `openspec/specs/modelforge/spec.md` (`openspec validate modelforge --strict`).
- Plan: `plans/PLAN-001.md` (completado).

## Tarea pendiente

- Configurar secretos en GitHub: `GH_PAT` (contents: write) y `CARGO_REGISTRY_TOKEN`.
- (Opcional) Actualizar `README.md`: aún dice "Proyecto en planificación" y referencia `openspec/changes/modelforge/` (ya archivado).

## Siguiente paso sugerido

- Al fusionar `development` → `main`, `release-prepare.yml` bumpa versión, genera `CHANGELOG.md`, crea tag y sincroniza `development` automáticamente.

## Notas de CI

- `ci.yml`: `fmt --check`, `clippy --all-targets -D warnings`, `test` en PR a `main`/`development`.
- Se corrigió un `unused import` en `src/import.rs` (tests) detectado por el toolchain stable de CI pero no por el local (1.97.0).

## Verificación rápida

```bash
cd /data/rust/modelforge
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
openspec validate modelforge --strict
```

## Notas

- Sin `unwrap`/`panic` en producción.
- Red aislada tras los traits `HfClient` / `OllamaClient`.
- El `gateway.yaml` real del usuario permanece intacto.
- Proyecto independiente de `llm-gateway`; no hay workspace ni crate compartido.
- El `route` escribe en `~/.config/opencode/gateway.yaml` con backup.
- Modelo de referencia: `deepseek-v4-pro-9b-heretic:q6_K-256k` (256k, 12 GB VRAM, 100% GPU).
