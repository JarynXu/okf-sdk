//! Pluggable OKF Library domain model and runtime.
//!
//! The Library model is storage-independent. Concrete storage and transport technologies are
//! adapters that implement [`LibraryProvider`].

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type for Library operations.
pub type LibraryResult<T> = std::result::Result<T, LibraryError>;

/// Stable Library runtime errors.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum LibraryError {
    /// A Library identifier is syntactically invalid.
    #[error("invalid library id: {0}")]
    InvalidLibraryId(String),
    /// A knowledge URI is syntactically invalid.
    #[error("invalid knowledge URI: {0}")]
    InvalidUri(String),
    /// The requested Library is unknown to the registry.
    #[error("unknown library: {0}")]
    UnknownLibrary(String),
    /// The requested Library is registered but not mounted.
    #[error("library is not mounted: {0}")]
    NotMounted(String),
    /// A provider does not implement the requested capability.
    #[error("unsupported capability: {0:?}")]
    UnsupportedCapability(LibraryCapability),
    /// A node could not be resolved.
    #[error("knowledge node not found: {0}")]
    NodeNotFound(String),
    /// Registration conflicts with an existing Library identity.
    #[error("library already registered: {0}")]
    Conflict(String),
    /// Provider-specific failure represented without leaking a transport type into the domain.
    #[error("provider failed: {0}")]
    Provider(String),
}

/// Stable identifier of an independently mountable knowledge Library.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct LibraryId(String);

impl LibraryId {
    /// Parses a portable Library identifier.
    pub fn parse(value: impl Into<String>) -> LibraryResult<Self> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
            });
        if !valid {
            return Err(LibraryError::InvalidLibraryId(value));
        }
        Ok(Self(value))
    }

    /// Returns the identifier text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for LibraryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Canonical address of a logical knowledge node.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct KnowledgeUri {
    library: LibraryId,
    path: String,
}

impl KnowledgeUri {
    /// Creates a URI from a Library identity and logical path.
    pub fn new(library: LibraryId, path: impl Into<String>) -> LibraryResult<Self> {
        let path = normalize_path(&path.into())?;
        Ok(Self { library, path })
    }

    /// Parses the canonical `okf://<library>/<path>` form.
    pub fn parse(value: &str) -> LibraryResult<Self> {
        let remainder = value
            .strip_prefix("okf://")
            .ok_or_else(|| LibraryError::InvalidUri(value.to_owned()))?;
        let (library, path) = remainder.split_once('/').unwrap_or((remainder, ""));
        Self::new(LibraryId::parse(library)?, path)
    }

    /// Owning Library.
    pub fn library(&self) -> &LibraryId {
        &self.library
    }

    /// Logical path relative to the Library root.
    pub fn path(&self) -> &str {
        &self.path
    }
}

impl fmt::Display for KnowledgeUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.path.is_empty() {
            write!(formatter, "okf://{}/", self.library)
        } else {
            write!(formatter, "okf://{}/{}", self.library, self.path)
        }
    }
}

fn normalize_path(value: &str) -> LibraryResult<String> {
    let value = value.trim().trim_matches('/');
    if value.split('/').any(|segment| segment == "..") || value.contains('\\') {
        return Err(LibraryError::InvalidUri(value.to_owned()));
    }
    Ok(value.to_owned())
}

/// Runtime capability exposed by a Library provider.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LibraryCapability {
    /// Enumerate logical nodes.
    List,
    /// Read logical node content.
    Read,
    /// Provide a semantic catalog.
    Catalog,
    /// Execute a query.
    Query,
    /// Refresh derived provider state.
    Refresh,
    /// Mutate or maintain knowledge content.
    Maintain,
}

/// How a Library can be obtained or resolved.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum LibrarySource {
    /// Existing local directory.
    Local {
        /// Local source path.
        path: PathBuf,
    },
    /// Git repository source.
    Git {
        /// Repository URL.
        repository: String,
        /// Optional branch, tag, or commit reference.
        reference: Option<String>,
    },
    /// Provider-defined source descriptor.
    Custom {
        /// Stable source kind understood by an adapter.
        kind: String,
        /// Opaque source location.
        location: String,
    },
}

/// Portable declaration of a Library instance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibraryManifest {
    /// Stable identity.
    pub id: LibraryId,
    /// Human-readable display name.
    pub name: String,
    /// Library content version when known.
    pub version: Option<String>,
    /// Acquisition source when known.
    pub source: Option<LibrarySource>,
    /// Optional source/provider revision used for freshness and caches.
    pub revision: Option<String>,
}

impl LibraryManifest {
    /// Creates a minimal manifest.
    pub fn new(id: LibraryId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            version: None,
            source: None,
            revision: None,
        }
    }
}

/// Type of a logical namespace node.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnowledgeNodeKind {
    /// Container node with children.
    Directory,
    /// Readable content node. It may be physical or virtual.
    Content,
}

/// Metadata for one logical knowledge node.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct KnowledgeNode {
    /// Canonical URI.
    pub uri: KnowledgeUri,
    /// Node kind.
    pub kind: KnowledgeNodeKind,
    /// Optional display title.
    pub title: Option<String>,
    /// Whether the node is generated dynamically.
    pub virtual_node: bool,
}

