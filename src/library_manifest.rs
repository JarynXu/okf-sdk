//! Portable `okf-library.yaml` package manifest.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::library::{CatalogEntry, KnowledgeUri, LibraryCatalog, LibraryId, LibraryManifest, LibrarySource};

/// Canonical Library package manifest filename.
pub const LIBRARY_MANIFEST_FILENAME: &str = "okf-library.yaml";

/// Errors produced while loading a Library package manifest.
#[derive(Debug, Error)]
pub enum LibraryManifestError {
    /// Manifest file could not be read.
    #[error("failed to read Library manifest {path}: {source}")]
    Io {
        /// Manifest path.
        path: String,
        /// I/O error.
        source: std::io::Error,
    },
    /// Manifest YAML is invalid.
    #[error("failed to parse Library manifest {path}: {message}")]
    Parse {
        /// Manifest path.
        path: String,
        /// Parser diagnostic.
        message: String,
    },
    /// Manifest declares an unsupported schema version.
    #[error("unsupported Library manifest schema version: {0}")]
    UnsupportedSchema(String),
    /// Manifest contains an invalid Library identity or knowledge path.
    #[error("invalid Library manifest: {0}")]
    Invalid(String),
}

/// Portable package declaration read from `okf-library.yaml`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibraryPackageManifest {
    /// Manifest schema version. The first stable draft is `1`.
    pub schema_version: String,
    /// Stable Library identifier.
    pub id: String,
    /// Human-readable Library name.
    pub name: String,
    /// Optional knowledge-package version.
    #[serde(default)]
    pub version: Option<String>,
    /// Semantic navigation entries owned by this Library.
    #[serde(default)]
    pub catalog: Vec<LibraryCatalogDeclaration>,
    /// Optional retrieval guidance for hosts and query providers.
    #[serde(default)]
    pub query: LibraryQueryDeclaration,
}

impl LibraryPackageManifest {
    /// Parses a manifest from YAML text.
    pub fn parse_yaml(source: &str) -> Result<Self, LibraryManifestError> {
        let manifest = yaml_serde::from_str::<Self>(source).map_err(|error| LibraryManifestError::Parse {
            path: LIBRARY_MANIFEST_FILENAME.to_owned(),
            message: error.to_string(),
        })?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Loads the canonical manifest from a Library root directory.
    pub fn load(root: impl AsRef<Path>) -> Result<Self, LibraryManifestError> {
        let path = root.as_ref().join(LIBRARY_MANIFEST_FILENAME);
        let source = fs::read_to_string(&path).map_err(|source| LibraryManifestError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let manifest = yaml_serde::from_str::<Self>(&source).map_err(|error| LibraryManifestError::Parse {
            path: path.display().to_string(),
            message: error.to_string(),
        })?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// Returns whether a canonical Library manifest exists under `root`.
    pub fn exists(root: impl AsRef<Path>) -> bool {
        root.as_ref().join(LIBRARY_MANIFEST_FILENAME).is_file()
    }

    /// Validates portable fields without accessing provider state.
    pub fn validate(&self) -> Result<(), LibraryManifestError> {
        if self.schema_version != "1" {
            return Err(LibraryManifestError::UnsupportedSchema(self.schema_version.clone()));
        }
        let id = LibraryId::parse(self.id.clone())
            .map_err(|error| LibraryManifestError::Invalid(error.to_string()))?;
        for entry in &self.catalog {
            KnowledgeUri::new(id.clone(), &entry.path)
                .map_err(|error| LibraryManifestError::Invalid(error.to_string()))?;
            if entry.id.trim().is_empty() || entry.title.trim().is_empty() {
                return Err(LibraryManifestError::Invalid(
                    "catalog entry id and title must be non-empty".to_owned(),
                ));
            }
        }
        Ok(())
    }

    /// Converts package identity fields into a resolved runtime manifest.
    pub fn runtime_manifest(
        &self,
        source: Option<LibrarySource>,
    ) -> Result<LibraryManifest, LibraryManifestError> {
        let mut manifest = LibraryManifest::new(
            LibraryId::parse(self.id.clone())
                .map_err(|error| LibraryManifestError::Invalid(error.to_string()))?,
            self.name.clone(),
        );
        manifest.version = self.version.clone();
        manifest.source = source;
        Ok(manifest)
    }

    /// Resolves semantic catalog declarations into canonical knowledge URIs.
    pub fn runtime_catalog(&self) -> Result<LibraryCatalog, LibraryManifestError> {
        let id = LibraryId::parse(self.id.clone())
            .map_err(|error| LibraryManifestError::Invalid(error.to_string()))?;
        let entries = self
            .catalog
            .iter()
            .map(|entry| {
                Ok(CatalogEntry {
                    id: entry.id.clone(),
                    title: entry.title.clone(),
                    description: entry.description.clone(),
                    uri: KnowledgeUri::new(id.clone(), &entry.path)
                        .map_err(|error| LibraryManifestError::Invalid(error.to_string()))?,
                    terms: entry.terms.clone(),
                })
            })
            .collect::<Result<Vec<_>, LibraryManifestError>>()?;
        Ok(LibraryCatalog { library: id, entries })
    }
}

/// One semantic topic declaration inside a package manifest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibraryCatalogDeclaration {
    /// Stable topic identifier.
    pub id: String,
    /// Display title.
    pub title: String,
    /// Optional explanation of the topic scope.
    #[serde(default)]
    pub description: Option<String>,
    /// Logical path within the Library namespace.
    pub path: String,
    /// Aliases, vocabulary, and routing terms.
    #[serde(default)]
    pub terms: BTreeSet<String>,
}

/// Retrieval guidance declared by a Library package.
///
/// This declaration does not execute code. It tells the host and registered query providers which
/// retrieval modes the Library expects to support and which mode should be preferred.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LibraryQueryDeclaration {
    /// Preferred query mode, such as `lexical`, `semantic`, `graph`, or `agentic`.
    #[serde(default)]
    pub preferred: Option<String>,
    /// Query modes supported by the Library deployment when corresponding providers are present.
    #[serde(default)]
    pub capabilities: BTreeSet<String>,
    /// Domain-specific routing/search hints for a query provider.
    #[serde(default)]
    pub hints: Vec<String>,
}
