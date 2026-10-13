> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/layout.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/layout.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/layout.rs sha256:c1e2c462dbfbdf9e8402547af995183a0190f2e8ead44e0b51e0a3da5783fc9e -->

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
