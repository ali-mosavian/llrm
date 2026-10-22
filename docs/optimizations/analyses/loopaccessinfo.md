> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/loopaccessinfo.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/loopaccessinfo.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/loopaccessinfo.rs sha256:039a663bc8f47ec831d59aabdc050465e4fcac2c4e5d24eb68cedbb80cdf61ef -->

# loopaccessinfo

`LoopAccessInfo`: whether the memory accesses of an innermost loop can be
reordered across iterations, and the runtime checks that would make them
so.

## How it works
A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`
(`LoopAccessInfo::canAnalyzeLoop`, `analyzeLoop`, `print`). The loop's
loads and stores are collected in reverse postorder (a call or any other
memory instruction that is not a simple load or store makes the loop
unanalysable); the pointers they use are given to an `AccessAnalysis` that
partitions them by what they may alias, builds the dependence classes and
finds the bounds for runtime checks; the `MemoryDepChecker` then classifies
the dependences within the classes, and if those are not provably safe but
may be with checks, the bounds are found again for every pointer.

## Example
`a[i + 1] = a[i] + x` over a loop with a known trip count: "Memory
dependences are safe", one forward dependence, no checks.

## DEVIATE 24 (no predicated SCEV), 25 (no symbolic strides)
See [`crate::loopaccess`](loopaccess.md): the counts are the loop's own, no predicate is
ever collected, and `collectStridedAccess` finds nothing to speculate.

## DEVIATE 27 (no loop metadata, no debug locations)
MIR has no `llvm.loop.parallel_accesses` annotation
(`isAnnotatedParallel` is false), no `llvm.loop.distribute.enable` and no
debug location on a remark ("Memory location is the same as accessed at"
is not added), and no vector function database (`VFDatabase`).
