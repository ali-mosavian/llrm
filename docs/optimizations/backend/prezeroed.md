> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/prezeroed.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/prezeroed.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/prezeroed.rs sha256:34cd30157de3f70c7ebf5604e20e83d4254921cc61c0ad58258cf5da175974e4 -->

# prezeroed

The entry block's zero writes to a local nothing has touched yet, for a
frame the runtime zero-fills (QB's `B$ENRA`): the frame already holds them.
Not the pin's (it has no such runtime); both selectors drop the same set.

DEVIATE SD222: the DAG path drops these writes as the old selector does;
the pin has no runtime that zero-fills a frame. Owner w12. Exit: none
(a QB runtime fact). Review 2026-12-10.

## Example
No honest example: no dump prints the function before and after. In an
entry block `store 0 -> local; call f(local)`, the store is dropped, the
call stays.

## Used by
The DAG path (`selectiondag/builder.rs`) and the old selector (`isel.rs`),
for every function they select.
