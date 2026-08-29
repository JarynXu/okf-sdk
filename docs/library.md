# OKF Library Runtime

The Library runtime implements the draft OKF Library Extension as a dependency-inverted knowledge host.

## Layers

The stable domain layer contains Library identity, source metadata, virtual knowledge URIs/nodes, semantic catalogs, portable queries/results, provider capabilities, and lifecycle state. Concrete local/Git/object-storage/HTTP/database/agent implementations belong outside this layer.

`LibraryRegistry` is both registry and mount table. Registration means an instance is known; mounting means it participates in the active knowledge space. Unmounting never implies deletion of installed/materialized source data.

## Provider model

`LibraryProvider` is capability-oriented. Runtime code checks declared capabilities and delegates to the provider. It does not switch on storage types.

The initial providers are:

- `BundleLibraryProvider`: adapts an already parsed OKF `Bundle`;
- `VirtualLibraryProvider`: generated/in-memory nodes with no physical files.

`LibrarySource::Git` deliberately records acquisition metadata rather than introducing Git behavior into runtime routing. A Git resolver/materializer can produce a resolved Library whose provider is then treated exactly like any other provider.

## Semantic catalog

A Library contributes its own `LibraryCatalog`. The host aggregates mounted catalogs without trying to infer domain semantics from storage paths. This lets specialized Libraries carry optimized topic maps, aliases, routing terms, and future search hints.

## Virtual namespace

Canonical logical addresses use `okf://<library-id>/<path>`. A node may correspond to Markdown, a generated runtime status value, an object in S3, a database record, or remote content. Filesystem/FUSE and MCP representations are adapters over the same logical URI.

## Query model

Queries are dispatched to the selected Library provider. Results preserve:

- strategy (`exact`, `lexical`, `semantic`, `graph`, `agentic`, or custom);
- provider identity;
- evidence URIs and snippets;
- optional answer synthesis;
- bounded provenance metadata.

Cross-Library query uses `LibraryRegistry::query_all`; higher-level routing can first narrow candidate Libraries using the global semantic catalog.

## Project Context

Project Context is intentionally not a separate runtime. It is a Library profile whose content may include architecture, decisions, history, constraints, current status, revision/freshness metadata, and dynamically generated repository state. It can therefore use the same mount, namespace, catalog, query, and provider contracts as any other Library.
