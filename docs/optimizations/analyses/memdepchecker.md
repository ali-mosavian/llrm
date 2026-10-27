> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/memdepchecker.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/memdepchecker.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/memdepchecker.rs sha256:82b59a080e07cedab734c474963dd2e2f91f7d544b00c88f7368baf8260114ca -->

# memdepchecker

`MemoryDepChecker`: whether the accesses of a loop to memory that may be
the same can be reordered across iterations, and by how much.

## How it works
A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`. Each load
and store of the loop is numbered in program order (`addAccess`; a phi of
pointers inside the loop gives one access for each incoming pointer).
`areDepsSafe` takes each dependence class of the accesses that need a check
and classifies every pair, a write against another access, by `isDependent`:
the SCEV distance between the two pointers, in bytes, against the strides
and the sizes of the accesses and the loop's trip count. The result is the
worst status over the pairs, the smallest dependence distance, and the
dependences themselves while there are fewer than `-max-dependences`.

## Example
`a[i + 1] = a[i]` is a forward dependence of distance one element: the
load is safe to run ahead of the store. `a[i] = a[i + 1]` is a backward
dependence, safe to vectorize when the distance covers the vector.

## DEVIATE 24 (no predicated SCEV)
Where LLVM adds a wrap predicate or an addrec-with-predicates to prove a
stride (`getPtrStride` with a `Predicates` argument) the access has no
stride here and the pair is `IndirectUnsafe`.

## Used by
Named by `accessanalysis`, `loopaccessinfo`, `looploadelim`, `runtimecheck`.
