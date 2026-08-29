//! Agent-backed semantic query adapter.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::library::{
    LibraryError, LibraryId, LibraryQuery, LibraryQueryResult, LibraryResult, QueryStrategy,
};
use crate::library_providers::QueryProvider;

/// Executes a bounded agentic retrieval turn for one Library.
///
/// Implementations own model/provider credentials, tool policy, recursion limits, and prompt
/// construction. The OKF SDK only requires a deterministic request/result boundary with evidence.
pub trait AgentQueryExecutor: Send + Sync {
    /// Stable executor identity for provenance.
    fn executor_id(&self) -> &str;

    /// Executes one Library-scoped retrieval request.
    fn execute(
        &self,
        library: &LibraryId,
        query: &LibraryQuery,
    ) -> LibraryResult<LibraryQueryResult>;
}

/// Adapts an [`AgentQueryExecutor`] to the generic [`QueryProvider`] contract.
pub struct AgentQueryProvider {
    id: String,
    executor: Arc<dyn AgentQueryExecutor>,
}

impl std::fmt::Debug for AgentQueryProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AgentQueryProvider")
            .field("id", &self.id)
            .field("executor", &self.executor.executor_id())
            .finish()
    }
}

impl AgentQueryProvider {
    /// Creates an agent-backed query provider.
    pub fn new(id: impl Into<String>, executor: Arc<dyn AgentQueryExecutor>) -> Self {
        Self {
            id: id.into(),
            executor,
        }
    }
}

impl QueryProvider for AgentQueryProvider {
    fn provider_id(&self) -> &str {
        &self.id
    }

    fn query(
        &self,
        library: &LibraryId,
        query: &LibraryQuery,
    ) -> LibraryResult<LibraryQueryResult> {
        let mut result = self.executor.execute(library, query)?;
        if result.hits.iter().any(|hit| hit.uri.library() != library) {
            return Err(LibraryError::Provider(format!(
                "agent executor '{}' returned evidence outside Library '{}'",
                self.executor.executor_id(),
                library
            )));
        }
        result.provider = self.id.clone();
        result.strategy = QueryStrategy::Agentic;
        result.provenance.insert(
            "agent_executor".to_owned(),
            self.executor.executor_id().to_owned(),
        );
        if result.provenance.is_empty() {
            result.provenance = BTreeMap::new();
        }
        Ok(result)
    }
}
