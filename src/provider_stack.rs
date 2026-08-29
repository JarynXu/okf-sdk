//! Ordered composition of multiple full Library providers.

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::library::{
    KnowledgeNode, KnowledgeUri, LibraryCapability, LibraryCatalog, LibraryError, LibraryId,
    LibraryProvider, LibraryQuery, LibraryQueryResult, LibraryResult,
};

/// Presents multiple provider adapters as one Runtime-facing Library provider.
///
/// Provider order is significant: the first provider declaring an operation's capability handles
/// that operation. This keeps deployment composition deterministic while allowing catalog, content,
/// query, and refresh behavior to come from different adapters.
#[derive(Default)]
pub struct ProviderStack {
    id: String,
    providers: Vec<Arc<dyn LibraryProvider>>,
}

impl std::fmt::Debug for ProviderStack {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderStack")
            .field("id", &self.id)
            .field(
                "providers",
                &self
                    .providers
                    .iter()
                    .map(|provider| provider.provider_id())
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ProviderStack {
    /// Creates an empty ordered provider stack.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            providers: Vec::new(),
        }
    }

    /// Appends one provider at the lowest remaining precedence.
    pub fn push(&mut self, provider: Arc<dyn LibraryProvider>) {
        self.providers.push(provider);
    }

    /// Appends one provider and returns the stack.
    pub fn with_provider(mut self, provider: Arc<dyn LibraryProvider>) -> Self {
        self.push(provider);
        self
    }

    /// Returns provider identities in dispatch order.
    pub fn provider_ids(&self) -> impl Iterator<Item = &str> {
        self.providers.iter().map(|provider| provider.provider_id())
    }

    fn provider_for(&self, capability: LibraryCapability) -> LibraryResult<&dyn LibraryProvider> {
        self.providers
            .iter()
            .find(|provider| provider.capabilities().contains(&capability))
            .map(AsRef::as_ref)
            .ok_or(LibraryError::UnsupportedCapability(capability))
    }
}

impl LibraryProvider for ProviderStack {
    fn provider_id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> BTreeSet<LibraryCapability> {
        self.providers
            .iter()
            .flat_map(|provider| provider.capabilities())
            .collect()
    }

    fn catalog(&self, library: &LibraryId) -> LibraryResult<LibraryCatalog> {
        self.provider_for(LibraryCapability::Catalog)?.catalog(library)
    }

    fn list(&self, library: &LibraryId, path: &str) -> LibraryResult<Vec<KnowledgeNode>> {
        self.provider_for(LibraryCapability::List)?.list(library, path)
    }

    fn read(&self, uri: &KnowledgeUri) -> LibraryResult<String> {
        self.provider_for(LibraryCapability::Read)?.read(uri)
    }

    fn query(
        &self,
        library: &LibraryId,
        query: &LibraryQuery,
    ) -> LibraryResult<LibraryQueryResult> {
        self.provider_for(LibraryCapability::Query)?.query(library, query)
    }

    fn refresh(&self) -> LibraryResult<()> {
        self.provider_for(LibraryCapability::Refresh)?.refresh()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::providers::VirtualLibraryProvider;

    #[test]
    fn unions_capabilities_and_uses_declared_order() {
        let first = Arc::new(
            VirtualLibraryProvider::new("first")
                .with_content("a", "first")
                .with_catalog_entry("a", "A", "a", ["a"]),
        );
        let second = Arc::new(
            VirtualLibraryProvider::new("second")
                .with_content("a", "second")
                .with_catalog_entry("a", "A", "a", ["a"]),
        );
        let stack = ProviderStack::new("stack")
            .with_provider(first)
            .with_provider(second);
        let library = LibraryId::parse("demo").expect("library");
        let uri = KnowledgeUri::new(library, "a").expect("uri");
        assert_eq!(stack.read(&uri).expect("read"), "first");
        assert!(stack.capabilities().contains(&LibraryCapability::Query));
    }
}
