//! Vector semantic query provider.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::library::{
    KnowledgeUri, LibraryError, LibraryId, LibraryQuery, LibraryQueryHit, LibraryQueryResult,
    LibraryResult, QueryStrategy,
};
use crate::library_providers::QueryProvider;

/// Produces embedding vectors for semantic retrieval.
pub trait EmbeddingProvider: Send + Sync {
    /// Stable embedding-provider identity.
    fn provider_id(&self) -> &str;

    /// Embeds one query or document string.
    fn embed(&self, text: &str) -> LibraryResult<Vec<f32>>;
}

/// Adapts a thread-safe Rust closure into an [`EmbeddingProvider`].
pub struct FnEmbeddingProvider<F> {
    id: String,
    embed: F,
}

impl<F> std::fmt::Debug for FnEmbeddingProvider<F> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FnEmbeddingProvider")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl<F> FnEmbeddingProvider<F> {
    /// Creates a closure-backed embedding adapter.
    pub fn new(id: impl Into<String>, embed: F) -> Self {
        Self {
            id: id.into(),
            embed,
        }
    }
}

impl<F> EmbeddingProvider for FnEmbeddingProvider<F>
where
    F: Fn(&str) -> LibraryResult<Vec<f32>> + Send + Sync,
{
    fn provider_id(&self) -> &str {
        &self.id
    }

    fn embed(&self, text: &str) -> LibraryResult<Vec<f32>> {
        (self.embed)(text)
    }
}

/// One pre-embedded knowledge entry used by semantic retrieval.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticEntry {
    /// Canonical knowledge URI represented by this vector.
    pub uri: KnowledgeUri,
    /// Optional display title.
    pub title: Option<String>,
    /// Optional bounded evidence snippet.
    pub snippet: Option<String>,
    /// Embedding vector.
    pub vector: Vec<f32>,
    /// Provider-specific metadata preserved in hits.
    pub metadata: BTreeMap<String, String>,
}

/// Semantic query provider over a precomputed vector index.
pub struct VectorSemanticQueryProvider {
    id: String,
    embedder: Arc<dyn EmbeddingProvider>,
    entries: Vec<SemanticEntry>,
}

impl std::fmt::Debug for VectorSemanticQueryProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VectorSemanticQueryProvider")
            .field("id", &self.id)
            .field("embedder", &self.embedder.provider_id())
            .field("entries", &self.entries.len())
            .finish()
    }
}

impl VectorSemanticQueryProvider {
    /// Creates a semantic provider and validates vector dimensionality and values.
    pub fn new(
        id: impl Into<String>,
        embedder: Arc<dyn EmbeddingProvider>,
        entries: Vec<SemanticEntry>,
    ) -> LibraryResult<Self> {
        let dimension = entries.first().map(|entry| entry.vector.len());
        if dimension == Some(0) {
            return Err(LibraryError::Provider(
                "semantic index vectors must not be empty".to_owned(),
            ));
        }
        if let Some(dimension) = dimension {
            if entries.iter().any(|entry| entry.vector.len() != dimension) {
                return Err(LibraryError::Provider(
                    "semantic index contains inconsistent vector dimensions".to_owned(),
                ));
            }
        }
        if entries
            .iter()
            .flat_map(|entry| entry.vector.iter())
            .any(|value| !value.is_finite())
        {
            return Err(LibraryError::Provider(
                "semantic index contains non-finite vector values".to_owned(),
            ));
        }
        Ok(Self {
            id: id.into(),
            embedder,
            entries,
        })
    }
}

impl QueryProvider for VectorSemanticQueryProvider {
    fn provider_id(&self) -> &str {
        &self.id
    }

