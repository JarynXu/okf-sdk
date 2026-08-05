use std::fs;

use okf::{BundleParser, Error, parse_document};
use serde_json::json;
use tempfile::tempdir;

#[test]
fn parses_directory_front_matter_and_path_ids() -> okf::Result<()> {
    let directory = tempdir().expect("temporary directory");
    fs::create_dir_all(directory.path().join("concepts"))
        .expect("nested directory should be created");
    fs::write(
        directory.path().join("concepts/sidecar.md"),
        r#"---
title: Sidecar
tags: [architecture, ai]
aliases: [sidecar]
owner: platform
links:
  - target: runtime
    relation: used-by
---

A sidecar supplies tools to an AI runtime.
"#,
    )
    .expect("document should be written");
    fs::write(
        directory.path().join("runtime.md"),
        "# Runtime\n\nThe runtime executes workflows.\n",
    )
    .expect("document should be written");
    fs::write(directory.path().join("ignored.txt"), "ignored")
        .expect("ignored file should be written");

    let bundle = BundleParser::default().parse_dir(directory.path())?;

    assert_eq!(bundle.len(), 2);
    let sidecar = bundle.get_by_id("concepts/sidecar").expect("sidecar document");
    assert_eq!(sidecar.title(), "Sidecar");
    assert!(sidecar.metadata().tags.contains("architecture"));
    assert_eq!(sidecar.metadata().extra.get("owner"), Some(&json!("platform")));
    assert_eq!(sidecar.metadata().links[0].relation(), "used-by");
    assert_eq!(sidecar.source_path().expect("source path").to_string_lossy(), "concepts/sidecar.md");

    Ok(())
}

#[test]
fn supports_reference_shorthand() -> okf::Result<()> {
    let document = parse_document(
        r#"---
id: source
links: [target]
---
Body
"#,
        "source.md",
    )?;

    assert_eq!(document.metadata().links[0].target().as_str(), "target");
    assert_eq!(document.metadata().links[0].relation(), "related");
    Ok(())
}

#[test]
fn reports_duplicate_explicit_ids() {
    let directory = tempdir().expect("temporary directory");
    fs::write(
        directory.path().join("one.md"),
        "---\nid: duplicate\n---\nOne",
    )
    .expect("first document should be written");
    fs::write(
        directory.path().join("two.md"),
        "---\nid: duplicate\n---\nTwo",
    )
    .expect("second document should be written");

    let error = BundleParser::default()
        .parse_dir(directory.path())
        .expect_err("duplicate ids should fail");

    assert!(matches!(error, Error::DuplicateDocument { .. }));
}

#[test]
fn reports_unterminated_front_matter() {
    let error = parse_document("---\ntitle: Broken\n", "broken.md")
        .expect_err("unterminated front matter should fail");

    assert!(matches!(error, Error::UnterminatedFrontMatter { .. }));
}
