# Zed extension for Nib

Highlighting, outline, brackets and indentation come from the grammar in
`editors/tree-sitter-nib`. Diagnostics, symbols, definitions, hover and
completion come from `nib-lsp` (see `docs/lsp.md`).

The extension downloads the server from the GitHub release tagged
`nib-lsp-v<version>`, where the version is the extension's. Pushing that tag
builds the release (`.github/workflows/nib-lsp-release.yml`); bump the version
in `extension.toml` and `Cargo.toml` together.

To run a local build instead:

```json
"lsp": { "nib-lsp": { "binary": { "path": "/path/to/nib-lsp" } } }
```

Install with Zed's `zed: install dev extension` on this directory. Zed
compiles the extension with rustup's stable toolchain and the
`wasm32-wasip2` target.