    fn query(
        &self,
        library: &LibraryId,
        query: &LibraryQuery,
    ) -> LibraryResult<LibraryQueryResult> {
        if query.limit == 0 || self.entries.is_empty() {
            return Ok(LibraryQueryResult {
                answer: None,
                hits: Vec::new(),
                provider: self.id.clone(),
                strategy: QueryStrategy::Semantic,
                provenance: BTreeMap::from([(
                    "embedding_provider".to_owned(),
                    self.embedder.provider_id().to_owned(),
                )]),
            });
        }
        if self.entries.iter().any(|entry| entry.uri.library() != library) {
            return Err(LibraryError::Provider(format!(
                "semantic index '{}' contains entries for another Library",
                self.id
            )));
        }
        let query_vector = self.embedder.embed(&query.text)?;
        let dimension = self.entries[0].vector.len();
        if query_vector.len() != dimension {
            return Err(LibraryError::Provider(format!(
                "embedding provider returned dimension {}, expected {}",
                query_vector.len(), dimension
            )));
        }
        if query_vector.iter().any(|value| !value.is_finite()) {
            return Err(LibraryError::Provider(
                "embedding provider returned non-finite values".to_owned(),
            ));
        }

        let mut scored = self
            .entries
            .iter()
            .map(|entry| (cosine_similarity(&query_vector, &entry.vector), entry))
            .collect::<Vec<_>>();
        scored.sort_by(|(left_score, left), (right_score, right)| {
            right_score
                .partial_cmp(left_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.uri.cmp(&right.uri))
        });

        let hits = scored
            .into_iter()
            .take(query.limit)
            .map(|(score, entry)| LibraryQueryHit {
                uri: entry.uri.clone(),
                title: entry.title.clone(),
                snippet: entry.snippet.clone(),
                score: Some(score),
                metadata: entry.metadata.clone(),
            })
            .collect();

        Ok(LibraryQueryResult {
            answer: None,
            hits,
            provider: self.id.clone(),
            strategy: QueryStrategy::Semantic,
            provenance: BTreeMap::from([
                (
                    "embedding_provider".to_owned(),
                    self.embedder.provider_id().to_owned(),
                ),
                ("similarity".to_owned(), "cosine".to_owned()),
            ]),
        })
    }
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f64 {
    let mut dot = 0.0_f64;
    let mut left_norm = 0.0_f64;
    let mut right_norm = 0.0_f64;
    for (left, right) in left.iter().zip(right) {
        let left = f64::from(*left);
        let right = f64::from(*right);
        dot += left * right;
        left_norm += left * left;
        right_norm += right * right;
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_provider_ranks_cosine_similarity() {
        let library = LibraryId::parse("demo").expect("library");
        let embedder = Arc::new(FnEmbeddingProvider::new("test", |text: &str| {
            Ok(if text.contains("alpha") {
                vec![1.0, 0.0]
            } else {
                vec![0.0, 1.0]
            })
        }));
        let provider = VectorSemanticQueryProvider::new(
            "vector",
            embedder,
            vec![
                SemanticEntry {
                    uri: KnowledgeUri::new(library.clone(), "alpha").expect("uri"),
                    title: None,
                    snippet: None,
                    vector: vec![1.0, 0.0],
                    metadata: BTreeMap::new(),
                },
                SemanticEntry {
                    uri: KnowledgeUri::new(library.clone(), "beta").expect("uri"),
                    title: None,
                    snippet: None,
                    vector: vec![0.0, 1.0],
                    metadata: BTreeMap::new(),
                },
            ],
        )
        .expect("provider");
        let result = provider
            .query(&library, &LibraryQuery::new("alpha").limit(1))
            .expect("query");
        assert_eq!(result.hits[0].uri.path(), "alpha");
    }

    #[test]
    fn semantic_provider_rejects_non_finite_index_values() {
        let library = LibraryId::parse("demo").expect("library");
        let embedder = Arc::new(FnEmbeddingProvider::new("test", |_text: &str| {
            Ok(vec![1.0])
        }));
        let result = VectorSemanticQueryProvider::new(
            "vector",
            embedder,
            vec![SemanticEntry {
                uri: KnowledgeUri::new(library, "bad").expect("uri"),
                title: None,
                snippet: None,
                vector: vec![f32::NAN],
                metadata: BTreeMap::new(),
            }],
        );
        assert!(result.is_err());
    }
}
