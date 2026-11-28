> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/interleavedaccess.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/interleavedaccess.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/interleavedaccess.rs sha256:91fce486b19e98f8a17040c1d6b6b45a66e27a6c29aa89d7e74ecdb888b82b3c -->

# interleavedaccess

`InterleaveGroup` and `InterleavedAccessInfo`: the loads and stores of a
loop that stride through memory in steps of two to four elements, grouped
so a vectorizer can make one wide access and shuffles of each group.

## How it works
A translation of LLVM `d1106deb71cc` `VectorUtils.h` (`InterleaveGroup`,
`canReorderMemAccessesForInterleavedGroups`, `collectDependences`) and
`VectorUtils.cpp` (`collectConstStrideAccesses`, `analyzeInterleaving`,
`invalidateGroupsRequiringScalarEpilogue`). Every access with a constant
stride (`getPtrStride`) is visited bottom up in program order; an access
`B` of an allowed stride starts a group, and each earlier access `A` of the
same kind, stride, size and address space whose distance from `B` is a
whole number of elements joins it, unless a dependence would be moved over
by hoisting a load of the group or sinking a store of it. A group with gaps
must not wrap (`getPtrStride` with `ShouldCheckWrap`); a load group with a
gap at its end needs a scalar epilogue iteration.

## Example
```text
for (i = 0; i < n; i++) { p[2*i] = a; p[2*i + 1] = b; }   // one store group, factor 2
```

## DEVIATE rows
- IAI1 (owner llrm-arm64-vec, exit 2026-11-24): no
  `PredicatedScalarEvolution` (LoopAccessAnalysis D24): the predicates
  `collectConstStrideAccesses` would collect are never added, so an access
  whose stride needs an assumption has stride 0 and joins no group.
- IAI2 (same owner and exit): `MaxInterleaveGroupFactor` is its default, 8;
  the `-max-interleave-group-factor` option does not exist.

## Used by
Named by `loopvectorize`.
