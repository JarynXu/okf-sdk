//! Error types returned by the SDK.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Result alias used by fallible SDK operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors produced while loading or constructing an OKF bundle.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// A filesystem operation failed.
    #[error("failed to access {path:?}: {source}")]
    Io {
        /// Path being accessed.
        path: PathBuf,
        /// Original I/O error.
        #[source]
        source: io::Error,
    },

    /// Walking a bundle directory failed.
    #[error("failed to walk bundle at {path:?}: {source}")]
    Walk {
        /// Best-effort path associated with the traversal error.
        path: Option<PathBuf>,
        /// Original traversal error.
        #[source]
        source: walkdir::Error,
    },

    /// YAML front matter could not be decoded.
    #[error("invalid YAML front matter in {path:?}: {source}")]
    FrontMatter {
        /// Source Markdown path.
        path: PathBuf,
        /// YAML parser error.
        #[source]
        source: yaml_serde::Error,
    },

    /// A front-matter opening delimiter had no closing delimiter.
    #[error("unterminated front matter in {path:?}")]
    UnterminatedFrontMatter {
        /// Source Markdown path.
        path: PathBuf,
    },

    /// A document identifier is invalid.
    #[error(transparent)]
    InvalidDocumentId(#[from] crate::model::InvalidDocumentId),

    /// Two source files declared the same document identifier.
    #[error("duplicate document id '{id}' from {first_path:?} and {second_path:?}")]
    DuplicateDocument {
        /// Conflicting identifier.
        id: String,
        /// Source path of the document already in the bundle.
        first_path: PathBuf,
        /// Source path of the document being inserted.
        second_path: PathBuf,
    },

    /// A requested root path is not a directory.
    #[error("bundle root is not a directory: {path:?}")]
    NotDirectory {
        /// Invalid root path.
        path: PathBuf,
    },
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
