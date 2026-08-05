# Contributing

Contributions should preserve the SDK's vendor-neutral scope and deterministic behavior.

Before opening a pull request:

1. run `cargo fmt --all -- --check`;
2. run `cargo clippy --all-targets --all-features`;
3. run `cargo test --all-features`;
4. document user-visible behavior and add tests for parser or validation changes.

Public API changes should include a rationale and migration notes because downstream CLI, MCP,
and skill implementations are expected to depend on this crate.
