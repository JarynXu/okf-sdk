use std::fs;

use okf::{BundleLibraryProvider, BundleParser, LibraryPackageManifest, LibraryProvider};

#[test]
fn package_manifest_provides_semantic_catalog_and_query_hints() {
    let yaml = r#"
schema_version: "1"
id: mcx
name: Mission Critical Services
version: "2026.1"
catalog:
  - id: xcap
    title: XCAP interfaces
    description: XCAP documents, selectors, AUIDs, and procedures.
    path: interfaces/xcap
    terms: [xcap, auid, document-selector]
query:
  preferred: semantic
  capabilities: [lexical, semantic, agentic]
  hints:
    - Prefer interfaces/xcap for XCAP terminology.
"#;

    let manifest = LibraryPackageManifest::parse_yaml(yaml).unwrap();
    let catalog = manifest.runtime_catalog().unwrap();
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].uri.to_string(), "okf://mcx/interfaces/xcap");
    assert_eq!(manifest.query.preferred.as_deref(), Some("semantic"));
    assert!(manifest.query.capabilities.contains("agentic"));
}

#[test]
fn bundle_provider_can_use_library_owned_catalog() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir_all(directory.path().join("interfaces")).unwrap();
    fs::write(
        directory.path().join("interfaces/xcap.md"),
        "---\nid: interfaces/xcap\ntitle: XCAP\n---\n\nXCAP knowledge.\n",
    )
    .unwrap();
    fs::write(
        directory.path().join("okf-library.yaml"),
        r#"schema_version: "1"
id: mcx
name: MCX
catalog:
  - id: xcap
    title: XCAP knowledge
    path: interfaces/xcap
    terms: [xcap, auid]
query:
  preferred: lexical
"#,
    )
    .unwrap();

    let package = LibraryPackageManifest::load(directory.path()).unwrap();
    let bundle = BundleParser::default().parse_dir(directory.path()).unwrap();
    let runtime_manifest = package.runtime_manifest(None).unwrap();
    let provider = BundleLibraryProvider::new(bundle).with_catalog(package.runtime_catalog().unwrap());
    let catalog = provider.catalog(&runtime_manifest.id).unwrap();
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].id, "xcap");
}
