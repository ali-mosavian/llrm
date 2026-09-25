# nib-lsp

A language server for Nib (language id `nib`), speaking LSP over stdio. It
runs the compiler's own frontend, so what it reports is what `llrm-nib`
would.

## Building

```sh
cargo build --release --no-default-features --bin nib-lsp
```

This needs no external tools and builds on macOS. The default `toolchain`
feature, which bootstraps Open Watcom, jwasm, jwlink and DOSBox-X, is only
for the compilers; without it `llrm-c` reports that it was built without the
toolchain feature.

## Running

The editor starts `target/release/nib-lsp` with no arguments. Imports are
read from the directories beside the edited file, from the editor's buffer
when it has one open. Modules the compiler supplies (`std.*`, `abi.*`) are
copied under `$TMPDIR/nib-lsp/` so that a definition can open them.

## Features

- Diagnostics on open, change and save: the frontend's first error, through
  type checking. One in an imported module is shown on that module and on
  the import line that reaches it.
- Document symbols: functions (methods as `Type.name`), structs and their
  fields, enums and their variants, consts, vars, types and protocols.
- Go to definition: locals and parameters, top-level names, `alias.name`
  through imports (into `std` too), and fields and methods of a value whose
  type the checker found.
- Hover: a declaration's signature and the comment above it; a local's type.
- Completion: keywords, the file's top-level names, and after `alias.` the
  public names of that module.
