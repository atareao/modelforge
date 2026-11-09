//! Puente a llama.cpp: convierte un directorio de safetensors a GGUF.
//!
//! En Linux, Ollama no puede crear un modelo a partir de safetensors (enruta el
//! snapshot a su motor MLX, sin backend en Linux). La vía soportada es convertir a
//! GGUF con `llama.cpp` (`convert_hf_to_gguf.py` + `llama-quantize`) y crear el
//! modelo en Ollama a partir del GGUF resultante.
//!
//! El flujo vive detrás del trait [`Converter`], de modo que la orquestación
//! ([`run`]) se prueba con un **doble** sin depender de que `llama.cpp` esté
//! instalado. La implementación real, [`LlamaCppConverter`], invoca las
//! herramientas con [`tokio::process::Command`] para no bloquear el reactor de
//! Tokio.
//!
//! Flujo de [`run`]:
//!
//! 1. valida el `--quantize` contra la lista de quants de llama.cpp ([`crate::quant`]);
//! 2. localiza `python3`, `convert_hf_to_gguf.py` y `llama-quantize` en `PATH`
//!    (error accionable si falta alguno);
//! 3. crea un directorio temporal;
//! 4. convierte a `model-f16.gguf` (FP16);
//! 5. si se pidió, cuantiza a `model-<QUANT>.gguf`;
//! 6. crea el modelo en Ollama a partir del GGUF resultante (reutiliza
//!    [`crate::create::run`]); el `Modelfile` del reporte se **omite**, porque
//!    apuntaría a un GGUF temporal ya eliminado;
//! 7. elimina el directorio temporal **siempre** (éxito y error).

use std::ffi::OsString;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Output;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::create::{self, CreateOptions, Progress};
use crate::error::AppError;
use crate::ollama::OllamaClient;
use crate::quant;

/// Rutas de las herramientas externas de llama.cpp localizadas en `PATH`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tools {
    /// Ruta al intérprete de Python 3 (`python3`).
    pub python: PathBuf,
    /// Ruta a `convert_hf_to_gguf.py` (script de conversión de llama.cpp).
    pub convert_script: PathBuf,
    /// Ruta al binario `llama-quantize`.
    pub quantize_bin: PathBuf,
}

