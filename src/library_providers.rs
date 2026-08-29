//! Capability-specific provider contracts and composition helpers.
//!
//! These roles let a Library compose catalog, content, query, and refresh implementations from
//! different infrastructure adapters while still presenting one [`LibraryProvider`] to the Runtime.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::library::{
    KnowledgeNode, KnowledgeUri, LibraryCapability, LibraryCatalog, LibraryId, LibraryProvider,
    LibraryQuery, LibraryQueryResult, LibraryResult,
};

/// Provides hierarchical knowledge content independent of physical storage.
pub trait ContentProvider: Send + Sync {
    /// Stable provider identity for provenance and diagnostics.
    fn provider_id(&self) -> &str;

    /// Lists direct children under a logical path.
    fn list(&self, library: &LibraryId, path: &str) -> LibraryResult<Vec<KnowledgeNode>>;

    /// Reads a canonical logical knowledge node.
    fn read(&self, uri: &KnowledgeUri) -> LibraryResult<String>;
}

/// Provides Library-owned semantic navigation.
pub trait CatalogProvider: Send + Sync {
    /// Stable provider identity for provenance and diagnostics.
    fn provider_id(&self) -> &str;

    /// Returns the current semantic catalog for the Library.
    fn catalog(&self, library: &LibraryId) -> LibraryResult<LibraryCatalog>;
}

/// Provides retrieval/query behavior for a Library.
pub trait QueryProvider: Send + Sync {
    /// Stable provider identity for provenance and diagnostics.
    fn provider_id(&self) -> &str;

    /// Executes a query using provider-specific retrieval intelligence.
    fn query(&self, library: &LibraryId, query: &LibraryQuery) -> LibraryResult<LibraryQueryResult>;
}

/// Refreshes provider-derived state such as caches, remote metadata, or indexes.
pub trait RefreshProvider: Send + Sync {
    /// Stable provider identity for diagnostics.
    fn provider_id(&self) -> &str;

    /// Refreshes provider state.
    fn refresh(&self) -> LibraryResult<()>;
}

/// Composes capability-specific providers into one Runtime-facing Library provider.
///
/// For example a Library may use a generated catalog, an object-storage content provider, and an
/// agent-backed query provider without changing Runtime routing or Library identity.
#[derive(Default)]
pub struct CompositeLibraryProvider {
    id: String,
    catalog: Option<Arc<dyn CatalogProvider>>,
    content: Option<Arc<dyn ContentProvider>>,
    query: Option<Arc<dyn QueryProvider>>,
    refresh: Option<Arc<dyn RefreshProvider>>,
}

impl std::fmt::Debug for CompositeLibraryProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompositeLibraryProvider")
            .field("id", &self.id)
            .field("catalog", &self.catalog.as_ref().map(|provider| provider.provider_id()))
            .field("content", &self.content.as_ref().map(|provider| provider.provider_id()))
            .field("query", &self.query.as_ref().map(|provider| provider.provider_id()))
            .field("refresh", &self.refresh.as_ref().map(|provider| provider.provider_id()))
            .finish()
    }
}

impl CompositeLibraryProvider {
    /// Creates an empty composition with a stable Runtime-facing provider identity.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            ..Self::default()
        }
    }

    /// Sets the semantic catalog provider.
    pub fn with_catalog(mut self, provider: Arc<dyn CatalogProvider>) -> Self {
        self.catalog = Some(provider);
        self
    }

    /// Sets the content provider.
    pub fn with_content(mut self, provider: Arc<dyn ContentProvider>) -> Self {
        self.content = Some(provider);
        self
    }

    /// Sets the query provider.
    pub fn with_query(mut self, provider: Arc<dyn QueryProvider>) -> Self {
        self.query = Some(provider);
        self
    }

    /// Sets the refresh provider.
    pub fn with_refresh(mut self, provider: Arc<dyn RefreshProvider>) -> Self {
        self.refresh = Some(provider);
        self
    }
}

impl LibraryProvider for CompositeLibraryProvider {
    fn provider_id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> BTreeSet<LibraryCapability> {
        let mut capabilities = BTreeSet::new();
        if self.catalog.is_some() {
            capabilities.insert(LibraryCapability::Catalog);
        }
        if self.content.is_some() {
            capabilities.insert(LibraryCapability::List);
            capabilities.insert(LibraryCapability::Read);
        }
        if self.query.is_some() {
            capabilities.insert(LibraryCapability::Query);
        }
        if self.refresh.is_some() {
            capabilities.insert(LibraryCapability::Refresh);
        }
        capabilities
    }

    fn catalog(&self, library: &LibraryId) -> LibraryResult<LibraryCatalog> {
        self.catalog
            .as_ref()
            .expect("Runtime checks declared Catalog capability before dispatch")
            .catalog(library)
    }

    fn list(&self, library: &LibraryId, path: &str) -> LibraryResult<Vec<KnowledgeNode>> {
        self.content
            .as_ref()
            .expect("Runtime checks declared List capability before dispatch")
            .list(library, path)
    }

    fn read(&self, uri: &KnowledgeUri) -> LibraryResult<String> {
        self.content
            .as_ref()
            .expect("Runtime checks declared Read capability before dispatch")
            .read(uri)
    }

    fn query(&self, library: &LibraryId, query: &LibraryQuery) -> LibraryResult<LibraryQueryResult> {
        self.query
            .as_ref()
            .expect("Runtime checks declared Query capability before dispatch")
            .query(library, query)
    }

    fn refresh(&self) -> LibraryResult<()> {
        self.refresh
            .as_ref()
            .expect("Runtime checks declared Refresh capability before dispatch")
            .refresh()
    }
}
