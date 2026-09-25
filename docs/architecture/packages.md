# llrm packages

A package is a library of compiled modules that programs import without
recompiling: an OMF `.LIB` whose objects each carry a module's code for the
linker and its interface for the compiler. Importing costs reading interfaces
and linking; only the importing program's own functions, and the generic
instances it makes, go through the optimizer. Versions, a registry and
dependency resolution come later, on top of this format.

## What a module holds

- **Object code**, one segment per function, so the linker takes only what a
  program reaches.
- **The interface**: the public declarations and the types they reach. An
  importer checks against it and never parses the module's source.
- **Summaries** of each public function: what it does to its arguments and to
  memory, whether it returns. The optimizer reads a callee's summary where it
  would read its body, so a call into a package is no less known than a local
  one. The analysis that proves each fact writes it once, when the module is
  built.
- **Templates**: generic functions, generators and protocol defaults, as their
  checked source. They are instantiated with the importer's types, so they are
  compiled in the importer.

## Envelope

The interface is one blob per object, split across `COMENT` records right
after `THEADR`. Each record's body is:

| Offset | Size | Field |
|---|---|---|
| 0 | 1 | attribute `0xC0`: no purge, no list |
| 1 | 1 | class `0x9C` |
| 2 | 4 | tag `"LLRM"` |
| 6 | 2 | chunk index |
| 8 | 2 | chunk count |
| 10 | ≤ 1012 | payload |

The blob is the payloads in chunk order. A record is at most 1 KB.

Librarians keep the records and linkers skip them. That held for Open Watcom
`wlib` with jwlink, QuickBASIC 4.5's LIB 3.14 and LINK 3.69, and MASM 6.11's
LIB 3.20 and LINK 5.31: with 64 KB, 300 KB and 2 MB of records in one object,
every record came through the librarian byte for byte, and each linker built
the same EXE as without them.

A record holds at most 64 KB, which the chunking avoids. An OMF library holds
at most 65,535 pages; at the 512-byte pages `wlib` uses, that is 32 MB of code
and interfaces together.

## Blob

All values are little-endian. Every row is a fixed-size struct, fields at
their natural alignment, padding written as zero so that the hashes are
deterministic.

The blob starts with a 64-byte header:

| Offset | Size | Field |
|---|---|---|
| 0 | 4 | magic `"INTF"` |
| 4 | 2 | format version |
| 6 | 2 | section count |
| 8 | 16 | compiler fingerprint |
| 24 | 16 | source hash |
| 40 | 16 | interface hash |
| 56 | 4 | module name (`Str`) |
| 60 | 2 | language of the frontend that wrote it |
| 62 | 2 | reserved |

A directory of 12-byte entries follows, one per section:

| Offset | Size | Field |
|---|---|---|
| 0 | 2 | section kind |
| 2 | 2 | row size |
| 4 | 4 | offset in the blob |
| 8 | 4 | row count |

A reader skips a section kind it does not know, and takes the fields it knows
from each row, skipping the rest; a later version may append fields to a row.

A `Str` is a `u32` offset into `STRINGS`, which holds each string as a `u16`
length and its bytes. A `Ty` is a `u32` row of `TYPES`. A list is a `u32` first
row and a `u16` count in a shared table.

## Sections

| Kind | Section | Row | Holds |
|---|---|---|---|
| 1 | `STRINGS` | raw | names and symbols |
| 2 | `BYTES` | raw | constant values |
| 3 | `TEXT` | raw | template source |
| 4 | `IMPORTS` | 20 | each module this one imports |
| 5 | `TYPES` | 12 | every type the interface names |
| 6 | `TYPE_LIST` | 4 | type arguments |
| 7 | `DIMS` | 4 | array dimensions |
| 8 | `STRUCTS` | 16 | structs |
| 9 | `FIELDS` | 12 | fields of structs and variants |
| 10 | `ENUMS` | 12 | enums |
| 11 | `VARIANTS` | 16 | enum variants |
| 12 | `FIXED` | 8 | fixed-point types |
| 13 | `CONSTS` | 12 | constants |
| 14 | `STATICS` | 12 | module variables |
| 15 | `FUNCTIONS` | 28 | functions and methods |
| 16 | `PARAMS` | 12 | parameters |
| 17 | `PROTOCOLS` | 12 | protocols |
| 18 | `SUMMARIES` | 8 | one per function, in the same order |
| 19 | `EFFECTS` | 1 | one per parameter of a summarized function |
| 20 | `TEMPLATES` | 12 | generic, generator and protocol-default source |
| 21 | `DOCS` | 12 | comments, for the language server |

