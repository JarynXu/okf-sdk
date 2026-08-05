use std::collections::BTreeMap;

/// Extensible OKF metadata container.
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    fields: BTreeMap<String, String>,
}

impl Metadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.fields.insert(key.into(), value.into());
    }
}
