> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/layout.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/layout.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/layout.rs sha256:4a2a771edc21160c3a28cf91bfc7a2e569474788df271a55794516576b39de94 -->

# layout

The pairs of branch conditions that are each other's inverse.

## How it works
`_PAIRS` lists each condition mnemonic with the one that is true exactly
when it is false (`je`/`jne`, `jb`/`jae`, and the aliases of each).
`_OPPOSITE` is the table in both directions. `jcxz` and `loop*` are absent
on purpose: they have no inverse to name, so a block ending in one keeps its
jump.

## Example
No example: a table of condition names. See `jumps.rs` for the inversions it
serves.

## Used by
`jumps`, to invert a branch when it makes the other successor the
fall-through.
