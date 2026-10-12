> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/promote.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/promote.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/promote.rs sha256:85812510555cf29a956c17e202ffd176c7bd4809ac3ceb3f171b6fc5535e113f -->

# promote

Write-through promotion: a load of a cell a store left its value in on
every path becomes that value, a phi where paths join; every store
stays. `Sroa` does it for aggregates' leaves alone.
LLVM's counterpart: LICM's scalar promotion, function-wide; `Sroa`'s, SROA.

Adapted from llrm-core's `optimize/promote.rs`. What an instruction
names and writes is `memoryssa::Accesses`' answer, and whether a write
reaches a cell
`regions::overlapping`'s. Globals and callees' attributes come through
the outer proxy.

What changed with the IR:
- A cell is its exact leaf, or else its pointer decomposed (`Key::Ref`) over
  any root: the old `Addr` key, and `_allocation_leaves`' grouping by affine
  root. A leaf of one start takes any stride.
- One LLVM type per cell stands for the old width and float checks
  (`_float_cells`, `_stores_value`, `_converts_integer`, `_forwards_float`):
  a store keeps its type's value.
- A load becomes the stored operand itself, so `_restated` and `_order` have
  nothing to do; the phis are placed here, as `ssa::constructed` was not
  ported.
- `_canonical_leaf_types` types a leaf in both passes: the old `Sroa`
  rewrote the body the old `Promote` then read.

Dropped, with no rich MIR counterpart:
- `READS`, `_separated`, `split_updates`: an arithmetic memory operand.
- `CELLS`, `Bounds`: the x86 spaces and landmarks `regions` dropped.
- `_initializers`: every object access carries exact provenance, so a
  narrower store into a wider cell blocks both.
- `_allocation_leaves`, `_affine_values`, `_signed`, `_rewritten_refs`: no
  array request; `MemRef::at` decomposes an address.
- `_bounded_leaves`, `_bounded_ref`, `_pointed_ref`: `alias::annotated`
  narrows constant indices and follows exact pointers.
- `_split_copies` and its helpers: `splitcopy`, for a `llvm.memcpy`.
- `loop_only`: no caller set it.
