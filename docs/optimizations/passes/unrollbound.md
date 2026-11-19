> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/unrollbound.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/unrollbound.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/unrollbound.rs sha256:5402a3df25851b6d5bb83b49c3dbe7b71e7e35dcb27fd0726545ed8c43f06300 -->

# unrollbound

The most trips a loop can make, from the size of the object it accesses.

A load or store outside the object its pointer is based on is undefined. If
one runs on every trip of a loop, and its address is `base + start + step *
trip` with a positive constant step, the loop cannot make more trips than
fit in the object. gcc reads the same bound from the extents of C arrays
(`infer_loop_bounds_from_array`); this reads the extent of the whole object
(a global, a static alloca), which LLVM IR guarantees where an inner array
extent it does not.

## Deviations
- OB1 The pin has no such bound: `getSmallConstantMaxTripCount` is
  `getConstantMaxBackedgeTakenCount` alone (pin :8419). Beyond it, read only
  by the unroller, and only where the target asks
  (`Tti::unroll_object_bound`). Owner w12, exit 2026-11-27.

## Example
`B[k][j]` with `double B[12][12]`, `k` counting from 0 and `j >= 0`, is
`@B + 8*j + 96*k`: the loop over `k` makes at most 12 trips.

## Used by
Named by `loopunrollpass`.
