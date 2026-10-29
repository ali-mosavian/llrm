# nib-lsp

Audience: editor users and contributors. `nib-lsp` is a language server for
Nib (language id `nib`), speaking LSP over stdio. It runs the compiler's own
frontend, so what it reports is what `llrm-nib` would.

## Building

```sh
cargo build --release --no-default-features --bin nib-lsp
```

This needs no external tools and builds on macOS. The default `toolchain`
feature, which bootstraps Open Watcom, jwasm, jwlink and DOSBox-X, is only
for the compilers; without it `llrm-cc` reports that it was built without the
toolchain feature.

## Running

The editor starts `target/release/nib-lsp` with no arguments. Imports are
read from the directories beside the edited file, from the editor's buffer
when it has one open. Modules the compiler supplies (`std.*`, `abi.*`) are
copied under `$TMPDIR/nib-lsp/` so that a definition can open them.

## The project's target

The server checks for real mode (`-m16`) unless the editor sends the
project's mode in `initialize`: `"initializationOptions": {"mode": 32}`, gcc's
`-m32`. That target's layout, calling conventions and OS layer (`std.os`) then
decide what is an error and what a hover or definition shows. A mode no target
has fails `initialize` (`no target for -m64`). Zed passes the
`lsp.nib-lsp.initialization_options` setting (see `editors/zed/README.md`).

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
- Completion: the locals in scope, keywords and the module's own names; after
  `alias.` the public names of that module; after any other value and `.`, the
  fields and methods of its type.
