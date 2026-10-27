> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/memsetpattern.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/memsetpattern.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/memsetpattern.rs sha256:614d711e29d6f2fc6b7ac3c32c3ca9fbe58634559f952454c0397489d6e48028 -->

# memsetpattern

Which selectors take `memset.pattern` as a loop. The expansion itself is
[`llrm_transforms::memsetpattern`](memsetpattern.md).

## How it works
The DAG path has no node for the intrinsic, so a selector that is not the
old one gets its input expanded into a loop by the transform; the old
selector lowers the intrinsic itself.

## Used by
The compile driver (`driver/mod.rs`), which asks `expands_for` of its
selector and expands each module with `expanded`.

## Example
`memset.pattern(p, pat, n)` reaches the DAG selector as the loop `for i in
0..n { p[i] = pat }`.