/// One semantic navigation entry contributed by a Library.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CatalogEntry {
    /// Stable topic identifier within the Library.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Optional explanation of the knowledge scope.
    pub description: Option<String>,
    /// Preferred knowledge URI for this topic.
    pub uri: KnowledgeUri,
    /// Search terms, aliases, or domain vocabulary useful for routing.
    pub terms: BTreeSet<String>,
}

/// Semantic knowledge map produced by a Library.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibraryCatalog {
    /// Owning Library.
    pub library: LibraryId,
    /// Catalog entries in deterministic order.
    pub entries: Vec<CatalogEntry>,
}

impl LibraryCatalog {
    /// Creates an empty catalog for a Library.
    pub fn empty(library: LibraryId) -> Self {
        Self {
            library,
            entries: Vec::new(),
        }
    }
}

/// Query strategy actually used by a provider.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum QueryStrategy {
    /// Exact or identifier-based lookup.
    Exact,
    /// Deterministic lexical retrieval.
    Lexical,
    /// Embedding or other semantic retrieval.
    Semantic,
    /// Graph traversal or graph-aware retrieval.
    Graph,
    /// Multi-step agent-backed retrieval.
    Agentic,
    /// Provider-specific strategy.
    Custom(String),
}

/// Portable query request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibraryQuery {
    /// Natural-language or structured query text.
    pub text: String,
    /// Maximum number of evidence hits.
    pub limit: usize,
}

impl LibraryQuery {
    /// Creates a query with a default limit of 20.
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            limit: 20,
        }
    }

    /// Sets the maximum number of evidence hits.
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }
}

/// One evidence item returned by a query provider.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LibraryQueryHit {
    /// Evidence URI.
    pub uri: KnowledgeUri,
    /// Optional display title.
    pub title: Option<String>,
    /// Optional bounded excerpt.
    pub snippet: Option<String>,
    /// Provider-local relevance score.
    pub score: Option<f64>,
    /// Additional portable metadata.
    pub metadata: BTreeMap<String, String>,
}

/// Portable query result envelope.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LibraryQueryResult {
    /// Optional synthesized answer. Deterministic providers may omit this.
    pub answer: Option<String>,
    /// Evidence items.
    pub hits: Vec<LibraryQueryHit>,
    /// Provider identity useful for provenance and diagnostics.
    pub provider: String,
    /// Retrieval strategy actually used.
    pub strategy: QueryStrategy,
    /// Optional bounded execution provenance.
    pub provenance: BTreeMap<String, String>,
}

/// Polymorphic access to Library knowledge.
///
/// Implementations may be local, Git-materialized, object-storage-backed, remote, generated,
/// database-backed, or agent-backed. Runtime routing depends only on this capability contract.
pub trait LibraryProvider: Send + Sync {
    /// Stable provider identity.
    fn provider_id(&self) -> &str;

    /// Capabilities implemented by this provider.
    fn capabilities(&self) -> BTreeSet<LibraryCapability>;

    /// Returns the semantic catalog.
    fn catalog(&self, library: &LibraryId) -> LibraryResult<LibraryCatalog> {
        let _ = library;
        Err(LibraryError::UnsupportedCapability(
            LibraryCapability::Catalog,
        ))
    }

    /// Lists direct children under a logical path.
    fn list(&self, library: &LibraryId, path: &str) -> LibraryResult<Vec<KnowledgeNode>> {
        let _ = (library, path);
        Err(LibraryError::UnsupportedCapability(LibraryCapability::List))
    }

    /// Reads a logical content node.
    fn read(&self, uri: &KnowledgeUri) -> LibraryResult<String> {
        let _ = uri;
        Err(LibraryError::UnsupportedCapability(LibraryCapability::Read))
    }

    /// Executes a provider-defined query.
    fn query(
        &self,
        library: &LibraryId,
        query: &LibraryQuery,
    ) -> LibraryResult<LibraryQueryResult> {
        let _ = (library, query);
        Err(LibraryError::UnsupportedCapability(
            LibraryCapability::Query,
        ))
    }

    /// Refreshes provider-derived state.
    fn refresh(&self) -> LibraryResult<()> {
        Err(LibraryError::UnsupportedCapability(
            LibraryCapability::Refresh,
        ))
    }
}

/// Resolved Library instance ready for registration and mounting.
#[derive(Clone)]
pub struct LibraryInstance {
    manifest: LibraryManifest,
    provider: Arc<dyn LibraryProvider>,
}

impl fmt::Debug for LibraryInstance {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LibraryInstance")
            .field("manifest", &self.manifest)
            .field("provider_id", &self.provider.provider_id())
            .finish()
    }
}

impl LibraryInstance {
    /// Creates a resolved Library backed by a provider.
    pub fn new(manifest: LibraryManifest, provider: Arc<dyn LibraryProvider>) -> Self {
        Self { manifest, provider }
    }

    /// Manifest.
    pub fn manifest(&self) -> &LibraryManifest {
        &self.manifest
    }

