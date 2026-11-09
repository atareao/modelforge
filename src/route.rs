//! Lógica del subcomando `route`: añade una ruta al `gateway.yaml`.
//!
//! Comportamiento:
//!
//! * **Idempotente:** si ya existe una ruta con ese nombre, no se duplica; se avisa.
//! * **Con backup:** antes de reescribir un `gateway.yaml` existente se guarda una
//!   copia en `<fichero>.bak`.
//! * **Validado:** si el YAML existente no se puede parsear, se aborta sin escribir.
//!
//! Cada ruta consta de dos *backends*: el local de Ollama (el modelo indicado) y el
//! de respaldo remoto. Este esquema reproduce la estructura real de
//! `~/.config/opencode/gateway.yaml`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// URL base del backend local de Ollama.
pub const LOCAL_BASE_URL: &str = "http://localhost:11434/v1";
/// URL base del backend de respaldo (gateway remoto).
pub const BACKUP_BASE_URL: &str = "https://openrouter.ai/api/v1";
/// Modelo por defecto del backend de respaldo.
pub const BACKUP_MODEL: &str = "deepseek/deepseek-v4.1-flash";
/// Variable de entorno con la API key del backend de respaldo.
pub const BACKUP_API_KEY_ENV: &str = "OPENROUTER_API_KEY";
/// Timeout por defecto de un backend, en milisegundos.
pub const DEFAULT_TIMEOUT_MS: u64 = 120_000;

/// Un backend de una ruta del gateway.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Backend {
    /// URL base del endpoint compatible con OpenAI.
    pub base_url: String,
    /// Modelo servido por este backend.
    pub model: String,
    /// Variable de entorno que contiene la API key (solo backends remotos).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,
    /// Timeout de la petición, en milisegundos.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// Una ruta del gateway: un nombre y una lista ordenada de backends.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Route {
    /// Nombre lógico de la ruta.
    pub name: String,
    /// Backends, en orden de preferencia (el primero es el primario).
    pub backend: Vec<Backend>,
}

/// Documento `gateway.yaml`: una lista de rutas bajo la clave `route`.
///
/// El resto de claves top-level (p. ej. `port`, `model`, `log_level`) se
/// conservan en [`Gateway::extra`] y se vuelven a escribir intactas, de modo que
/// reescribir el documento nunca destruye configuración ajena.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Gateway {
    /// Rutas definidas en el gateway.
    #[serde(default)]
    pub route: Vec<Route>,
    /// Claves top-level desconocidas, preservadas tal cual.
    #[serde(default, flatten)]
    pub extra: serde_yaml::Mapping,
}

/// Resultado de aplicar `route` sobre el gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteStatus {
    /// La ruta era nueva y se ha añadido.
    Added,
    /// La ruta ya existía y no se ha duplicado.
    AlreadyExists,
}

/// Resultado del subcomando `route`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RouteOutcome {
    /// Estado de la operación.
    pub status: RouteStatus,
    /// Nombre de la ruta.
    pub name: String,
    /// Modelo local asociado a la ruta.
    pub model: String,
    /// Ruta del `gateway.yaml`.
    pub path: PathBuf,
    /// Copia de seguridad creada, si el fichero ya existía.
    pub backup: Option<PathBuf>,
    /// YAML resultante (tal y como se escribiría).
    #[serde(skip)]
    pub yaml: String,
}

/// Ruta por defecto del `gateway.yaml` (`$HOME/.config/opencode/gateway.yaml`).
///
/// # Errors
///
/// Devuelve [`AppError::NoHomeDirectory`] si `$HOME` no está definido.
pub fn default_path() -> Result<PathBuf, AppError> {
    let home = std::env::var_os("HOME").ok_or(AppError::NoHomeDirectory)?;
    Ok(PathBuf::from(home).join(".config/opencode/gateway.yaml"))
}

/// Construye la ruta a añadir: backend local de Ollama + backend de respaldo.
#[must_use]
pub fn build_route(name: &str, model: &str) -> Route {
    Route {
        name: name.to_string(),
        backend: vec![
            Backend {
                base_url: LOCAL_BASE_URL.to_string(),
                model: model.to_string(),
                api_key_env: None,
                timeout_ms: Some(DEFAULT_TIMEOUT_MS),
            },
            Backend {
                base_url: BACKUP_BASE_URL.to_string(),
                model: BACKUP_MODEL.to_string(),
                api_key_env: Some(BACKUP_API_KEY_ENV.to_string()),
                timeout_ms: Some(DEFAULT_TIMEOUT_MS),
            },
        ],
    }
}

