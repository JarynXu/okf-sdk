//! S3-compatible content provider.

use std::collections::BTreeMap;

use s3::bucket::Bucket;

use crate::library::{
    KnowledgeNode, KnowledgeNodeKind, KnowledgeUri, LibraryError, LibraryId, LibraryResult,
};
use crate::library_providers::ContentProvider;

/// Read-only Library content provider backed by AWS S3 or an S3-compatible object store.
#[derive(Clone, Debug)]
pub struct S3ContentProvider {
    id: String,
    bucket: Box<Bucket>,
    prefix: String,
}

impl S3ContentProvider {
    /// Creates a provider from an already authenticated/configured S3 bucket.
    ///
    /// Credential and endpoint policy deliberately remain deployment concerns outside the Library
    /// domain model.
    pub fn new(id: impl Into<String>, bucket: Box<Bucket>, prefix: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            bucket,
            prefix: normalize_prefix(prefix.into()),
        }
    }

    fn key(&self, path: &str) -> String {
        let path = path.trim_matches('/');
        match (self.prefix.is_empty(), path.is_empty()) {
            (true, _) => path.to_owned(),
            (false, true) => self.prefix.trim_end_matches('/').to_owned(),
            (false, false) => format!("{}{}", self.prefix, path),
        }
    }

    fn list_prefix(&self, path: &str) -> String {
        let key = self.key(path);
        if key.is_empty() || key.ends_with('/') {
            key
        } else {
            format!("{key}/")
        }
    }
}

impl ContentProvider for S3ContentProvider {
    fn provider_id(&self) -> &str {
        &self.id
    }

    fn list(&self, library: &LibraryId, path: &str) -> LibraryResult<Vec<KnowledgeNode>> {
        let prefix = self.list_prefix(path);
        let pages = self
            .bucket
            .list(prefix.clone(), Some("/".to_owned()))
            .map_err(s3_error)?;
        let base = path.trim_matches('/');
        let mut children = BTreeMap::<String, KnowledgeNodeKind>::new();

        for page in pages {
            if let Some(common_prefixes) = page.common_prefixes {
                for common in common_prefixes {
                    let logical = strip_storage_prefix(&common.prefix, &self.prefix)
                        .trim_end_matches('/')
                        .to_owned();
                    if logical.is_empty() || logical == base {
                        continue;
                    }
                    children.insert(logical, KnowledgeNodeKind::Directory);
                }
            }
            for object in page.contents {
                let logical = strip_storage_prefix(&object.key, &self.prefix).to_owned();
                if logical.is_empty() || logical.ends_with('/') || logical == base {
                    continue;
                }
                let remainder = if base.is_empty() {
                    logical.as_str()
                } else if let Some(remainder) = logical.strip_prefix(&format!("{base}/")) {
                    remainder
                } else {
                    continue;
                };
                if remainder.contains('/') {
                    continue;
                }
                children
                    .entry(logical)
                    .or_insert(KnowledgeNodeKind::Content);
            }
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
        let response = self
            .bucket
            .get_object(self.key(uri.path()))
            .map_err(s3_error)?;
        String::from_utf8(response.as_slice().to_vec()).map_err(|error| {
            LibraryError::Provider(format!(
                "S3 object '{}' is not UTF-8 knowledge content: {error}",
                uri
            ))
        })
    }
}

fn normalize_prefix(prefix: String) -> String {
    let prefix = prefix.trim_matches('/');
    if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}/")
    }
}

fn strip_storage_prefix<'a>(key: &'a str, prefix: &str) -> &'a str {
    key.strip_prefix(prefix).unwrap_or(key)
}

fn s3_error(error: impl std::fmt::Display) -> LibraryError {
    LibraryError::Provider(format!("S3 provider error: {error}"))
}
