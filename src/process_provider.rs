//! External process-backed Library provider adapter.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use serde::de::DeserializeOwned;
use wait_timeout::ChildExt;

use crate::library::{
    KnowledgeNode, KnowledgeUri, LibraryCapability, LibraryCatalog, LibraryError, LibraryId,
    LibraryProvider, LibraryQuery, LibraryQueryResult, LibraryResult,
};
use crate::provider_protocol::{
    ProviderRequest, ProviderResponse, decode_provider_response,
};

/// Executes Library capabilities through the `okf-provider/1` process protocol.
#[derive(Clone, Debug)]
pub struct ProcessLibraryProvider {
    id: String,
    library: LibraryId,
    command: PathBuf,
    args: Vec<String>,
    cwd: Option<PathBuf>,
    env: BTreeMap<String, String>,
    capabilities: BTreeSet<LibraryCapability>,
    timeout: Duration,
}

impl ProcessLibraryProvider {
    /// Creates a process-backed provider with a default 30 second request timeout.
    pub fn new(
        id: impl Into<String>,
        library: LibraryId,
        command: impl Into<PathBuf>,
        capabilities: impl IntoIterator<Item = LibraryCapability>,
    ) -> Self {
        Self {
            id: id.into(),
            library,
            command: command.into(),
            args: Vec::new(),
            cwd: None,
            env: BTreeMap::new(),
            capabilities: capabilities.into_iter().collect(),
            timeout: Duration::from_secs(30),
        }
    }

    /// Sets process arguments.
    pub fn args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the process working directory.
    pub fn cwd(mut self, path: impl Into<PathBuf>) -> Self {
        self.cwd = Some(path.into());
        self
    }

    /// Adds one explicitly authorized environment variable.
    pub fn env(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(name.into(), value.into());
        self
    }

    /// Sets the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn ensure_library(&self, library: &LibraryId) -> LibraryResult<()> {
        if library == &self.library {
            Ok(())
        } else {
            Err(LibraryError::Provider(format!(
                "process provider '{}' belongs to Library '{}' but was called for '{}'",
                self.id, self.library, library
            )))
        }
    }

    fn invoke<T: DeserializeOwned>(&self, request: &ProviderRequest) -> LibraryResult<T> {
        let mut command = Command::new(&self.command);
        command
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("OKF_LIBRARY_ID", self.library.as_str())
            .env("OKF_PROVIDER_ID", &self.id)
            .envs(&self.env);
        if let Some(cwd) = &self.cwd {
            command.current_dir(cwd);
        }

        let mut child = command.spawn().map_err(|error| {
            LibraryError::Provider(format!(
                "failed to start process provider '{}': {error}",
                self.id
            ))
        })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| LibraryError::Provider("process stdout was not piped".to_owned()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| LibraryError::Provider("process stderr was not piped".to_owned()))?;
        let stdout_reader = thread::spawn(move || read_all(stdout));
        let stderr_reader = thread::spawn(move || read_all(stderr));

        let request_bytes = serde_json::to_vec(request)
            .map_err(|error| LibraryError::Provider(format!("failed to encode request: {error}")))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&request_bytes).map_err(|error| {
                LibraryError::Provider(format!("failed to write process provider request: {error}"))
            })?;
            stdin.write_all(b"\n").map_err(|error| {
                LibraryError::Provider(format!("failed to finish process provider request: {error}"))
            })?;
        }

        let status = child
            .wait_timeout(self.timeout)
            .map_err(|error| LibraryError::Provider(format!("failed waiting for provider: {error}")))?;
        let status = match status {
            Some(status) => status,
            None => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(LibraryError::Provider(format!(
                    "process provider '{}' timed out after {:?}",
                    self.id, self.timeout
                )));
            }
        };
        let stdout = stdout_reader
            .join()
            .map_err(|_| LibraryError::Provider("provider stdout reader panicked".to_owned()))??;
        let stderr = stderr_reader
            .join()
            .map_err(|_| LibraryError::Provider("provider stderr reader panicked".to_owned()))??;

        if !status.success() {
            return Err(LibraryError::Provider(format!(
                "process provider '{}' exited with {}: {}",
                self.id,
                status,
                String::from_utf8_lossy(&stderr).trim()
            )));
        }

        let response: ProviderResponse = decode_provider_response(&stdout)?;
        response.into_typed()
    }
}

impl LibraryProvider for ProcessLibraryProvider {
    fn provider_id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> BTreeSet<LibraryCapability> {
        self.capabilities.clone()
    }

    fn catalog(&self, library: &LibraryId) -> LibraryResult<LibraryCatalog> {
        self.ensure_library(library)?;
        self.invoke(&ProviderRequest::catalog(library.clone()))
    }

    fn list(&self, library: &LibraryId, path: &str) -> LibraryResult<Vec<KnowledgeNode>> {
        self.ensure_library(library)?;
        self.invoke(&ProviderRequest::list(library.clone(), path))
    }

    fn read(&self, uri: &KnowledgeUri) -> LibraryResult<String> {
        self.ensure_library(uri.library())?;
        self.invoke(&ProviderRequest::read(uri.clone()))
    }

    fn query(
        &self,
        library: &LibraryId,
        query: &LibraryQuery,
    ) -> LibraryResult<LibraryQueryResult> {
        self.ensure_library(library)?;
        self.invoke(&ProviderRequest::query(library.clone(), query.clone()))
    }

    fn refresh(&self) -> LibraryResult<()> {
        self.invoke(&ProviderRequest::refresh(self.library.clone()))
    }
}

fn read_all(mut reader: impl Read) -> LibraryResult<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .read_to_end(&mut bytes)
        .map_err(|error| LibraryError::Provider(format!("failed reading provider output: {error}")))?;
    Ok(bytes)
}