/// Abstracción sobre las herramientas de llama.cpp, para poder inyectar un doble.
///
/// [`find_tools`](Converter::find_tools) es síncrono (solo inspecciona `PATH`),
/// mientras que [`convert`](Converter::convert) y [`quantize`](Converter::quantize)
/// devuelven un `Future`: son procesos externos que no deben bloquear el reactor de
/// Tokio. Las implementaciones son `Send`, de modo que el doble de prueba sigue
/// siendo inyectable sin cambios en la orquestación.
pub trait Converter {
    /// Localiza `python3`, `convert_hf_to_gguf.py` y `llama-quantize` en `PATH`.
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::ConverterToolMissing`] si falta alguna.
    fn find_tools(&self) -> Result<Tools, AppError>;

    /// Convierte un directorio de safetensors a un GGUF FP16 en `outfile`.
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::ConverterFailed`] si el proceso falla.
    fn convert(
        &self,
        tools: &Tools,
        source: &Path,
        outfile: &Path,
    ) -> impl Future<Output = Result<(), AppError>> + Send;

    /// Cuantiza el GGUF `input` a `quant`, escribiendo el resultado en `outfile`.
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::ConverterFailed`] si el proceso falla.
    fn quantize(
        &self,
        tools: &Tools,
        input: &Path,
        outfile: &Path,
        quant: &str,
    ) -> impl Future<Output = Result<(), AppError>> + Send;
}

/// Opciones del flujo de conversión con llama.cpp.
#[derive(Debug, Clone)]
pub struct ConvertOptions<'a> {
    /// Nombre del modelo a crear en Ollama.
    pub name: &'a str,
    /// Directorio local con `.safetensors`.
    pub source_dir: &'a Path,
    /// Quant de llama.cpp pedido con `--quantize`, si lo hay.
    pub quantize: Option<&'a str>,
    /// Raíz donde crear el directorio temporal (por defecto, el del sistema).
    pub tmp_root: Option<&'a Path>,
}

/// Implementación real de [`Converter`] que invoca las herramientas de llama.cpp.
#[derive(Debug, Default)]
pub struct LlamaCppConverter {
    /// `PATH` a inspeccionar; `None` usa el de la variable de entorno.
    path: Option<OsString>,
}

impl LlamaCppConverter {
    /// Crea un conversor que resuelve las herramientas con el `PATH` del entorno.
    #[must_use]
    pub fn new() -> Self {
        Self { path: None }
    }

    /// `PATH` efectivo a inspeccionar.
    fn effective_path(&self) -> OsString {
        self.path
            .clone()
            .or_else(|| std::env::var_os("PATH"))
            .unwrap_or_default()
    }
}

impl Converter for LlamaCppConverter {
    fn find_tools(&self) -> Result<Tools, AppError> {
        let path = self.effective_path();
        let python = find_in_path(PYTHON, &path).ok_or_else(|| missing_tool(PYTHON))?;
        let convert_script = find_script_in_path(CONVERT_SCRIPT, &path)
            .ok_or_else(|| missing_tool(CONVERT_SCRIPT))?;
        let quantize_bin =
            find_in_path(QUANTIZE_BIN, &path).ok_or_else(|| missing_tool(QUANTIZE_BIN))?;
        Ok(Tools {
            python,
            convert_script,
            quantize_bin,
        })
    }

    fn convert(
        &self,
        tools: &Tools,
        source: &Path,
        outfile: &Path,
    ) -> impl Future<Output = Result<(), AppError>> + Send {
        // Se clonan las rutas para moverlas al futuro: `output()` es asíncrono y no
        // debe tomar prestados los argumentos.
        let python = tools.python.clone();
        let script = tools.convert_script.clone();
        let source = source.to_path_buf();
        let outfile = outfile.to_path_buf();
        async move {
            // Invocación documentada de llama.cpp:
            //   python3 <convert_hf_to_gguf.py> <dir-safetensors> --outfile <out> --outtype f16
            let output = tokio::process::Command::new(&python)
                .arg(&script)
                .arg(&source)
                .arg("--outfile")
                .arg(&outfile)
                .arg("--outtype")
                .arg("f16")
                .output()
                .await
                .map_err(|err| AppError::ConverterFailed {
                    tool: CONVERT_SCRIPT.to_string(),
                    message: format!("no se pudo ejecutar `{}`: {err}", python.display()),
                })?;
            check_status(output, CONVERT_SCRIPT)
        }
    }

    fn quantize(
        &self,
        tools: &Tools,
        input: &Path,
        outfile: &Path,
        quant: &str,
    ) -> impl Future<Output = Result<(), AppError>> + Send {
        let bin = tools.quantize_bin.clone();
        let input = input.to_path_buf();
        let outfile = outfile.to_path_buf();
        let quant = quant.to_string();
        async move {
            // Invocación documentada de llama.cpp:
            //   llama-quantize <in.gguf> <out.gguf> <TYPE>
            let output = tokio::process::Command::new(&bin)
                .arg(&input)
                .arg(&outfile)
                .arg(&quant)
                .output()
                .await
                .map_err(|err| AppError::ConverterFailed {
                    tool: QUANTIZE_BIN.to_string(),
                    message: format!("no se pudo ejecutar `{}`: {err}", bin.display()),
                })?;
            check_status(output, QUANTIZE_BIN)
        }
    }
}

/// Construye el error de herramienta ausente para `tool`.
fn missing_tool(tool: &str) -> AppError {
    AppError::ConverterToolMissing {
        tool: tool.to_string(),
    }
}

/// Busca un **ejecutable** llamado `name` en los directorios de `path`.
///
/// Un fichero sin permiso de ejecución se trata como ausente: si existe pero no se
/// puede ejecutar, `PATH` no ofrece la herramienta.
fn find_in_path(name: &str, path: &std::ffi::OsStr) -> Option<PathBuf> {
    find_candidate(name, path, is_executable)
}

/// Busca un **fichero** llamado `name` en los directorios de `path`.
///
/// Se usa para `convert_hf_to_gguf.py`: es un script que se ejecuta como
/// `python3 <script>`, así que no necesita bit de ejecución (en la distribución de
/// llama.cpp suele faltarle).
fn find_script_in_path(name: &str, path: &std::ffi::OsStr) -> Option<PathBuf> {
    find_candidate(name, path, Path::is_file)
}

/// Primer candidato `dir/name` de `path` que cumple `accept`.
fn find_candidate(
    name: &str,
    path: &std::ffi::OsStr,
    accept: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    std::env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(name))
        .find(|candidate| accept(candidate))
}

/// `true` si `path` es un fichero regular con algún bit de ejecución (Unix).
#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

/// `true` si `path` es un fichero regular (fuera de Unix no se comprueba el bit).
#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Convierte el resultado de un proceso externo en `Ok`/`AppError`.
///
/// Incluye **ambas** salidas en el mensaje: se prefiere `stderr` y, si hay `stdout`
/// adicional, se añade a continuación (los procesos de llama.cpp reparten el
/// detalle del error entre las dos).
fn check_status(output: Output, tool: &str) -> Result<(), AppError> {
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = stderr.trim();
    let stdout = stdout.trim();
    let mut parts: Vec<String> = Vec::new();
    if !stderr.is_empty() {
        parts.push(stderr.to_string());
    }
    if !stdout.is_empty() {
        parts.push(format!("stdout: {stdout}"));
    }
    let detail = if parts.is_empty() {
        "(sin salida)".to_string()
    } else {
        parts.join(" | ")
    };
    let code = output
        .status
        .code()
        .map_or_else(|| "señal".to_string(), |code| code.to_string());
    Err(AppError::ConverterFailed {
        tool: tool.to_string(),
        message: format!("código {code}: {detail}"),
    })
}

/// Ejecuta el flujo completo de llama.cpp y crea el modelo en Ollama.
///
/// # Errors
///
/// - [`AppError::UnsupportedQuantize`] si `--quantize` no es un quant de llama.cpp.
/// - [`AppError::ConverterToolMissing`] si falta `python3`, `convert_hf_to_gguf.py`
///   o `llama-quantize`.
/// - [`AppError::ConverterFailed`] si la conversión o la cuantización fallan.
/// - Los errores de [`crate::create::run`] al crear el modelo en Ollama.
pub async fn run<C, P, T>(
    client: &C,
    converter: &T,
    opts: &ConvertOptions<'_>,
    progress: &P,
) -> Result<create::CreateReport, AppError>
where
    C: OllamaClient + Sync,
    P: Progress,
    T: Converter,
{
    // 1. Se valida el quant ANTES de invocar nada: es un error de uso.
    let quantize = match opts.quantize {
        Some(value) => Some(quant::canonical_llamacpp_quant(value).ok_or_else(|| {
            AppError::UnsupportedQuantize {
                value: value.to_string(),
                supported: quant::supported_llamacpp_quants_values(),
            }
        })?),
        None => None,
    };

    // 2. Se localizan las herramientas; si faltan, error accionable sin auto-install.
    let tools = converter.find_tools()?;

    // 3. Directorio temporal (se limpia al salir de la función, en éxito y en error).
    let root = opts
        .tmp_root
        .map_or_else(std::env::temp_dir, Path::to_path_buf);
    let tmp = TempDir::create(&root)?;

    // 4. Conversión a GGUF FP16.
    progress.stage(create::Stage::ConvertingSafetensors);
    tracing::info!(
        source = %opts.source_dir.display(),
        "convirtiendo safetensors a GGUF con llama.cpp"
    );
    let f16 = tmp.path().join("model-f16.gguf");
    converter.convert(&tools, opts.source_dir, &f16).await?;

    // 5. Cuantización opcional.
    let gguf = match &quantize {
        Some(quant) => {
            progress.stage_with_value(create::Stage::Quantizing, quant);
            tracing::info!(quant = %quant, "cuantizando GGUF con llama-quantize");
            let out = tmp.path().join(format!("model-{quant}.gguf"));
            converter.quantize(&tools, &f16, &out, quant).await?;
            out
        }
        None => f16,
    };

    // 6. Creación en Ollama a partir del GGUF resultante. Se usa `&()`, porque las
    //    etapas del flujo llama.cpp ya las emite este módulo.
    progress.stage(create::Stage::Creating);
    let create_opts = CreateOptions {
        name: opts.name,
        from: &gguf,
        mmproj: None,
        ctx: None,
        renderer: None,
        parser: None,
        quantize: None,
    };
    let mut report = create::run(client, &create_opts, &()).await?;

    // 7. El GGUF vive en un directorio temporal que se elimina al salir de esta
    //    función (RAII), así que un Modelfile con `FROM <tmp>` apuntaría a una ruta
    //    inexistente. Para no exponer una ruta muerta, el reporte omite el Modelfile
    //    en este flujo (a diferencia del import nativo, donde el origen persiste).
    report.modelfile = String::new();
    Ok(report)
}

/// Directorio temporal con limpieza automática recursiva al destruirse.
struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Crea un directorio temporal único bajo `root`.
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::Io`] si no se puede crear ningún directorio único.
    fn create(root: &Path) -> Result<Self, AppError> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        for _ in 0..1000 {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = root.join(format!("modelforge-llamacpp-{pid}-{n}"));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => return Err(AppError::Io(err)),
            }
        }
        Err(AppError::Io(std::io::Error::other(
            "no se pudo crear un directorio temporal único",
        )))
    }

    /// Ruta del directorio temporal.
    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Nombre del intérprete de Python 3 usado para ejecutar el script de conversión.