    /// Provider.
    pub fn provider(&self) -> &Arc<dyn LibraryProvider> {
        &self.provider
    }
}

/// Dynamic runtime registry and mount table.
#[derive(Default)]
pub struct LibraryRegistry {
    registered: BTreeMap<LibraryId, LibraryInstance>,
    mounted: BTreeSet<LibraryId>,
}

impl LibraryRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a resolved Library without mounting it.
    pub fn register(&mut self, library: LibraryInstance) -> LibraryResult<()> {
        let id = library.manifest.id.clone();
        if self.registered.contains_key(&id) {
            return Err(LibraryError::Conflict(id.to_string()));
        }
        self.registered.insert(id, library);
        Ok(())
    }

    /// Removes a Library from the registry. Mounted Libraries are unmounted first.
    pub fn unregister(&mut self, id: &LibraryId) -> LibraryResult<LibraryInstance> {
        self.mounted.remove(id);
        self.registered
            .remove(id)
            .ok_or_else(|| LibraryError::UnknownLibrary(id.to_string()))
    }

    /// Mounts a registered Library into the active knowledge space.
    pub fn mount(&mut self, id: &LibraryId) -> LibraryResult<()> {
        if !self.registered.contains_key(id) {
            return Err(LibraryError::UnknownLibrary(id.to_string()));
        }
        self.mounted.insert(id.clone());
        Ok(())
    }

    /// Unmounts a Library without unregistering/materially deleting it.
    pub fn unmount(&mut self, id: &LibraryId) -> LibraryResult<()> {
        if !self.registered.contains_key(id) {
            return Err(LibraryError::UnknownLibrary(id.to_string()));
        }
        self.mounted.remove(id);
        Ok(())
    }

    /// Returns all registered manifests in identity order.
    pub fn libraries(&self) -> Vec<&LibraryManifest> {
        self.registered
            .values()
            .map(LibraryInstance::manifest)
            .collect()
    }

    /// Returns mounted Library identities in deterministic order.
    pub fn mounted(&self) -> impl Iterator<Item = &LibraryId> {
        self.mounted.iter()
    }

    /// Returns whether a Library is currently mounted.
    pub fn is_mounted(&self, id: &LibraryId) -> bool {
        self.mounted.contains(id)
    }

    /// Returns one mounted Library's semantic catalog.
    pub fn catalog(&self, id: &LibraryId) -> LibraryResult<LibraryCatalog> {
        let library = self.mounted_library(id)?;
        require_capability(library.provider.as_ref(), LibraryCapability::Catalog)?;
        library.provider.catalog(id)
    }

    /// Aggregates all mounted Library catalogs while preserving Library boundaries.
    pub fn global_catalog(&self) -> LibraryResult<Vec<LibraryCatalog>> {
        self.mounted
            .iter()
            .map(|id| self.catalog(id))
            .collect::<LibraryResult<Vec<_>>>()
    }

    /// Lists logical children within a mounted Library.
    pub fn list(&self, id: &LibraryId, path: &str) -> LibraryResult<Vec<KnowledgeNode>> {
        let library = self.mounted_library(id)?;
        require_capability(library.provider.as_ref(), LibraryCapability::List)?;
        library.provider.list(id, path)
    }

    /// Reads a canonical knowledge URI from the owning mounted Library.
    pub fn read(&self, uri: &KnowledgeUri) -> LibraryResult<String> {
        let library = self.mounted_library(uri.library())?;
        require_capability(library.provider.as_ref(), LibraryCapability::Read)?;
        library.provider.read(uri)
    }

    /// Queries one mounted Library using that Library's own provider strategy.
    pub fn query(&self, id: &LibraryId, query: &LibraryQuery) -> LibraryResult<LibraryQueryResult> {
        let library = self.mounted_library(id)?;
        require_capability(library.provider.as_ref(), LibraryCapability::Query)?;
        library.provider.query(id, query)
    }

    /// Queries every mounted Library that declares query support.
    pub fn query_all(
        &self,
        query: &LibraryQuery,
    ) -> Vec<(LibraryId, LibraryResult<LibraryQueryResult>)> {
        self.mounted
            .iter()
            .filter_map(|id| {
                let library = self.registered.get(id)?;
                if library
                    .provider
                    .capabilities()
                    .contains(&LibraryCapability::Query)
                {
                    Some((id.clone(), library.provider.query(id, query)))
                } else {
                    None
                }
            })
            .collect()
    }

    fn mounted_library(&self, id: &LibraryId) -> LibraryResult<&LibraryInstance> {
        let library = self
            .registered
            .get(id)
            .ok_or_else(|| LibraryError::UnknownLibrary(id.to_string()))?;
        if !self.mounted.contains(id) {
            return Err(LibraryError::NotMounted(id.to_string()));
        }
        Ok(library)
    }
}

fn require_capability(
    provider: &dyn LibraryProvider,
    capability: LibraryCapability,
) -> LibraryResult<()> {
    if provider.capabilities().contains(&capability) {
        Ok(())
    } else {
        Err(LibraryError::UnsupportedCapability(capability))
    }
}
