use okf::{
    Bundle, Document, DocumentId, KnowledgeGraph, Metadata, Reference, Severity, Validator,
};

fn document(id: &str, title: &str, metadata: Metadata) -> Document {
    Document::new(
        DocumentId::new(id).expect("valid test identifier"),
        title,
        format!("Body for {id}"),
        metadata,
    )
}

#[test]
fn validates_references_aliases_and_orphans() -> okf::Result<()> {
    let mut source_metadata = Metadata::default();
    source_metadata
        .links
        .push(Reference::related(DocumentId::new("missing")?));
    source_metadata.aliases.insert("shared".to_owned());

    let mut target_metadata = Metadata::default();
    target_metadata.aliases.insert("shared".to_owned());

    let mut bundle = Bundle::default();
    bundle.insert(document("source", "Source", source_metadata))?;
    bundle.insert(document("target", "Target", target_metadata))?;

    let report = Validator::default().validate(&bundle);
    let codes = report
        .issues()
        .iter()
        .map(|issue| issue.code.as_str())
        .collect::<Vec<_>>();

    assert!(!report.is_valid());
    assert!(codes.contains(&"OKF104"));
    assert!(codes.contains(&"OKF203"));
    assert!(codes.contains(&"OKF301"));
    assert!(report.errors().all(|issue| issue.severity == Severity::Error));
    Ok(())
}

#[test]
fn resolves_aliases_in_graph_and_finds_shortest_path() -> okf::Result<()> {
    let mut a_metadata = Metadata::default();
    a_metadata
        .links
        .push(Reference::new(DocumentId::new("middle")?, "next"));

    let mut b_metadata = Metadata::default();
    b_metadata.aliases.insert("middle".to_owned());
    b_metadata
        .links
        .push(Reference::new(DocumentId::new("c")?, "next"));

    let mut bundle = Bundle::default();
    bundle.insert(document("a", "A", a_metadata))?;
    bundle.insert(document("b", "B", b_metadata))?;
    bundle.insert(document("c", "C", Metadata::default()))?;

    let graph = KnowledgeGraph::from_bundle(&bundle);
    let path = graph
        .shortest_path(&DocumentId::new("a")?, &DocumentId::new("c")?)
        .expect("path should exist");
    let path = path.iter().map(DocumentId::as_str).collect::<Vec<_>>();

    assert_eq!(path, vec!["a", "b", "c"]);
    assert_eq!(graph.edge_count(), 2);
    assert_eq!(
        graph
            .relations(&DocumentId::new("a")?, &DocumentId::new("b")?)
            .collect::<Vec<_>>(),
        vec!["next"]
    );
    Ok(())
}
