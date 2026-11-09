# Tasks

## 1. Selección de backend

- [ ] 1.1 Flag `--backend <ollama|llama.cpp>` en `import` (clap), valor por defecto `ollama`, con test unitario de parseo.
- [ ] 1.2 Enrutado: safetensors + `--backend llama.cpp` → flujo de conversión; safetensors + `--backend ollama` → flujo nativo actual. Test unitario.

## 2. Detección de herramientas

- [ ] 2.1 Trait `Converter` + implementación real que localiza `convert_hf_to_gguf.py` y `llama-quantize` en `PATH`. Test unitario (con doble).
- [ ] 2.2 Error accionable `ConverterToolMissing` con instrucciones si falta alguna herramienta. Test.

## 3. Conversión y cuantización

- [ ] 3.1 Conversión safetensors → GGUF FP16 (directorio temporal) y test con converter falso.
- [ ] 3.2 Cuantización con `llama-quantize` y validación del quant destino contra la lista de llama.cpp. Test.
- [ ] 3.3 `ollama create` con `FROM <gguf>` del resultado. Test de integración.
- [ ] 3.4 Limpieza del GGUF intermedio y del directorio temporal. Test.

## 4. Integración con el CLI

- [ ] 4.1 `import <dir> --backend llama.cpp [--quantize <Q>] --name <m>` de extremo a extremo. Test de integración CLI.
- [ ] 4.2 `--json` con eventos por etapa (NDJSON en stderr, objeto único en stdout). Test.
- [ ] 4.3 Códigos de salida distintos de 0 en los errores. Test.
- [ ] 4.4 Documentar el puente en `docs/native-import.md` (o nuevo `docs/llamacpp-bridge.md`).

## 5. Verificación

- [ ] 5.1 `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --all -- --check`.
- [ ] 5.2 Flujo real con llama.cpp instalado: safetensors → `--backend llama.cpp --quantize Q4_K_M` → Ollama (`#[ignore]`, gateado por env).
