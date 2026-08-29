//! SQLite-backed Library provider.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Mutex;

use rusqlite::{Connection, OptionalExtension, params};

use crate::library::{
    CatalogEntry, KnowledgeNode, KnowledgeNodeKind, KnowledgeUri, LibraryCapability,
    LibraryCatalog, LibraryError, LibraryId, LibraryProvider, LibraryQuery, LibraryQueryHit,
    LibraryQueryResult, LibraryResult, QueryStrategy,
};

/// Read-oriented Library provider backed by a portable SQLite database.
pub struct SqliteLibraryProvider {
    id: String,
    library: LibraryId,
    connection: Mutex<Connection>,
}

impl std::fmt::Debug for SqliteLibraryProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SqliteLibraryProvider")
            .field("id", &self.id)
            .field("library", &self.library)
            .finish_non_exhaustive()
    }
}

impl SqliteLibraryProvider {
    /// Opens an existing SQLite knowledge database.
    ///
    /// The provider expects `okf_nodes(path TEXT PRIMARY KEY, title TEXT, content TEXT NOT NULL)`.
    /// An optional `okf_catalog(id, title, description, path, terms_json)` table supplies curated
    /// semantic navigation; otherwise the provider derives a catalog from nodes.
    pub fn open(
        id: impl Into<String>,
        library: LibraryId,
        path: impl AsRef<Path>,
    ) -> LibraryResult<Self> {
        let connection = Connection::open(path).map_err(sqlite_error)?;
        let provider = Self {
            id: id.into(),
            library,
            connection: Mutex::new(connection),
        };
        provider.validate_schema()?;
        Ok(provider)
    }

    /// Creates the reference read-model schema in an existing connection.
    pub fn initialize_schema(connection: &Connection) -> LibraryResult<()> {
        connection
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS okf_nodes (\
                    path TEXT PRIMARY KEY,\
                    title TEXT,\
                    content TEXT NOT NULL\
                 );\
                 CREATE TABLE IF NOT EXISTS okf_catalog (\
                    id TEXT PRIMARY KEY,\
                    title TEXT NOT NULL,\
                    description TEXT,\
                    path TEXT NOT NULL,\
                    terms_json TEXT NOT NULL DEFAULT '[]'\
                 );",
            )
            .map_err(sqlite_error)
    }

    fn validate_schema(&self) -> LibraryResult<()> {
        let connection = self.connection.lock().map_err(lock_error)?;
        let exists: Option<String> = connection
            .query_row(
                "SELECT name FROM sqlite_master WHERE type='table' AND name='okf_nodes'",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?;
        if exists.is_some() {
            Ok(())
        } else {
            Err(LibraryError::Provider(
                "SQLite provider requires an okf_nodes table".to_owned(),
            ))
        }
    }

    fn ensure_library(&self, library: &LibraryId) -> LibraryResult<()> {
        if library == &self.library {
            Ok(())
        } else {
            Err(LibraryError::Provider(format!(
                "SQLite provider '{}' belongs to Library '{}' but was called for '{}'",
                self.id, self.library, library
            )))
        }
    }
}

impl LibraryProvider for SqliteLibraryProvider {
    fn provider_id(&self) -> &str {
        &self.id
    }

    fn capabilities(&self) -> BTreeSet<LibraryCapability> {
        [
            LibraryCapability::Catalog,
            LibraryCapability::List,
            LibraryCapability::Read,
            LibraryCapability::Query,
        ]
        .into_iter()
        .collect()
    }

