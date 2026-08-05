# OKF alpha format

An OKF bundle is a directory tree containing UTF-8 Markdown files. Other files are ignored by
default.

## Document identifiers

A document identifier is either supplied by the `id` front-matter field or derived from the
relative path with the final extension removed.

Examples:

| Path | Derived identifier |
| --- | --- |
| `concepts/sidecar.md` | `concepts/sidecar` |
| `architecture/index.md` | `architecture` |
| `index.md` | `index` |

Identifiers use `/` as the separator and may contain Unicode alphanumeric characters plus
`-`, `_`, and `.`. Empty path segments, `.` segments, and `..` segments are rejected.

## Front matter

Front matter is optional. When present, it starts on the first line with `---` and ends with
`---` or `...` on its own line.

```yaml
id: architecture/runtime
title: Runtime architecture
summary: Coordinates workflow execution and tool access.
tags:
  - architecture
  - runtime
aliases:
  - runtime
links:
  - target: concepts/sidecar
    relation: depends-on
owner: platform-team
```

Recognized fields:

- `id`: stable document identifier;
- `title`: display title;
- `summary`: short description used by retrieval;
- `tags`: deterministic set of labels;
- `aliases`: alternative identifier-like names;
- `links` or `references`: directed links to other documents.

A link can use object form:

```yaml
links:
  - target: concepts/sidecar
    relation: depends-on
```

or shorthand form, which uses the `related` relation:

```yaml
links:
  - concepts/sidecar
```

Unknown front-matter fields are preserved in `Metadata::extra` as JSON-compatible values.

## Title inference

When `title` is absent, the parser uses the first level-one Markdown heading. If no such heading
exists, it uses the final identifier segment.

## Validation

The alpha validator checks:

- empty bundles;
- missing titles;
- invalid and conflicting aliases;
- unresolved and self-referential links;
- empty relation names;
- documents with no incoming or outgoing links when orphan warnings are enabled.

Validation issue codes are stable within the `0.1` line and are intended for CLI and editor
integrations.