/// Ruta del fichero de backup: el mismo nombre con el sufijo `.bak`.
fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".bak");
    PathBuf::from(name)
}

/// Aplica la ruta sobre un `gateway.yaml` (en texto) sin duplicar.
///
/// # Errors
///
/// Devuelve [`AppError::InvalidGateway`] si `existing` no es un YAML válido.
pub fn apply(
    existing: Option<&str>,
    name: &str,
    model: &str,
    path: &Path,
) -> Result<(Gateway, RouteStatus), AppError> {
    if name.trim().is_empty() {
        return Err(AppError::InvalidRouteName {
            name: name.to_string(),
        });
    }

    let mut gateway = match existing {
        Some(text) if !text.trim().is_empty() => {
            serde_yaml::from_str::<Gateway>(text).map_err(|err| AppError::InvalidGateway {
                path: path.display().to_string(),
                message: err.to_string(),
            })?
        }
        _ => Gateway::default(),
    };

    if gateway.route.iter().any(|route| route.name == name) {
        return Ok((gateway, RouteStatus::AlreadyExists));
    }

    gateway.route.push(build_route(name, model));
    Ok((gateway, RouteStatus::Added))
}

/// Añade la ruta al fichero `path`, con backup y validación.
///
/// Si la ruta ya existe o `dry_run` está activo, no se modifica el fichero ni se
/// crea backup.
///
/// # Errors
///
/// Devuelve [`AppError::InvalidGateway`] si el YAML existente es inválido y
/// [`AppError::Io`] si falla la lectura, el backup o la escritura.
pub fn run(path: &Path, name: &str, model: &str, dry_run: bool) -> Result<RouteOutcome, AppError> {
    let existing = match std::fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(err.into()),
    };

    let (gateway, status) = apply(existing.as_deref(), name, model, path)?;
    let yaml = serde_yaml::to_string(&gateway).map_err(|err| AppError::InvalidGateway {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;

    let outcome = RouteOutcome {
        status,
        name: name.to_string(),
        model: model.to_string(),
        path: path.to_path_buf(),
        backup: None,
        yaml,
    };

    if status == RouteStatus::AlreadyExists || dry_run {
        return Ok(outcome);
    }

    let backup = if existing.is_some() {
        let backup = backup_path(path);
        std::fs::copy(path, &backup)?;
        Some(backup)
    } else {
        None
    };

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)?;
    }
    write_atomic(path, outcome.yaml.as_bytes())?;

    Ok(RouteOutcome { backup, ..outcome })
}