Only public declarations, and the types they reach, are written. A private
function exists only as object code.

### Rows

Offsets in bytes; `—` is zero padding.

**Import** (20): `0 module: Str`, `4 interface: [u8; 16]`, the imported
module's interface hash when this one was built.

**Type** (12): `0 kind: u8`, `1 arg: u8`, `2 count: u16`, `4 a: u32`,
`8 b: u32`. By kind:

| Kind | | `arg` | `a` | `b` | `count` |
|---|---|---|---|---|---|
| 0 | scalar | scalar code | | | |
| 1 | declared | section | row | | |
| 2 | imported | | module `Str` | name `Str` | |
| 3 | pointer | space and `mut` | target `Ty` | | |
| 4 | array | | element `Ty` | first of `DIMS` | rank |
| 5 | view | rank | element `Ty` | | |
| 6 | instance | | generic's name `Str` | first of `TYPE_LIST` | arguments |
| 7 | function | ABI | first of `PARAMS` | result `Ty` | parameters |

**Struct** (16): `0 name: Str`, `4 first_field: u32`, `8 fields: u16`,
`10 size: u16`, `12 align: u8`, `13 pack: u8`, `14 flags: u16` (public,
`bits`).

**Field** (12): `0 name: Str`, `4 ty: Ty`, `8 offset: u16`, `10 bits: u8`
(a `bits` struct's width, else 0), `11 flags: u8` (`mut`, public).

**Enum** (12): `0 name: Str`, `4 first_variant: u32`, `8 variants: u16`,
`10 tag_width: u8`, `11 flags: u8`.

**Variant** (16): `0 name: Str`, `4 tag: i32`, `8 first_field: u32`,
`12 fields: u16`, `14 —`.

**Fixed** (8): `0 name: Str`, `4 storage: u8`, `5 fraction: u8`,
`6 flags: u16`.

**Const** (12): `0 name: Str`, `4 ty: Ty`, `8 value: u32`, an offset into
`BYTES`; the type gives its size.

**Static** (12): `0 name: Str`, `4 ty: Ty`, `8 symbol: Str`.

**Function** (28): `0 name: Str`, `4 symbol: Str`, `8 result: Ty`,
`12 first_param: u32`, `16 params: u16`, `18 flags: u16` (public, method,
generic, generator, extern, export), `20 abi: u16` (0 is llrm's own
convention), `22 —`, `24 template: u32` (a row of `TEMPLATES`, or
`u32::MAX`).

**Param** (12): `0 name: Str`, `4 ty: Ty`, `8 mode: u8` (value, borrow,
mutable borrow, owned), `9 —`.

**Protocol** (12): `0 name: Str`, `4 first_method: u32` (rows of
`FUNCTIONS`), `8 methods: u16`, `10 flags: u16`.

**Summary** (8): `0 flags: u16` (never returns, pure, reads globals, writes
globals, allocates, may panic), `2 —`, `4 first_effect: u32`: the function's
`params` rows of `EFFECTS`.

**Effect** (1): a parameter's bits: read `1`, write `2`, escape `4`.

**Template** (12): `0 text: u32`, `4 length: u32`, both in `TEXT`, and
`8 line: u32` for diagnostics.

**Doc** (12): `0 section: u16`, `2 —`, `4 row: u32`, `8 text: Str`.

## Rebuilding

The interface hash covers every section but `DOCS`, and every header field
but the source and interface hashes. A module is stale when its source hash,
the compiler fingerprint, or the interface hash recorded for any import
differs from what it was built with. A module whose implementation changed but whose interface
did not leaves its dependents as they are; a changed comment rebuilds
nothing.

## Trade-off

A non-generic function in a package is not inlined into its callers. The
summaries keep what the optimizer learns from a callee besides its body:
memory effects, purity, whether it returns. Optimized MIR of small functions
may be added later as a section of its own, for inlining; readers that do not
know it skip it.

## Open

- Templates are stored as source text and re-parsed by the importing
  frontend. A binary syntax tree would save the parse and cost a format that
  follows every change to the syntax.
- The summary fields are to be fixed from what `callmemory`, `noreturn` and
  the other interprocedural analyses consume, not guessed.
