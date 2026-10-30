> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/runtimecheck.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/runtimecheck.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/runtimecheck.rs sha256:b032fdfab45b497d9daf0eb039b8318f7b4d2b97e98d567b6ed8e765bc92104c -->

# runtimecheck

`RuntimePointerChecking`: the bounds a loop's pointers walk, grouped so a
few comparisons prove that the writes do not overlap the other accesses.

## How it works
A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`
(`getStartAndEndForAccess`, `RuntimePointerChecking`,
`RuntimeCheckingPtrGroup`). `insert` records a pointer with the lowest
byte it touches in the loop and the highest plus one, from its SCEV and the
loop's backedge-taken counts. `generate_checks` groups the pointers of
each dependency class whose bounds differ by a constant, and every pair of
groups some write and a different dependency set make needs a check.
`crate::scev` expands the checks' bounds in `llrm-transforms`
(`addRuntimeChecks`).

## Example
`a[i] = b[i]` over `n` trips: `a` is bounded by `[a, a + 4n)` and `b` by
`[b, b + 4n)`; the one check is that the two ranges do not meet.

## DEVIATE 23 (no operand bundles)
MIR has no operand bundles, so `evaluatePtrAddRecAtMaxBTCWillNotWrap`'s
`getKnowledgeForValue(..., Dereferenceable, ...)` finds no assumption: the
dereferenceable bytes are the value's own. Exit: operand bundles in MIR.

## DEVIATE 24 (no predicated SCEV)
`insert` takes the loop's backedge-taken counts where LLVM takes a
`PredicatedScalarEvolution`, which here adds no predicate (D1: SCEV has
none). Exit: PredicatedScalarEvolution, with `replaceSymbolicStrideSCEV`.

## Used by
Named by `accessanalysis`, `laa_testing`, `loopaccessinfo`, `looploadelim`, `loopversioning`, `memdepchecker`, `runtimechecks`.
