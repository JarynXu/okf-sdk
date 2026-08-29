//! Pluggable Library source acquisition contracts.

use std::sync::Arc;

use crate::library::{LibraryError, LibraryInstance, LibraryResult, LibrarySource};

/// Resolves one class of [`LibrarySource`] into a Runtime-ready Library instance.
///
/// Source resolution is deliberately separate from content/query provider behavior. A resolver may
/// materialize a Git repository, attach to object storage, or return a fully remote/virtual provider.
pub trait LibrarySourceResolver: Send + Sync {
    /// Stable resolver identity.
    fn resolver_id(&self) -> &str;

    /// Whether this resolver understands the supplied source descriptor.
    fn supports(&self, source: &LibrarySource) -> bool;

    /// Resolves the source into a Runtime-ready Library instance.
    fn resolve(&self, source: &LibrarySource) -> LibraryResult<LibraryInstance>;
}

/// Ordered registry of source resolvers used by an application/runtime adapter.
#[derive(Default)]
pub struct LibrarySourceResolvers {
    resolvers: Vec<Arc<dyn LibrarySourceResolver>>,
}

impl std::fmt::Debug for LibrarySourceResolvers {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_list()
            .entries(self.resolvers.iter().map(|resolver| resolver.resolver_id()))
            .finish()
    }
}

impl LibrarySourceResolvers {
    /// Creates an empty resolver registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a resolver. Earlier resolvers have higher priority when multiple support a source.
    pub fn register(&mut self, resolver: Arc<dyn LibrarySourceResolver>) {
        self.resolvers.push(resolver);
    }

    /// Resolves a source through the first compatible resolver.
    pub fn resolve(&self, source: &LibrarySource) -> LibraryResult<LibraryInstance> {
        self.resolvers
            .iter()
            .find(|resolver| resolver.supports(source))
            .ok_or_else(|| {
                LibraryError::Provider(format!("no source resolver supports {source:?}"))
            })?
            .resolve(source)
    }
}