/// Escribe `contents` en `path` de forma atómica.
///
/// Vuelca a un fichero temporal `<path>.tmp` en el MISMO directorio, fuerza su
/// sincronización a disco y lo renombra sobre el original con `fs::rename`
/// (atómico dentro del mismo sistema de ficheros). Así, un fallo a mitad de
/// escritura nunca deja el `gateway.yaml` truncado o corrupto.
///
/// # Errors
///
/// Devuelve [`AppError::Io`] si falla la escritura, el `sync_all` o el rename.
fn write_atomic(path: &Path, contents: &[u8]) -> Result<(), AppError> {
    use std::io::Write as _;

    let tmp_path = tmp_path_for(path);
    {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(contents)?;
        file.sync_all()?;
    }
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Ruta del temporal de escritura atómica: el mismo nombre con el sufijo `.tmp`.
fn tmp_path_for(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".tmp");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_yaml() -> &'static str {
        "route:\n  - name: local-fast\n    backend:\n      - base_url: \"http://localhost:11434/v1\"\n        model: \"llama3.2:3b\"\n        timeout_ms: 30000\n      - base_url: \"https://openrouter.ai/api/v1\"\n        model: \"deepseek/deepseek-v4.1-flash\"\n        api_key_env: \"OPENROUTER_API_KEY\"\n        timeout_ms: 120000\n"
    }

    #[test]
    fn build_route_has_local_backend_and_backup() {
        let route = build_route("mi-ruta", "qwen3:8b");

        assert_eq!(route.name, "mi-ruta");
        assert_eq!(route.backend.len(), 2);
        assert_eq!(route.backend[0].base_url, LOCAL_BASE_URL);
        assert_eq!(route.backend[0].model, "qwen3:8b");
        assert_eq!(route.backend[0].api_key_env, None);
        assert_eq!(route.backend[1].base_url, BACKUP_BASE_URL);
        assert_eq!(route.backend[1].model, BACKUP_MODEL);
        assert_eq!(
            route.backend[1].api_key_env.as_deref(),
            Some(BACKUP_API_KEY_ENV)
        );
    }

    #[test]
    fn apply_adds_route_to_empty_gateway() {
        let path = Path::new("gateway.yaml");
        let (gateway, status) = apply(None, "nueva", "modelo:x", path).expect("apply");

        assert_eq!(status, RouteStatus::Added);
        assert_eq!(gateway.route.len(), 1);
        assert_eq!(gateway.route[0].name, "nueva");
        assert_eq!(gateway.route[0].backend[0].model, "modelo:x");
    }

    #[test]
    fn apply_treats_blank_yaml_as_empty_gateway() {
        let path = Path::new("gateway.yaml");
        let (gateway, status) = apply(Some("\n  \n"), "nueva", "modelo:x", path).expect("apply");

        assert_eq!(status, RouteStatus::Added);
        assert_eq!(gateway.route.len(), 1);
    }

    #[test]
    fn apply_preserves_existing_routes() {
        let path = Path::new("gateway.yaml");
        let (gateway, status) =
            apply(Some(sample_yaml()), "segunda", "qwen3:8b", path).expect("apply");

        assert_eq!(status, RouteStatus::Added);
        assert_eq!(gateway.route.len(), 2);
        assert_eq!(gateway.route[0].name, "local-fast");
        assert_eq!(gateway.route[1].name, "segunda");
    }

    #[test]
    fn apply_does_not_duplicate_existing_route() {
        let path = Path::new("gateway.yaml");
        let (gateway, status) =
            apply(Some(sample_yaml()), "local-fast", "otro:modelo", path).expect("apply");

        assert_eq!(status, RouteStatus::AlreadyExists);
        assert_eq!(gateway.route.len(), 1);
        assert_eq!(gateway.route[0].backend[0].model, "llama3.2:3b");
    }

    #[test]
    fn apply_rejects_invalid_yaml() {
        let path = Path::new("gateway.yaml");
        let err =
            apply(Some("route: [no válido"), "nueva", "modelo:x", path).expect_err("debe fallar");

        assert!(matches!(err, AppError::InvalidGateway { .. }), "{err}");
    }

    #[test]
    fn apply_rejects_yaml_with_wrong_shape() {
        let path = Path::new("gateway.yaml");
        let err = apply(Some("route: 42\n"), "nueva", "modelo:x", path).expect_err("debe fallar");

        assert!(matches!(err, AppError::InvalidGateway { .. }), "{err}");
    }

    #[test]
    fn serialized_gateway_round_trips() {
        let path = Path::new("gateway.yaml");
        let (gateway, _) = apply(Some(sample_yaml()), "segunda", "qwen3:8b", path).expect("apply");
        let yaml = serde_yaml::to_string(&gateway).expect("serializar");
        let reparsed: Gateway = serde_yaml::from_str(&yaml).expect("reparsear");

        assert_eq!(gateway, reparsed);
    }

    #[test]
    fn run_creates_file_without_backup_when_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("gateway.yaml");

        let outcome = run(&path, "nueva", "modelo:x", false).expect("run");

        assert_eq!(outcome.status, RouteStatus::Added);
        assert!(outcome.backup.is_none(), "sin fichero previo no hay backup");
        let written = std::fs::read_to_string(&path).expect("leer");
        let gateway: Gateway = serde_yaml::from_str(&written).expect("yaml");
        assert_eq!(gateway.route.len(), 1);
        assert_eq!(gateway.route[0].name, "nueva");
    }

    #[test]
    fn run_merges_into_existing_file_and_creates_backup() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("gateway.yaml");
        std::fs::write(&path, sample_yaml()).expect("escribir previo");

        let outcome = run(&path, "segunda", "qwen3:8b", false).expect("run");

        assert_eq!(outcome.status, RouteStatus::Added);
        let backup = outcome.backup.expect("debe crear backup");
        assert_eq!(backup, dir.path().join("gateway.yaml.bak"));
        assert_eq!(
            std::fs::read_to_string(&backup).expect("leer backup"),
            sample_yaml()
        );
        let gateway: Gateway =
            serde_yaml::from_str(&std::fs::read_to_string(&path).expect("leer")).expect("yaml");
        assert_eq!(gateway.route.len(), 2);
    }

    #[test]
    fn run_is_idempotent_and_does_not_backup_on_duplicate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("gateway.yaml");
        std::fs::write(&path, sample_yaml()).expect("escribir previo");

        let outcome = run(&path, "local-fast", "otro:modelo", false).expect("run");

        assert_eq!(outcome.status, RouteStatus::AlreadyExists);
        assert!(outcome.backup.is_none(), "sin cambios no hay backup");
        assert!(
            !dir.path().join("gateway.yaml.bak").exists(),
            "no debe crear backup"
        );
        assert_eq!(
            std::fs::read_to_string(&path).expect("leer"),
            sample_yaml(),
            "el fichero no debe cambiar"
        );
    }

    #[test]
    fn run_aborts_on_invalid_yaml_without_writing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("gateway.yaml");
        std::fs::write(&path, "route: [no válido").expect("escribir previo");

        let err = run(&path, "nueva", "modelo:x", false).expect_err("debe fallar");

        assert!(matches!(err, AppError::InvalidGateway { .. }), "{err}");
        assert!(!dir.path().join("gateway.yaml.bak").exists());
        assert_eq!(
            std::fs::read_to_string(&path).expect("leer"),
            "route: [no válido"
        );
    }

    #[test]
    fn run_with_dry_run_does_not_write() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("gateway.yaml");

        let outcome = run(&path, "nueva", "modelo:x", true).expect("run");

        assert_eq!(outcome.status, RouteStatus::Added);
        assert!(outcome.backup.is_none());
        assert!(!path.exists(), "dry-run no debe crear el fichero");
        assert!(
            outcome.yaml.contains("nueva"),
            "el preview debe incluir la ruta"
        );
    }

    #[test]
    fn default_path_is_under_home_config() {
        // Solo comprueba la forma; no depende de que exista.
        if let Ok(path) = default_path() {
            assert!(path.ends_with(".config/opencode/gateway.yaml"), "{path:?}");
        }
    }

    #[test]
    fn apply_rejects_blank_name() {
        let path = Path::new("gateway.yaml");
        for blank in ["", "   ", "\t\n"] {
            let err = apply(None, blank, "modelo:x", path).expect_err("debe fallar");
            assert!(matches!(err, AppError::InvalidRouteName { .. }), "{err}");
        }
    }

    #[test]
    fn apply_preserves_unknown_top_level_keys() {
        let path = Path::new("gateway.yaml");
        let existing = "port: 8080\nmodel: default\nlog_level: info\nroute:\n  - name: local-fast\n    backend:\n      - base_url: \"http://localhost:11434/v1\"\n        model: \"llama3.2:3b\"\n";
        let (gateway, status) = apply(Some(existing), "segunda", "qwen3:8b", path).expect("apply");

        assert_eq!(status, RouteStatus::Added);
        assert_eq!(gateway.route.len(), 2, "se añade la ruta nueva");
        assert_eq!(
            gateway.extra.get("port"),
            Some(&serde_yaml::Value::from(8080))
        );
        assert_eq!(
            gateway.extra.get("log_level"),
            Some(&serde_yaml::Value::from("info"))
        );

        let yaml = serde_yaml::to_string(&gateway).expect("serializar");
        assert!(yaml.contains("port: 8080"), "yaml: {yaml}");
        assert!(yaml.contains("model: default"), "yaml: {yaml}");
        assert!(yaml.contains("log_level: info"), "yaml: {yaml}");
        assert!(yaml.contains("segunda"), "yaml: {yaml}");
    }

    #[test]
    fn run_preserves_unknown_keys_in_existing_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("gateway.yaml");
        std::fs::write(&path, "port: 9000\nroute: []\n").expect("escribir previo");

        run(&path, "nueva", "modelo:x", false).expect("run");

        let written = std::fs::read_to_string(&path).expect("leer");
        assert!(written.contains("port: 9000"), "yaml: {written}");
        assert!(written.contains("nueva"), "yaml: {written}");
    }

    #[test]
    fn run_writes_atomically_and_leaves_no_tmp_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("gateway.yaml");

        let outcome = run(&path, "nueva", "modelo:x", false).expect("run");

        assert_eq!(
            std::fs::read_to_string(&path).expect("leer"),
            outcome.yaml,
            "el fichero final debe ser exactamente el YAML esperado"
        );
        assert!(
            !dir.path().join("gateway.yaml.tmp").exists(),
            "no debe quedar ningún temporal `.tmp`"
        );
    }
}
