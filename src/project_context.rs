//! Project Context Library application profile helpers.
//!
//! These types deliberately sit above the generic Library runtime. They model repository-bound
//! freshness and incremental impact analysis without adding project-specific behavior to
//! [`crate::LibraryProvider`].

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Freshness state of a Project Context Library.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProjectContextState {
    /// No validated project-context checkpoint exists yet.
    Uninitialized,
    /// The validated repository revision matches the authoritative current revision.
    Valid,
    /// The repository moved beyond the validated revision and requires incremental revalidation.
    Dirty,
    /// The current authoritative revision cannot be established safely.
    Unknown,
}

/// Maps a project knowledge topic to repository path prefixes that can invalidate it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectContextImpactRule {
    /// Stable topic identifier or canonical knowledge URI.
    pub topic: String,
    /// Repository-relative path prefixes that affect the topic.
    pub path_prefixes: Vec<String>,
}

impl ProjectContextImpactRule {
    /// Creates an impact rule.
    pub fn new(topic: impl Into<String>, path_prefixes: Vec<String>) -> Self {
        Self {
            topic: topic.into(),
            path_prefixes,
        }
    }
}

/// Evaluated repository-bound state used by session recovery and incremental maintenance.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProjectContextStatus {
    /// Project identity chosen by the profile owner.
    pub project: String,
    /// Last repository revision for which the context was fully validated.
    pub validated_revision: Option<String>,
    /// Authoritative current repository revision when it can be established.
    pub current_revision: Option<String>,
    /// Derived freshness state.
    pub state: ProjectContextState,
    /// Repository-relative changed paths since the validated revision.
    pub changed_paths: Vec<String>,
    /// Knowledge topics invalidated or requiring revalidation.
    pub impacted_topics: Vec<String>,
}

impl ProjectContextStatus {
    /// Evaluates profile freshness and affected knowledge topics.
    pub fn evaluate(
        project: impl Into<String>,
        validated_revision: Option<String>,
        current_revision: Option<String>,
        changed_paths: Vec<String>,
        rules: &[ProjectContextImpactRule],
    ) -> Self {
        let state = match (&validated_revision, &current_revision) {
            (None, _) => ProjectContextState::Uninitialized,
            (Some(_), None) => ProjectContextState::Unknown,
            (Some(validated), Some(current)) if validated == current => ProjectContextState::Valid,
            (Some(_), Some(_)) => ProjectContextState::Dirty,
        };
        let impacted_topics = impacted_topics(&changed_paths, rules);
        Self {
            project: project.into(),
            validated_revision,
            current_revision,
            state,
            changed_paths,
            impacted_topics,
        }
    }
}

/// Computes deterministic topic invalidation from changed repository paths.
pub fn impacted_topics(
    changed_paths: &[String],
    rules: &[ProjectContextImpactRule],
) -> Vec<String> {
    let mut topics = BTreeSet::new();
    for rule in rules {
        if rule.path_prefixes.is_empty()
            || changed_paths.iter().any(|path| {
                rule.path_prefixes
                    .iter()
                    .any(|prefix| path_matches_prefix(path, prefix))
            })
        {
            topics.insert(rule.topic.clone());
        }
    }
    topics.into_iter().collect()
}

fn path_matches_prefix(path: &str, prefix: &str) -> bool {
    let path = normalize_repo_path(path);
    let prefix = normalize_repo_path(prefix);
    prefix.is_empty()
        || path == prefix
        || path
            .strip_prefix(&prefix)
            .is_some_and(|remainder| remainder.starts_with('/'))
}

fn normalize_repo_path(value: &str) -> String {
    value
        .trim()
        .trim_matches('/')
        .replace('\\', "/")
        .trim_start_matches("./")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> Vec<ProjectContextImpactRule> {
        vec![
            ProjectContextImpactRule::new(
                "okf://project/current/architecture",
                vec!["src".to_owned()],
            ),
            ProjectContextImpactRule::new(
                "okf://project/current/ci",
                vec![".github/workflows".to_owned()],
            ),
        ]
    }

    #[test]
    fn classifies_all_recovery_states() {
        assert_eq!(
            ProjectContextStatus::evaluate("p", None, Some("a".into()), vec![], &[]).state,
            ProjectContextState::Uninitialized
        );
        assert_eq!(
            ProjectContextStatus::evaluate(
                "p",
                Some("a".into()),
                Some("a".into()),
                vec![],
                &[]
            )
            .state,
            ProjectContextState::Valid
        );
        assert_eq!(
            ProjectContextStatus::evaluate(
                "p",
                Some("a".into()),
                Some("b".into()),
                vec![],
                &[]
            )
            .state,
            ProjectContextState::Dirty
        );
        assert_eq!(
            ProjectContextStatus::evaluate("p", Some("a".into()), None, vec![], &[]).state,
            ProjectContextState::Unknown
        );
    }

    #[test]
    fn derives_impacted_topics_from_changed_paths() {
        let status = ProjectContextStatus::evaluate(
            "p",
            Some("a".into()),
            Some("b".into()),
            vec!["src/library.rs".into(), ".github/workflows/ci.yml".into()],
            &rules(),
        );
        assert_eq!(
            status.impacted_topics,
            vec![
                "okf://project/current/architecture",
                "okf://project/current/ci"
            ]
        );
    }

    #[test]
    fn path_prefix_matching_respects_segment_boundaries() {
        assert!(path_matches_prefix("src/lib.rs", "src"));
        assert!(path_matches_prefix("src", "src"));
        assert!(!path_matches_prefix("src2/lib.rs", "src"));
    }
}
