> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/accessanalysis.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/accessanalysis.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/accessanalysis.rs sha256:990d5d65a1b20d684b46909d0f43eec9f97ead9e5cf10182fe4beff789b2ac99 -->

# accessanalysis

`AccessAnalysis`: the memory accesses of a loop, grouped by what they may
alias, for the dependence checker and the runtime checks.

## How it works
A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`. The loads
and stores are added with the locations they touch across all iterations;
an `AliasSetTracker` over a batch alias analysis in cross-iteration mode
partitions them. `buildDependenceSets` joins the accesses of one set that
share an underlying object into dependence classes (`DepCands`) and lists
those that need a dependence check; `canCheckPtrAtRT` finds the bounds of
each pointer of each set and fills the `RuntimePointerChecking`.

## Example
`a[i] = b[i]` over `a` and `b` that may alias: one set of two pointers, two
dependence classes, so one check that `[a, a + 4n)` and `[b, b + 4n)` do
not meet.

## DEVIATE 24 (no predicated SCEV)
`createCheckForAccess` in its `Assume` retry would add `PSE.getAsAddRec`
and wrap predicates; here a pointer that is not an add recurrence, or may
wrap, cannot be checked.

## DEVIATE 26 (no alias scope lists)
MIR's `AAInfo` has the TBAA tag and one `noalias` scope but no
`alias.scope` list and no `llvm.experimental.noalias.scope.decl` scopes
declared in a loop (`adjustAliasScopeList`): the locations are added as
they are.

## Used by
Named by `loopaccessinfo`.
