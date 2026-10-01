# Contributing

Issues and pull requests are welcome.

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

Keep Expander local-first, fast, and dependency-light. New MCP tools should have a clear prompt-library use case and must not execute stored prompt text themselves.

