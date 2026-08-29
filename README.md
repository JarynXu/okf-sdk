# OKF SDK

`okf` is a vendor-neutral Rust SDK for loading, validating, traversing, searching, and composing Open Knowledge Format knowledge.

The SDK now includes the draft **OKF Library Extension** runtime. Libraries are independently identifiable knowledge units that can be registered, mounted, navigated, queried, refreshed, and adapted to different storage technologies without leaking storage-specific branching into runtime logic.

## Core OKF bundle APIs

The first alpha keeps the core document format small:

- knowledge is stored as Markdown files;
- optional YAML front matter carries identifiers, tags, aliases, summaries, and links;
- identifiers are deterministic and portable across operating systems;
- graph and retrieval behavior is available through library APIs rather than embedded in a CLI.

```rust
use okf::{BundleParser, SearchQuery, Validator};

fn main() -> Result<(), okf::Error> {
    let bundle = BundleParser::default().parse_dir("./knowledge")?;
    let report = Validator::default().validate(&bundle);

    if !report.is_valid() {
        for issue in report.errors() {
            eprintln!("{}: {}", issue.code, issue.message);
        }
    }

    let hits = bundle.search(&SearchQuery::new("runtime architecture").limit(5));
    for hit in hits {
        println!("{} ({})", hit.document.title(), hit.score);
    }

    Ok(())
}
```

## Library runtime

The Library runtime separates stable domain capabilities from storage adapters:

- `LibraryManifest` and `LibrarySource` describe identity and acquisition;
- `KnowledgeUri` addresses logical nodes as `okf://<library>/<path>`;
- `LibraryProvider` is the capability-oriented provider contract;
- `LibraryRegistry` is the dynamic registry and mount table;
- `LibraryCatalog` carries semantic navigation contributed by each Library;
- `LibraryQueryResult` preserves provider, strategy, evidence URIs, and provenance;
- `BundleLibraryProvider` exposes ordinary OKF bundles through the same interface;
- `VirtualLibraryProvider` proves that Library nodes do not need physical files.

```rust
use std::sync::Arc;
use okf::{
    KnowledgeUri, LibraryId, LibraryInstance, LibraryManifest, LibraryRegistry,
    VirtualLibraryProvider,
};

let id = LibraryId::parse("project-context")?;
let provider = VirtualLibraryProvider::new("project-context")
    .with_content("status/current", "revision: abc123");

let mut runtime = LibraryRegistry::new();
runtime.register(LibraryInstance::new(
    LibraryManifest::new(id.clone(), "Project Context"),
    Arc::new(provider),
))?;
runtime.mount(&id)?;

assert_eq!(
    runtime.read(&KnowledgeUri::new(id, "status/current")?)?,
    "revision: abc123"
);
# Ok::<(), okf::LibraryError>(())
```

A Library source may be local, Git-backed, or custom. Source acquisition is intentionally separate from runtime provider capabilities so future S3, HTTP, database, generated, and agent-backed implementations can plug into the same domain model.

See `docs/library.md` for the runtime architecture and `docs/format.md` for the OKF document format.

## Core modules

- `model`: bundle, document, metadata, identifiers, and references;
- `parser`: directory and single-document parsing;
- `validator`: deterministic diagnostics with machine-readable codes;
- `graph`: incoming/outgoing edges, reachability, and shortest paths;
- `retrieval`: deterministic lexical ranking and tag filters;
- `library`: Library domain model, provider contract, registry, mount table, catalog, and query envelope;
- `providers`: local OKF bundle and purely virtual reference providers.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features
cargo test --all-features
```

## Status

The core bundle format and Library Extension are alpha APIs. Compatibility is not guaranteed across alpha versions.

## License

Licensed under the Apache License, Version 2.0.
