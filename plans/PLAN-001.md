# PLAN-001 — modelforge

- **Repositorio:** atareao/modelforge
- **Issue:** #1
- **Estado:** planificado
- **Objetivo:** CLI independiente `modelforge` para importar modelos GGUF de HuggingFace, crear el modelo en Ollama, validarlo y añadir la ruta al gateway.

## Alcance

- `import`, `create`, `test`, `bench`, `list`, `info`, `route`.
- Descarga reanudable con progreso, Modelfile reproducible, smoke test, benchmark y ruta opcional.

## Tareas

- [ ] 1.1 Crear el proyecto `modelforge` y verificar `cargo check`.
- [ ] 1.2 Añadir dependencias y verificar `cargo check`.
- [ ] 1.3 Configurar `--config`, `--ollama`, `--json`, `--verbose` y verificar `--help`.
- [ ] 2.1 Implementar `import` y verificar descarga real.
- [ ] 2.2 Implementar reanudación y verificar.
- [ ] 2.3 Filtro de quant y error claro, con test.
- [ ] 2.4 Documentar `import`.
- [ ] 3.1 Generar Modelfile, con test.
- [ ] 3.2 Crear vía `/api/create` y verificar con `ollama show`.
- [ ] 3.3 Ollama no disponible, con test de integración.
- [ ] 3.4 Documentar `create`.
- [ ] 4.1 `test` con texto, thinking, tools y vision.
- [ ] 4.2 `bench` con métricas, con test de integración.
- [ ] 4.3 `--compare` y tabla comparativa.
- [ ] 4.4 Documentar `test`/`bench`.
- [ ] 5.1 `list` e `info`, con test de integración.
- [ ] 5.2 `route` con `serde_yaml`, backup y sin duplicados, con test.
- [ ] 5.3 Documentar `route`.
- [ ] 6.1 Flujo completo real y verificar.
- [ ] 6.2 Revisar `docs/modelforge.md` y verificar.
- [ ] 6.3 `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

## Verificación

- Tests unitarios e integración.
- `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- Flujo real con un modelo de HuggingFace.