    fn catalog(&self, library: &LibraryId) -> LibraryResult<LibraryCatalog> {
        self.ensure_library(library)?;
        let connection = self.connection.lock().map_err(lock_error)?;
        let has_catalog: bool = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='okf_catalog')",
                [],
                |row| row.get(0),
            )
            .map_err(sqlite_error)?;

        let entries = if has_catalog {
            let mut statement = connection
                .prepare(
                    "SELECT id, title, description, path, terms_json FROM okf_catalog ORDER BY id",
                )
                .map_err(sqlite_error)?;
            let rows = statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })
                .map_err(sqlite_error)?;
            let mut entries = Vec::new();
            for row in rows {
                let (id, title, description, path, terms_json) = row.map_err(sqlite_error)?;
                let terms = serde_json::from_str::<BTreeSet<String>>(&terms_json)
                    .map_err(|error| LibraryError::Provider(format!("invalid catalog terms_json: {error}")))?;
                entries.push(CatalogEntry {
                    id,
                    title,
                    description,
                    uri: KnowledgeUri::new(library.clone(), path)?,
                    terms,
                });
            }
            entries
        } else {
            let mut statement = connection
                .prepare("SELECT path, COALESCE(title, path) FROM okf_nodes ORDER BY path")
                .map_err(sqlite_error)?;
            let rows = statement
                .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
                .map_err(sqlite_error)?;
            let mut entries = Vec::new();
            for row in rows {
                let (path, title) = row.map_err(sqlite_error)?;
                entries.push(CatalogEntry {
                    id: path.clone(),
                    title,
                    description: None,
                    uri: KnowledgeUri::new(library.clone(), path)?,
                    terms: BTreeSet::new(),
                });
            }
            entries
        };

        Ok(LibraryCatalog {
            library: library.clone(),
            entries,
        })
    }

    fn list(&self, library: &LibraryId, path: &str) -> LibraryResult<Vec<KnowledgeNode>> {
        self.ensure_library(library)?;
        let base = path.trim_matches('/');
        let prefix = if base.is_empty() {
            String::new()
        } else {
            format!("{base}/")
        };
        let like = format!("{prefix}%");
        let connection = self.connection.lock().map_err(lock_error)?;
        let mut statement = connection
            .prepare("SELECT path FROM okf_nodes WHERE path LIKE ?1 ORDER BY path")
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map([like], |row| row.get::<_, String>(0))
            .map_err(sqlite_error)?;
        let mut children = BTreeMap::<String, KnowledgeNodeKind>::new();
        for row in rows {
            let stored = row.map_err(sqlite_error)?;
            let Some(remainder) = stored.strip_prefix(&prefix) else {
                continue;
            };
            if remainder.is_empty() {
                continue;
            }
            let (name, nested) = remainder
                .split_once('/')
                .map_or((remainder, false), |(name, _)| (name, true));
            let child = if prefix.is_empty() {
                name.to_owned()
            } else {
                format!("{prefix}{name}")
            };
            let kind = if nested {
                KnowledgeNodeKind::Directory
            } else {
                KnowledgeNodeKind::Content
            };
            children
                .entry(child)
                .and_modify(|current| {
                    if kind == KnowledgeNodeKind::Directory {
                        *current = kind;
                    }
                })
                .or_insert(kind);
        }
        children
            .into_iter()
            .map(|(path, kind)| {
                Ok(KnowledgeNode {
                    uri: KnowledgeUri::new(library.clone(), path)?,
                    kind,
                    title: None,
                    virtual_node: true,
                })
            })
            .collect()
    }

    fn read(&self, uri: &KnowledgeUri) -> LibraryResult<String> {
        self.ensure_library(uri.library())?;
        let connection = self.connection.lock().map_err(lock_error)?;
        connection
            .query_row(
                "SELECT content FROM okf_nodes WHERE path = ?1",
                [uri.path()],
                |row| row.get(0),
            )
            .optional()
            .map_err(sqlite_error)?
            .ok_or_else(|| LibraryError::NodeNotFound(uri.to_string()))
    }

    fn query(
        &self,
        library: &LibraryId,
        query: &LibraryQuery,
    ) -> LibraryResult<LibraryQueryResult> {
        self.ensure_library(library)?;
        if query.limit == 0 {
            return Ok(LibraryQueryResult {
                answer: None,
                hits: Vec::new(),
                provider: self.id.clone(),
                strategy: QueryStrategy::Lexical,
                provenance: BTreeMap::new(),
            });
        }
        let pattern = format!("%{}%", query.text.trim());
        let limit = i64::try_from(query.limit).unwrap_or(i64::MAX);
        let connection = self.connection.lock().map_err(lock_error)?;
        let mut statement = connection
            .prepare(
                "SELECT path, title, content FROM okf_nodes \
                 WHERE path LIKE ?1 OR COALESCE(title, '') LIKE ?1 OR content LIKE ?1 \
                 ORDER BY CASE WHEN path LIKE ?1 THEN 0 WHEN COALESCE(title, '') LIKE ?1 THEN 1 ELSE 2 END, path \
                 LIMIT ?2",
            )
            .map_err(sqlite_error)?;
        let rows = statement
            .query_map(params![pattern, limit], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .map_err(sqlite_error)?;
        let mut hits = Vec::new();
        for row in rows {
            let (path, title, content) = row.map_err(sqlite_error)?;
            hits.push(LibraryQueryHit {
                uri: KnowledgeUri::new(library.clone(), path)?,
                title,
                snippet: Some(truncate(&content, 180)),
                score: None,
                metadata: BTreeMap::new(),
            });
        }
        Ok(LibraryQueryResult {
            answer: None,
            hits,
            provider: self.id.clone(),
            strategy: QueryStrategy::Lexical,
            provenance: BTreeMap::new(),
        })
    }
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut characters = value.chars();
    let prefix = characters.by_ref().take(max_chars).collect::<String>();
    if characters.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}

fn sqlite_error(error: impl std::fmt::Display) -> LibraryError {
    LibraryError::Provider(format!("SQLite provider error: {error}"))
}

fn lock_error<T>(error: std::sync::PoisonError<T>) -> LibraryError {
    LibraryError::Provider(format!("SQLite provider lock poisoned: {error}"))
}
