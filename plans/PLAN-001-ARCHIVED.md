# PLAN-001 — modelforge

- **Repositorio:** atareao/modelforge
- **Issue:** #1
- **Estado:** completado
- **Objetivo:** CLI independiente `modelforge` para importar modelos GGUF de HuggingFace, crear el modelo en Ollama, validarlo y añadir la ruta al gateway.

## Alcance

- `import`, `create`, `test`, `bench`, `list`, `info`, `route`.
- Descarga reanudable con progreso, Modelfile reproducible, smoke test, benchmark y ruta opcional.

## Tareas

- [x] 1.1 Crear el proyecto `modelforge` y verificar `cargo check`.
- [x] 1.2 Añadir dependencias y verificar `cargo check`.
- [x] 1.3 Configurar `--config`, `--ollama`, `--json`, `--verbose` y verificar `--help`.
- [x] 2.1 Implementar `import` y verificar descarga real.
- [x] 2.2 Implementar reanudación y verificar.
- [x] 2.3 Filtro de quant y error claro, con test.
- [x] 2.4 Documentar `import`.
- [x] 3.1 Generar Modelfile, con test.
- [x] 3.2 Crear vía `/api/create` y verificar con `ollama show`.
- [x] 3.3 Ollama no disponible, con test de integración.
- [x] 3.4 Documentar `create`.
- [x] 4.1 `test` con texto, thinking, tools y vision.
- [x] 4.2 `bench` con métricas, con test de integración.
- [x] 4.3 `--compare` y tabla comparativa.
- [x] 4.4 Documentar `test`/`bench`.
- [x] 5.1 `list` e `info`, con test de integración.
- [x] 5.2 `route` con `serde_yaml`, backup y sin duplicados, con test.
- [x] 5.3 Documentar `route`.
- [x] 6.1 Flujo completo real y verificar.
- [x] 6.2 Revisar `docs/modelforge.md` y verificar.
- [x] 6.3 `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

## Verificación

- Tests unitarios e integración.
- `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- Flujo real con un modelo de HuggingFace.