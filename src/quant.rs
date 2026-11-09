//! Selección del fichero GGUF (y del `mmproj`) para un quant dado.
//!
//! Lógica pura y sin red: opera sobre la lista de ficheros remotos que devuelve
//! el Hub, por lo que se prueba con listas de nombres tipo HuggingFace.

use crate::error::AppError;
use crate::hf::RemoteFile;

/// Ficheros elegidos para descargar a partir de la lista del repositorio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Selection<'a> {
    /// Fichero GGUF del quant solicitado.
    pub gguf: &'a RemoteFile,
    /// Proyector de visión (`mmproj`), si se pidió.
    pub mmproj: Option<&'a RemoteFile>,
}

/// `true` si la ruta corresponde a un proyector de visión (`mmproj`).
#[must_use]
pub fn is_mmproj(path: &str) -> bool {
    path.to_ascii_lowercase().contains("mmproj")
}

/// `true` si la ruta es un fichero GGUF.
#[must_use]
pub fn is_gguf(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".gguf")
}

/// Selecciona el GGUF del quant pedido y, opcionalmente, su `mmproj`.
///
/// # Errors
///
/// - [`AppError::NoGgufFound`] si el repo no tiene ficheros GGUF.
/// - [`AppError::QuantNotFound`] si ningún GGUF coincide con `quant`.
/// - [`AppError::MmprojNotFound`] si se pide `--mmproj` y no hay ninguno.
pub fn select<'a>(
    repo: &str,
    files: &'a [RemoteFile],
    quant: &str,
    want_mmproj: bool,
) -> Result<Selection<'a>, AppError> {
    let ggufs: Vec<&RemoteFile> = files
        .iter()
        .filter(|file| is_gguf(&file.path) && !is_mmproj(&file.path))
        .collect();

    if ggufs.is_empty() {
        return Err(AppError::NoGgufFound {
            repo: repo.to_string(),
        });
    }

    let gguf = ggufs
        .iter()
        .copied()
        .find(|file| has_quant(&file.path, quant))
        .ok_or_else(|| AppError::QuantNotFound {
            repo: repo.to_string(),
            quant: quant.to_string(),
            available: available_quants(files),
        })?;

    let mmproj = if want_mmproj {
        let candidates: Vec<&RemoteFile> = files
            .iter()
            .filter(|file| is_gguf(&file.path) && is_mmproj(&file.path))
            .collect();

        if candidates.is_empty() {
            return Err(AppError::MmprojNotFound {
                repo: repo.to_string(),
            });
        }

        let chosen = candidates
            .iter()
            .copied()
            .find(|file| has_quant(&file.path, quant))
            .unwrap_or_else(|| {
                let mut sorted = candidates.clone();
                sorted.sort_by(|a, b| a.path.cmp(&b.path));
                sorted[0]
            });
        Some(chosen)
    } else {
        None
    };

    Ok(Selection { gguf, mmproj })
}

/// Quants distintos presentes en el repo (sin duplicados y ordenados).
#[must_use]
pub fn available_quants(files: &[RemoteFile]) -> Vec<String> {
    let mut quants: Vec<String> = Vec::new();
    for file in files {
        if !is_gguf(&file.path) || is_mmproj(&file.path) {
            continue;
        }
        for token in tokens(&file.path) {
            if looks_like_quant(token) && !quants.iter().any(|q| q.eq_ignore_ascii_case(token)) {
                quants.push(token.to_string());
            }
        }
    }
    quants.sort();
    quants
}

/// `true` si alguno de los tokens del nombre coincide (sin distinguir mayúsculas).
fn has_quant(path: &str, quant: &str) -> bool {
    tokens(path).any(|token| token.eq_ignore_ascii_case(quant))
}

/// Tokens del nombre de fichero, separados por `-` y `.`.
///
/// Se respetan los `_` porque forman parte de los quants de llama.cpp
/// (`Q4_K_M`, `q4_0`).
fn tokens(path: &str) -> impl Iterator<Item = &str> {
    let stem = path
        .strip_suffix(".gguf")
        .or_else(|| path.strip_suffix(".GGUF"))
        .unwrap_or(path);
    stem.rsplit(['-', '.']).filter(|token| !token.is_empty())
}