pub const PYTHON: &str = "python3";

/// Nombre del script de conversión de llama.cpp.
pub const CONVERT_SCRIPT: &str = "convert_hf_to_gguf.py";

/// Nombre del binario de cuantización de llama.cpp.
pub const QUANTIZE_BIN: &str = "llama-quantize";

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::create::Stage;
    use crate::ollama::{
        ChatRequest, ChatResponse, CreateModelRequest, GenerateRequest, GenerateResponse,
        ModelInfo, PullRequest, RunningModel, Sha256Digest, TaggedModel,
    };

    /// Conversor falso: registra las llamadas y escribe ficheros ficticios.
    #[derive(Default)]
    struct FakeConverter {
        missing_tool: Option<String>,
        fail_convert: bool,
        fail_quantize: bool,
        calls: Mutex<Vec<String>>,
    }

    impl FakeConverter {
        fn calls(&self) -> Vec<String> {
            self.calls.lock().map(|g| g.clone()).unwrap_or_default()
        }
    }

    impl Converter for FakeConverter {
        fn find_tools(&self) -> Result<Tools, AppError> {
            if let Ok(mut guard) = self.calls.lock() {
                guard.push("find_tools".to_string());
            }
            if let Some(tool) = &self.missing_tool {
                return Err(AppError::ConverterToolMissing { tool: tool.clone() });
            }
            Ok(Tools {
                python: PathBuf::from("/fake/python3"),
                convert_script: PathBuf::from("/fake/convert_hf_to_gguf.py"),
                quantize_bin: PathBuf::from("/fake/llama-quantize"),
            })
        }

        fn convert(
            &self,
            _tools: &Tools,
            source: &Path,
            outfile: &Path,
        ) -> impl Future<Output = Result<(), AppError>> + Send {
            if let Ok(mut guard) = self.calls.lock() {
                guard.push(format!("convert {}", source.display()));
            }
            std::fs::write(outfile, b"GGUF-F16").expect("escribir f16 ficticio");
            let result = if self.fail_convert {
                Err(AppError::ConverterFailed {
                    tool: CONVERT_SCRIPT.to_string(),
                    message: "boom".to_string(),
                })
            } else {
                Ok(())
            };
            async move { result }
        }

        fn quantize(
            &self,
            _tools: &Tools,
            _input: &Path,
            outfile: &Path,
            quant: &str,
        ) -> impl Future<Output = Result<(), AppError>> + Send {
            if let Ok(mut guard) = self.calls.lock() {
                guard.push(format!("quantize {quant}"));
            }
            std::fs::write(outfile, b"GGUF-QUANT").expect("escribir quant ficticio");
            let result = if self.fail_quantize {
                Err(AppError::ConverterFailed {
                    tool: QUANTIZE_BIN.to_string(),
                    message: "boom".to_string(),
                })
            } else {
                Ok(())
            };
            async move { result }
        }
    }

    /// Cliente de Ollama falso que registra las peticiones de creación.
    #[derive(Default)]
    struct FakeOllama {
        created: Mutex<Vec<CreateModelRequest>>,
    }

    impl FakeOllama {
        fn created(&self) -> Vec<CreateModelRequest> {
            self.created.lock().map(|g| g.clone()).unwrap_or_default()
        }
    }

    impl OllamaClient for FakeOllama {
        async fn version(&self) -> Result<String, AppError> {
            Ok("1.2.3".to_string())
        }

        fn blob_exists(
            &self,
            _digest: &Sha256Digest,
        ) -> impl Future<Output = Result<bool, AppError>> + Send {
            let exists = true;
            async move { Ok(exists) }
        }

        fn push_blob(
            &self,
            _digest: &Sha256Digest,
            _path: &Path,
        ) -> impl Future<Output = Result<(), AppError>> + Send {
            let result: Result<(), AppError> = Ok(());
            async move { result }
        }

        fn create(
            &self,
            request: &CreateModelRequest,
        ) -> impl Future<Output = Result<(), AppError>> + Send {
            if let Ok(mut guard) = self.created.lock() {
                guard.push(request.clone());
            }
            async { Ok(()) }
        }

        async fn pull(&self, _request: &PullRequest) -> Result<String, AppError> {
            Ok("success".to_string())
        }

        fn show(&self, _model: &str) -> impl Future<Output = Result<ModelInfo, AppError>> + Send {
            let info = ModelInfo::default();
            async move { Ok(info) }
        }

        async fn chat(&self, _request: &ChatRequest) -> Result<ChatResponse, AppError> {
            Ok(ChatResponse::default())
        }

        async fn generate(&self, _request: &GenerateRequest) -> Result<GenerateResponse, AppError> {
            Ok(GenerateResponse::default())
        }

        async fn running(&self) -> Result<Vec<RunningModel>, AppError> {
            Ok(Vec::new())
        }

        async fn tags(&self) -> Result<Vec<TaggedModel>, AppError> {
            Ok(Vec::new())
        }
    }

    /// Observador que registra las etapas notificadas.
    #[derive(Default)]
    struct RecordingProgress {
        stages: Mutex<Vec<(Stage, Option<String>)>>,
    }

    impl RecordingProgress {
        fn recorded(&self) -> Vec<(Stage, Option<String>)> {
            self.stages.lock().map(|g| g.clone()).unwrap_or_default()
        }
    }

    impl Progress for RecordingProgress {
        fn stage(&self, stage: Stage) {
            if let Ok(mut guard) = self.stages.lock() {
                guard.push((stage, None));
            }
        }

        fn stage_with_value(&self, stage: Stage, value: &str) {
            if let Ok(mut guard) = self.stages.lock() {
                guard.push((stage, Some(value.to_string())));
            }
        }
    }

    /// Directorio con un `.safetensors` ficticio.
    fn safetensors_dir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("model.safetensors"), b"SAFETENSORS").expect("pesos");
        std::fs::write(dir.path().join("config.json"), b"{}").expect("config");
        dir
    }

    /// Recuento de entradas de `dir` (para comprobar la limpieza).
    fn entries(dir: &Path) -> usize {
        std::fs::read_dir(dir).expect("leer dir").count()
    }

    #[tokio::test]
    async fn converts_quantizes_and_creates() {
        let source = safetensors_dir();
        let tmp_root = tempfile::tempdir().expect("tmp_root");
        let client = FakeOllama::default();
        let converter = FakeConverter::default();
        let progress = RecordingProgress::default();
        let opts = ConvertOptions {
            name: "mi-modelo",
            source_dir: source.path(),
            quantize: Some("q4_k_m"),
            tmp_root: Some(tmp_root.path()),
        };

        let report = run(&client, &converter, &opts, &progress)
            .await
            .expect("flujo ok");

        assert_eq!(report.model, "mi-modelo");
        assert_eq!(
            converter.calls(),
            vec![
                "find_tools".to_string(),
                format!("convert {}", source.path().display()),
                "quantize Q4_K_M".to_string(),
            ],
            "el quant se canonicaliza a mayúsculas antes de invocar"
        );
        assert_eq!(
            progress.recorded(),
            vec![
                (Stage::ConvertingSafetensors, None),
                (Stage::Quantizing, Some("Q4_K_M".to_string())),
                (Stage::Creating, None),
            ]
        );

        let created = client.created();
        assert_eq!(created.len(), 1);
        assert_eq!(
            created[0].files.keys().cloned().collect::<Vec<_>>(),
            vec!["model-Q4_K_M.gguf".to_string()],
            "el FROM es el GGUF cuantizado"
        );
        assert_eq!(
            entries(tmp_root.path()),
            0,
            "el directorio temporal debe limpiarse tras el éxito"
        );
    }

    #[tokio::test]
    async fn creates_from_f16_without_quantize() {
        let source = safetensors_dir();
        let tmp_root = tempfile::tempdir().expect("tmp_root");
        let client = FakeOllama::default();
        let converter = FakeConverter::default();
        let progress = RecordingProgress::default();
        let opts = ConvertOptions {
            name: "sin-quant",
            source_dir: source.path(),
            quantize: None,
            tmp_root: Some(tmp_root.path()),
        };

        run(&client, &converter, &opts, &progress)
            .await
            .expect("flujo ok");

        assert_eq!(
            converter.calls(),
            vec![
                "find_tools".to_string(),
                format!("convert {}", source.path().display()),
            ]
        );
        assert_eq!(
            progress.recorded(),
            vec![
                (Stage::ConvertingSafetensors, None),
                (Stage::Creating, None),
            ]
        );
        assert_eq!(
            client.created()[0]
                .files
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            vec!["model-f16.gguf".to_string()]
        );
        assert_eq!(entries(tmp_root.path()), 0, "limpieza tras el éxito");
    }

    #[tokio::test]
    async fn fails_when_tool_is_missing_without_converting() {
        let source = safetensors_dir();
        let tmp_root = tempfile::tempdir().expect("tmp_root");
        let client = FakeOllama::default();
        let converter = FakeConverter {
            missing_tool: Some("llama-quantize".to_string()),
            ..FakeConverter::default()
        };
        let opts = ConvertOptions {
            name: "sin-tool",
            source_dir: source.path(),
            quantize: None,
            tmp_root: Some(tmp_root.path()),
        };

        let err = run(&client, &converter, &opts, &())
            .await
            .expect_err("debe fallar sin herramientas");

        assert!(
            matches!(err, AppError::ConverterToolMissing { .. }),
            "{err}"
        );
        assert_eq!(converter.calls(), vec!["find_tools".to_string()]);
        assert!(client.created().is_empty(), "no debe crear el modelo");
        assert_eq!(entries(tmp_root.path()), 0, "no debe dejar temporales");
    }

    #[tokio::test]
    async fn rejects_invalid_quant_before_touching_tools() {
        let source = safetensors_dir();
        let tmp_root = tempfile::tempdir().expect("tmp_root");
        let client = FakeOllama::default();
        let converter = FakeConverter::default();
        let opts = ConvertOptions {
            name: "quant-malo",
            source_dir: source.path(),
            quantize: Some("NO_EXISTE"),
            tmp_root: Some(tmp_root.path()),
        };

        let err = run(&client, &converter, &opts, &())
            .await
            .expect_err("debe rechazar el quant");

        assert!(matches!(err, AppError::UnsupportedQuantize { .. }), "{err}");
        assert!(err.to_string().contains("Q4_K_M"), "{err}");
        assert!(
            converter.calls().is_empty(),
            "no debe invocar llama.cpp antes de validar el quant"
        );
        assert!(client.created().is_empty(), "no debe crear el modelo");
    }

    #[tokio::test]
    async fn cleans_up_when_conversion_fails() {
        let source = safetensors_dir();
        let tmp_root = tempfile::tempdir().expect("tmp_root");
        let client = FakeOllama::default();
        let converter = FakeConverter {
            fail_convert: true,
            ..FakeConverter::default()
        };
        let opts = ConvertOptions {
            name: "fallo-conv",
            source_dir: source.path(),
            quantize: None,
            tmp_root: Some(tmp_root.path()),
        };

        let err = run(&client, &converter, &opts, &())
            .await
            .expect_err("debe propagar el fallo de conversión");

        assert!(matches!(err, AppError::ConverterFailed { .. }), "{err}");
        assert!(client.created().is_empty(), "no debe crear el modelo");
        assert_eq!(
            entries(tmp_root.path()),
            0,
            "el directorio temporal debe limpiarse aunque falle"
        );
    }

    #[tokio::test]
    async fn cleans_up_when_quantize_fails() {
        let source = safetensors_dir();
        let tmp_root = tempfile::tempdir().expect("tmp_root");
        let client = FakeOllama::default();
        let converter = FakeConverter {
            fail_quantize: true,
            ..FakeConverter::default()
        };
        let opts = ConvertOptions {
            name: "fallo-quant",
            source_dir: source.path(),
            quantize: Some("Q6_K"),
            tmp_root: Some(tmp_root.path()),
        };

        let err = run(&client, &converter, &opts, &())
            .await
            .expect_err("debe propagar el fallo de cuantización");

        assert!(matches!(err, AppError::ConverterFailed { .. }), "{err}");
        assert!(client.created().is_empty(), "no debe crear el modelo");
        assert_eq!(
            entries(tmp_root.path()),
            0,
            "limpieza aunque falle el quant"
        );
    }

    #[test]
    fn detects_missing_tools_with_an_empty_path() {
        let converter = LlamaCppConverter {
            path: Some(OsString::from("")),
        };
        let err = converter.find_tools().expect_err("sin PATH no hay tools");
        assert!(
            matches!(err, AppError::ConverterToolMissing { .. }),
            "{err}"
        );
    }

    /// El `Modelfile` del reporte no debe apuntar al GGUF temporal (ya eliminado).
    #[tokio::test]
    async fn report_omits_modelfile_referencing_temporary_gguf() {
        let source = safetensors_dir();
        let tmp_root = tempfile::tempdir().expect("tmp_root");
        let client = FakeOllama::default();
        let converter = FakeConverter::default();
        let opts = ConvertOptions {
            name: "mi-modelo",
            source_dir: source.path(),
            quantize: Some("q4_k_m"),
            tmp_root: Some(tmp_root.path()),
        };

        let report = run(&client, &converter, &opts, &())
            .await
            .expect("flujo ok");

        let tmp_root = tmp_root.path().display().to_string();
        assert!(
            report.modelfile.is_empty(),
            "el reporte debe omitir el Modelfile cuando la fuente es temporal: {}",
            report.modelfile
        );
        assert!(
            !report.modelfile.contains(&tmp_root),
            "el Modelfile no debe apuntar al directorio temporal: {}",
            report.modelfile
        );
        assert!(
            !report.modelfile.contains("modelforge-llamacpp"),
            "el Modelfile no debe contener una ruta temporal inexistente: {}",
            report.modelfile
        );
        assert!(
            !report.modelfile.contains(".gguf"),
            "el Modelfile no debe referenciar el GGUF temporal: {}",
            report.modelfile
        );
    }

    /// Escribe un fichero con permiso de ejecución (simula un binario en `PATH`).
    #[cfg(unix)]
    fn write_executable(path: &Path) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, b"#!/bin/sh\nexit 0\n").expect("escribir ejecutable");
        let mut perms = std::fs::metadata(path).expect("metadata").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(path, perms).expect("chmod +x");
    }

    /// Conversor real apuntando a un `PATH` controlado.
    #[cfg(unix)]
    fn converter_with_path(dir: &Path) -> LlamaCppConverter {
        LlamaCppConverter {
            path: Some(dir.as_os_str().to_os_string()),
        }
    }

    /// Las tres herramientas se localizan: `python3`, el script y el binario.
    #[cfg(unix)]
    #[test]
    fn finds_all_tools_in_path() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_executable(&dir.path().join(PYTHON));
        write_executable(&dir.path().join(QUANTIZE_BIN));
        // El script de conversión no necesita bit de ejecución: se invoca con `python3`.
        std::fs::write(dir.path().join(CONVERT_SCRIPT), b"print('x')\n").expect("script");

        let tools = converter_with_path(dir.path())
            .find_tools()
            .expect("las tres herramientas deben encontrarse");

        assert_eq!(tools.python, dir.path().join(PYTHON));
        assert_eq!(tools.convert_script, dir.path().join(CONVERT_SCRIPT));
        assert_eq!(tools.quantize_bin, dir.path().join(QUANTIZE_BIN));
    }

    /// Si falta `python3`, el error debe ser accionable y nombrarlo.
    #[cfg(unix)]
    #[test]
    fn reports_missing_python3() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_executable(&dir.path().join(QUANTIZE_BIN));
        std::fs::write(dir.path().join(CONVERT_SCRIPT), b"print('x')\n").expect("script");

        let err = converter_with_path(dir.path())
            .find_tools()
            .expect_err("falta python3");

        match err {
            AppError::ConverterToolMissing { tool } => assert_eq!(tool, PYTHON),
            other => panic!("se esperaba ConverterToolMissing, se obtuvo {other:?}"),
        }
    }

    /// Un binario sin bit de ejecución debe tratarse como herramienta ausente.
    #[cfg(unix)]
    #[test]
    fn treats_non_executable_binary_as_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        write_executable(&dir.path().join(PYTHON));
        // `llama-quantize` existe pero sin bit de ejecución: no es utilizable.
        std::fs::write(dir.path().join(QUANTIZE_BIN), b"bin\n").expect("bin sin +x");
        std::fs::write(dir.path().join(CONVERT_SCRIPT), b"print('x')\n").expect("script");

        let err = converter_with_path(dir.path())
            .find_tools()
            .expect_err("un binario sin +x debe considerarse ausente");

        match err {
            AppError::ConverterToolMissing { tool } => assert_eq!(tool, QUANTIZE_BIN),
            other => panic!("se esperaba ConverterToolMissing, se obtuvo {other:?}"),
        }
    }
}
