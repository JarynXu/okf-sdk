use std::fs;
use std::sync::Arc;

use okf::{
    BundleLibraryProvider, BundleParser, KnowledgeNodeKind, KnowledgeUri, LibraryId,
    LibraryInstance, LibraryManifest, LibraryQuery, LibraryRegistry, LibrarySource,
    VirtualLibraryProvider,
};

#[test]
fn virtual_library_mounts_without_physical_files() {
    let id = LibraryId::parse("project-context").unwrap();
    let provider = VirtualLibraryProvider::new("project-context-test")
        .with_content("status/current", "revision: abc123")
        .with_content("architecture/runtime", "The runtime owns orchestration.")
        .with_catalog_entry(
            "architecture",
            "Architecture",
            "architecture/runtime",
            ["runtime", "architecture"],
        );
    let instance = LibraryInstance::new(
        LibraryManifest::new(id.clone(), "Project Context"),
        Arc::new(provider),
    );

    let mut registry = LibraryRegistry::new();
    registry.register(instance).unwrap();
    assert!(!registry.is_mounted(&id));
    registry.mount(&id).unwrap();

    let content = registry
        .read(&KnowledgeUri::new(id.clone(), "status/current").unwrap())
        .unwrap();
    assert_eq!(content, "revision: abc123");

    let root = registry.list(&id, "").unwrap();
    assert!(root.iter().all(|node| node.virtual_node));
    assert!(root
        .iter()
        .any(|node| node.kind == KnowledgeNodeKind::Directory));

    let catalog = registry.catalog(&id).unwrap();
    assert_eq!(catalog.entries.len(), 1);

    let query = registry
        .query(&id, &LibraryQuery::new("runtime"))
        .unwrap();
    assert_eq!(query.hits.len(), 1);
    assert_eq!(query.hits[0].uri.path(), "architecture/runtime");

    registry.unmount(&id).unwrap();
    assert!(registry
        .read(&KnowledgeUri::new(id, "status/current").unwrap())
        .is_err());
}

#[test]
fn okf_bundle_uses_the_same_runtime_contract() {
    let directory = tempfile::tempdir().unwrap();
    fs::create_dir_all(directory.path().join("architecture")).unwrap();
    fs::write(
        directory.path().join("architecture/runtime.md"),
        "---\nid: architecture/runtime\ntitle: Runtime architecture\ntags: [architecture, runtime]\n---\n\nThe runtime coordinates tools and workflows.\n",
    )
    .unwrap();

    let bundle = BundleParser::default().parse_dir(directory.path()).unwrap();
    let id = LibraryId::parse("local-docs").unwrap();
    let mut manifest = LibraryManifest::new(id.clone(), "Local docs");
    manifest.source = Some(LibrarySource::Local {
        path: directory.path().to_path_buf(),
    });
    let instance = LibraryInstance::new(manifest, Arc::new(BundleLibraryProvider::new(bundle)));

    let mut registry = LibraryRegistry::new();
    registry.register(instance).unwrap();
    registry.mount(&id).unwrap();

    let text = registry
        .read(&KnowledgeUri::new(id.clone(), "architecture/runtime").unwrap())
        .unwrap();
    assert!(text.contains("coordinates tools"));
    assert_eq!(registry.catalog(&id).unwrap().entries.len(), 1);
    assert_eq!(registry.query(&id, &LibraryQuery::new("runtime")).unwrap().hits.len(), 1);
}

#[test]
fn git_source_is_portable_metadata_not_a_runtime_branch() {
    let source = LibrarySource::Git {
        repository: "https://example.com/acme/mcx-library.git".to_owned(),
        reference: Some("v1.2.0".to_owned()),
    };
    let json = serde_json::to_string(&source).unwrap();
    let decoded: LibrarySource = serde_json::from_str(&json).unwrap();
    assert_eq!(source, decoded);
}

#[test]
fn global_catalog_preserves_library_boundaries() {
    let first = LibraryId::parse("first").unwrap();
    let second = LibraryId::parse("second").unwrap();
    let mut registry = LibraryRegistry::new();

    for id in [&first, &second] {
        let provider = VirtualLibraryProvider::new(format!("provider-{id}"))
            .with_content("topic", format!("knowledge from {id}"))
            .with_catalog_entry("topic", "Topic", "topic", [id.as_str()]);
        registry
            .register(LibraryInstance::new(
                LibraryManifest::new(id.clone(), format!("Library {id}")),
                Arc::new(provider),
            ))
            .unwrap();
        registry.mount(id).unwrap();
    }

    let catalogs = registry.global_catalog().unwrap();
    assert_eq!(catalogs.len(), 2);
    assert_ne!(catalogs[0].library, catalogs[1].library);
}

#[test]
fn canonical_uri_round_trips() {
    let uri = KnowledgeUri::parse("okf://mcx/interfaces/xcap").unwrap();
    assert_eq!(uri.library().as_str(), "mcx");
    assert_eq!(uri.path(), "interfaces/xcap");
    assert_eq!(uri.to_string(), "okf://mcx/interfaces/xcap");
}
