//! Integration tests for deterministic lexical retrieval.

use okf::{Bundle, Document, DocumentId, MatchField, Metadata, SearchQuery};

fn insert(
    bundle: &mut Bundle,
    id: &str,
    title: &str,
    body: &str,
    tags: &[&str],
) -> okf::Result<()> {
    let mut metadata = Metadata::default();
    metadata
        .tags
        .extend(tags.iter().map(|tag| (*tag).to_owned()));
    bundle.insert(Document::new(DocumentId::new(id)?, title, body, metadata))
}

#[test]
fn ranks_titles_above_body_matches() -> okf::Result<()> {
    let mut bundle = Bundle::default();
    insert(
        &mut bundle,
        "architecture/runtime",
        "Runtime architecture",
        "Coordinates execution.",
        &["runtime"],
    )?;
    insert(
        &mut bundle,
        "notes/execution",
        "Execution notes",
        "A note mentioning runtime architecture in the body.",
        &["notes"],
    )?;

    let hits = bundle.search(&SearchQuery::new("runtime architecture"));

    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].document.id().as_str(), "architecture/runtime");
    assert!(hits[0].matched_fields.contains(&MatchField::Title));
    assert!(hits[0].score > hits[1].score);
    Ok(())
}

#[test]
fn applies_tag_filters_with_and_semantics() -> okf::Result<()> {
    let mut bundle = Bundle::default();
    insert(&mut bundle, "one", "One", "", &["ai", "architecture"])?;
    insert(&mut bundle, "two", "Two", "", &["ai"])?;

    let query = SearchQuery::new("")
        .with_tag("AI")
        .with_tag("architecture")
        .limit(10);
    let hits = bundle.search(&query);

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].document.id().as_str(), "one");
    Ok(())
}

#[test]
fn uses_identifier_order_to_break_score_ties() -> okf::Result<()> {
    let mut bundle = Bundle::default();
    insert(&mut bundle, "b", "Same", "", &[])?;
    insert(&mut bundle, "a", "Same", "", &[])?;

    let hits = bundle.search(&SearchQuery::new("same"));
    let ids = hits
        .iter()
        .map(|hit| hit.document.id().as_str())
        .collect::<Vec<_>>();

    assert_eq!(ids, vec!["a", "b"]);
    Ok(())
}
