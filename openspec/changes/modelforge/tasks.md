# Tasks

## 1. Setup

- [ ] 1.1 Crear el proyecto `modelforge` (Cargo.toml y src/main.rs) y verificar que `cargo check` compila.
- [ ] 1.2 Añadir dependencias (`clap`, `tokio`, `reqwest`, `serde`, `serde_json`, `serde_yaml`, `hf-hub`, `indicatif`, `anyhow`, `tracing`) y verificar que `cargo check` sigue pasando.
- [ ] 1.3 Configurar `--config`, `--ollama`, `--json` y `--verbose` y verificar `modelforge --help`.

## 2. Importación desde HuggingFace

- [ ] 2.1 Implementar `import <repo> --quant <q> --mmproj` con `hf-hub` y verificar descarga real en un repo público.
- [ ] 2.2 Implementar reanudación (`.part` + `Range`) y verificar interrumpiendo y relanzando.
- [ ] 2.3 Implementar filtro de quant y error claro si no existe, con test unitario.
- [ ] 2.4 Documentar `import` en `docs/modelforge.md` y verificar que el comando documentado funciona.

## 3. Creación en Ollama

- [ ] 3.1 Generar el Modelfile con `FROM`, `RENDERER`, `PARSER`, `num_ctx` y parámetros, con test unitario.
- [ ] 3.2 Crear el modelo vía `/api/create` y verificar con `ollama show`.
- [ ] 3.3 Manejar Ollama no disponible con error claro y código de salida distinto de 0, con test de integración.
- [ ] 3.4 Documentar `create` en `docs/modelforge.md` y verificar el comando documentado.

## 4. Test y benchmark

- [ ] 4.1 Implementar `test` con texto, thinking, tools y vision, y verificar con el modelo real.
- [ ] 4.2 Implementar `bench` con `eval_count`, `eval_duration`, `load_duration`, `size_vram`, `context` y `processor`, con test de integración.
- [ ] 4.3 Implementar `--compare` y verificar la tabla comparativa.
- [ ] 4.4 Documentar `test` y `bench` en `docs/modelforge.md`.

## 5. Listado, info y ruta

- [ ] 5.1 Implementar `list` e `info` con `/api/tags` y `/api/ps`, con test de integración.
- [ ] 5.2 Implementar `route` con `serde_yaml`, backup y sin duplicados, con test unitario.
- [ ] 5.3 Documentar `route` en `docs/modelforge.md` y verificar el backup.

## 6. Integración

- [ ] 6.1 Ejecutar el flujo completo con un modelo real (import → create → test → bench → route) y verificar el resultado.
- [ ] 6.2 Revisar `docs/modelforge.md` con ejemplos y verificar que los comandos funcionan.
- [ ] 6.3 Ejecutar `cargo test`, `cargo clippy --all-targets -- -D warnings` y `cargo fmt --all -- --check` y verificar que todo pasa.