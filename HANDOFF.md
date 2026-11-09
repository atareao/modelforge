# HANDOFF — modelforge

## Estado

- Repo independiente: `atareao/modelforge` (local: `/data/rust/modelforge`).
- Issue #1: https://github.com/atareao/modelforge/issues/1
- PR #2 **fusionado** a `development`. Rama `feature/initial-plan` eliminada.
- **Release v0.1.1 publicada automáticamente**: PR #11 (`development` → `main`) → `release-prepare.yml` bumpó 0.1.0→0.1.1, generó `CHANGELOG.md`, tag `v0.1.1` y sincronizó `development`; `release.yml` compiló binarios x86_64/aarch64 y creó la GitHub Release.
- Ramas: `main` (producción, v0.1.1) · `development` (integración). `development` == `main`.
- **CI/CD configurado**: `.github/workflows/{ci,release-prepare,release}.yml`, `.vampus.yml`, `cliff.toml`, `GIT_FLOW.md`, `AGENTS.md`.
- Secretos `GH_PAT` y `CARGO_REGISTRY_TOKEN` configurados en el repo.
- El job `publish` a crates.io se eliminó (el nombre `modelforge` ya está ocupado en crates.io).
- Incrementos 1–6: implementación funcional completa y verificada.
- Auditoría Rust: ✅ aprobada.
- Flujo e2e real verificado: import → create → test → bench → route → limpieza.
- Suite: 119 tests passing (los de red/Ollama real van `#[ignore]`).
- `cargo clippy --all-targets -- -D warnings` limpio.
- `cargo fmt --check` limpio.
- OpenSpec: change `modelforge` archivado en `openspec/changes/archive/2026-11-09-modelforge/`; spec fusionada en `openspec/specs/modelforge/spec.md` (`openspec validate modelforge --strict`).
- Plan: `plans/PLAN-001.md` (completado).

## Tarea pendiente

- Ninguna. Flujo de release automático validado de extremo a extremo.

- Licencia **MIT** añadida (`LICENSE`, Copyright © 2026 Lorenzo Carbonell) + `description`/`license`/`repository` en `Cargo.toml`.
- `Cargo.lock` sincronizado y `release-prepare.yml` corregido (`cargo update -p modelforge` tras el bump).

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
