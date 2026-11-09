//! Abstracción del Hub de HuggingFace: identificadores, listado y descarga.
//!
//! Toda la red queda aislada tras el trait [`HfClient`], de modo que la lógica de
//! importación (`crate::import`) se puede probar sin conexión inyectando una
//! implementación falsa. La implementación real, [`HfHubClient`], se apoya en
//! `hf-hub`, que gestiona xet, cache y los reintentos del Hub.

use std::future::Future;

use futures_util::StreamExt as _;
use tokio::fs::File;
use tokio::io::AsyncWriteExt as _;

use crate::error::AppError;

/// Identificador de un repositorio de HuggingFace en formato `owner/name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoId {
    owner: String,
    name: String,
}

impl RepoId {
    /// Parsea un identificador `owner/name`.
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::InvalidRepo`] si no hay exactamente dos segmentos no
    /// vacíos separados por `/`.
    pub fn parse(input: &str) -> Result<Self, AppError> {
        let mut parts = input.trim().split('/');
        match (parts.next(), parts.next(), parts.next()) {
            (Some(owner), Some(name), None) if !owner.is_empty() && !name.is_empty() => Ok(Self {
                owner: owner.to_string(),
                name: name.to_string(),
            }),
            _ => Err(AppError::InvalidRepo {
                input: input.to_string(),
            }),
        }
    }

    /// Propietario del repositorio (`org` o usuario).
    #[must_use]
    pub fn owner(&self) -> &str {
        &self.owner
    }

    /// Nombre del repositorio.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Identificador completo en formato `owner/name`.
    #[must_use]
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

impl std::fmt::Display for RepoId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.slug())
    }
}

/// Fichero remoto del repositorio: ruta relativa y tamaño en bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteFile {
    /// Ruta del fichero dentro del repositorio.
    pub path: String,
    /// Tamaño del fichero en bytes.
    pub size: u64,
}

/// Receptor de los avances de una descarga.
pub trait ProgressSink: Send {
    /// Notifica que se han escrito `downloaded` bytes de un total de `total`.
    fn on_bytes(&mut self, downloaded: u64, total: u64);
}

impl<F> ProgressSink for F
where
    F: FnMut(u64, u64) + Send,
{
    fn on_bytes(&mut self, downloaded: u64, total: u64) {
        self(downloaded, total);
    }
}

/// Cliente del Hub, aislado para permitir dobles de prueba sin red.
pub trait HfClient {
    /// Lista recursivamente los ficheros del repositorio.
    fn list_files(
        &self,
        repo: &RepoId,
    ) -> impl Future<Output = Result<Vec<RemoteFile>, AppError>> + Send;

    /// Descarga `path` a partir de `offset` y vuelca los bytes en `file`.
    ///
    /// `file` debe estar abierto en modo *append* (o recién truncado si `offset`
    /// es 0). `progress` recibe el total acumulado de bytes escritos. La escritura
    /// es asíncrona (`tokio::fs`), sin bloquear el reactor.
    fn download_from(
        &self,
        repo: &RepoId,
        path: &str,
        offset: u64,
        total: u64,
        file: &mut File,
        progress: &mut dyn ProgressSink,
    ) -> impl Future<Output = Result<(), AppError>> + Send;
}

/// Implementación real de [`HfClient`] sobre `hf-hub`.
pub struct HfHubClient {
    client: hf_hub::HFClient,
}

impl HfHubClient {
    /// Crea el cliente resolviendo el token desde el entorno (`HF_TOKEN`).
    ///
    /// # Errors
    ///
    /// Devuelve [`AppError::Hf`] si `hf-hub` no puede inicializarse.
    pub fn new() -> Result<Self, AppError> {
        Ok(Self {
            client: hf_hub::HFClient::new()?,
        })
    }
}

impl HfClient for HfHubClient {
    fn list_files(
        &self,
        repo: &RepoId,
    ) -> impl Future<Output = Result<Vec<RemoteFile>, AppError>> + Send {
        let repo = repo.clone();
        async move {
            let handle = self.client.model(repo.owner(), repo.name());
            let stream = handle.list_tree().recursive(true).send()?;
            let mut stream = Box::pin(stream);
            let mut files = Vec::new();
            while let Some(entry) = stream.next().await {
                if let hf_hub::repository::RepoTreeEntry::File { path, size, .. } = entry? {
                    files.push(RemoteFile { path, size });
                }
            }
            Ok(files)
        }
    }

    fn download_from(
        &self,
        repo: &RepoId,
        path: &str,
        offset: u64,
        total: u64,
        file: &mut File,
        progress: &mut dyn ProgressSink,
    ) -> impl Future<Output = Result<(), AppError>> + Send {
        let repo = repo.clone();
        let path = path.to_string();
        async move {
            let handle = self.client.model(repo.owner(), repo.name());
            let range = if offset > 0 {
                Some(offset..total.max(offset))
            } else {
                None
            };
            let (_len, stream) = handle
                .download_file_stream()
                .filename(path)
                .maybe_range(range)
                .send()
                .await?;
            let mut stream = Box::pin(stream);
            let mut written = offset;
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                file.write_all(&chunk).await?;
                written += chunk.len() as u64;
                progress.on_bytes(written, total);
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_owner_and_name() {
        let repo = RepoId::parse("org/model").expect("debe parsear");
        assert_eq!(repo.owner(), "org");
        assert_eq!(repo.name(), "model");
        assert_eq!(repo.slug(), "org/model");
        assert_eq!(repo.to_string(), "org/model");
    }

    #[test]
    fn trims_surrounding_whitespace() {
        let repo = RepoId::parse("  org/model  ").expect("debe parsear");
        assert_eq!(repo.slug(), "org/model");
    }

    #[test]
    fn rejects_malformed_ids() {
        for bad in ["model", "org/", "/model", "org/sub/model", "org//model", ""] {
            assert!(
                RepoId::parse(bad).is_err(),
                "`{bad}` no debería ser un repo válido"
            );
        }
    }
}
