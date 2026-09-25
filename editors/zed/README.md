# Zed extension for Nib

Highlighting, outline, brackets and indentation come from the grammar in
`editors/tree-sitter-nib`. Diagnostics, symbols, definitions, hover and
completion come from `nib-lsp` (see `docs/lsp.md`).

Build the server and put it on `PATH`:

```text
cargo build --release --no-default-features --bin nib-lsp
ln -s "$PWD/target/release/nib-lsp" ~/.cargo/bin/nib-lsp
```

Or name it in Zed's settings instead:

```json
"lsp": { "nib-lsp": { "binary": { "path": "/path/to/nib-lsp" } } }
```

Install with Zed's `zed: install dev extension` on this directory. Zed
compiles the extension with rustup's stable toolchain and the
`wasm32-wasip2` target.