/// `true` si el token tiene pinta de quant de llama.cpp (`Q4_K_M`, `q8_0`,
/// `IQ4_XS`, `F16`, `BF16`, `F32`…).
fn looks_like_quant(token: &str) -> bool {
    let bytes = token.as_bytes();
    if bytes.len() < 2 {
        return false;
    }
    let first = bytes[0].to_ascii_lowercase();
    if (first == b'q' || first == b'i') && bytes[1].is_ascii_digit() {
        return true;
    }
    matches!(token.to_ascii_uppercase().as_str(), "F16" | "F32" | "BF16")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> RemoteFile {
        RemoteFile {
            path: path.to_string(),
            size: 1_000_000,
        }
    }

    fn files(paths: &[&str]) -> Vec<RemoteFile> {
        paths.iter().map(|p| file(p)).collect()
    }

    #[test]
    fn is_mmproj_detects_projector() {
        assert!(is_mmproj("mmproj-model-f16.gguf"));
        assert!(is_mmproj("model-mmproj-f16.gguf"));
        assert!(!is_mmproj("model-Q6_K.gguf"));
    }

    #[test]
    fn selects_the_requested_quant_among_many() {
        let list = files(&[
            "model-Q4_K_M.gguf",
            "model-Q5_K_M.gguf",
            "model-Q6_K.gguf",
            "README.md",
        ]);
        let sel = select("org/model", &list, "Q6_K", false).expect("debe seleccionar Q6_K");
        assert_eq!(sel.gguf.path, "model-Q6_K.gguf");
        assert!(sel.mmproj.is_none());
    }

    #[test]
    fn quant_match_is_case_insensitive_and_dot_separated() {
        let list = files(&["stories15M-q4_0.gguf", "stories15M.Q8_0.gguf"]);
        let sel = select("org/model", &list, "q4_0", false).expect("debe seleccionar q4_0");
        assert_eq!(sel.gguf.path, "stories15M-q4_0.gguf");
    }

    #[test]
    fn selects_mmproj_when_requested() {
        let list = files(&[
            "model-Q6_K.gguf",
            "mmproj-model-f16.gguf",
            "model-Q4_K_M.gguf",
        ]);
        let sel = select("org/model", &list, "Q6_K", true).expect("debe seleccionar con mmproj");
        assert_eq!(sel.gguf.path, "model-Q6_K.gguf");
        assert_eq!(
            sel.mmproj.map(|m| m.path.as_str()),
            Some("mmproj-model-f16.gguf")
        );
    }

    #[test]
    fn prefers_mmproj_with_matching_quant() {
        let list = files(&["model-Q6_K.gguf", "mmproj-Q4_K_M.gguf", "mmproj-Q6_K.gguf"]);
        let sel = select("org/model", &list, "Q6_K", true).expect("debe seleccionar mmproj");
        assert_eq!(
            sel.mmproj.map(|m| m.path.as_str()),
            Some("mmproj-Q6_K.gguf")
        );
    }

    #[test]
    fn errors_when_mmproj_requested_but_absent() {
        let list = files(&["model-Q6_K.gguf", "model-Q4_K_M.gguf"]);
        let err = select("org/model", &list, "Q6_K", true).expect_err("debe fallar sin mmproj");
        assert!(matches!(err, AppError::MmprojNotFound { .. }), "{err}");
    }

    #[test]
    fn errors_with_available_quants_when_missing() {
        let list = files(&["model-Q4_K_M.gguf", "model-Q5_K_M.gguf"]);
        let err = select("org/model", &list, "Q6_K", false).expect_err("no existe Q6_K");
        match err {
            AppError::QuantNotFound {
                quant, available, ..
            } => {
                assert_eq!(quant, "Q6_K");
                assert_eq!(available, vec!["Q4_K_M".to_string(), "Q5_K_M".to_string()]);
            }
            other => panic!("se esperaba QuantNotFound, se obtuvo {other:?}"),
        }
    }

    #[test]
    fn errors_when_repo_has_no_gguf() {
        let list = files(&["README.md", "config.json", "tokenizer.json"]);
        let err = select("org/model", &list, "Q6_K", false).expect_err("no hay GGUF");
        assert!(matches!(err, AppError::NoGgufFound { .. }), "{err}");
    }

    #[test]
    fn available_quants_are_deduplicated_and_sorted() {
        let list = files(&[
            "a-Q6_K.gguf",
            "b-q6_k.gguf",
            "c-Q4_K_M.gguf",
            "mmproj-f16.gguf",
        ]);
        let quants = available_quants(&list);
        assert_eq!(quants, vec!["Q4_K_M".to_string(), "Q6_K".to_string()]);
    }
}
