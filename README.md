# OKF SDK

`okf` is a vendor-neutral Rust SDK for loading, validating, traversing, and searching
Open Knowledge Format bundles.

The first alpha deliberately keeps the format small:

- knowledge is stored as Markdown files;
- optional YAML front matter carries identifiers, tags, aliases, summaries, and links;
- identifiers are deterministic and portable across operating systems;
- graph and retrieval behavior is available through library APIs rather than embedded in a CLI.

## Status

This is an alpha release. The core model and issue codes are usable, but compatibility is not
yet guaranteed across alpha versions.

## Quick start

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

A document can be as small as:

```markdown
---
id: architecture/runtime
title: Runtime architecture
tags: [architecture, runtime]
links:
  - target: concepts/sidecar
    relation: depends-on
---

The runtime coordinates workflow execution and tool access.
```

When `id` is omitted, it is derived from the Markdown file's path relative to the bundle root.
See [`docs/format.md`](docs/format.md) for the complete alpha format.

## Core modules

- `model`: bundle, document, metadata, identifiers, and references;
- `parser`: directory and single-document parsing;
- `validator`: deterministic diagnostics with machine-readable codes;
- `graph`: incoming/outgoing edges, reachability, and shortest paths;
- `retrieval`: deterministic lexical ranking and tag filters.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features
cargo test --all-features
```

## License

Licensed under the Apache License, Version 2.0.
