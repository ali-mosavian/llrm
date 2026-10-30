> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/runtimechecks.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/runtimechecks.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/runtimechecks.rs sha256:be2a3e32abbbf513d040533765b6c516f79247b08984b177bc880108335c2923 -->

# runtimechecks

Runtime memory checks: code that proves, when a loop is entered, that the
groups of pointers it accesses do not overlap.

## How it works
A translation of LLVM `d1106deb71cc` `LoopUtils.cpp` (`expandBounds`,
`addRuntimeChecks`, `addDiffRuntimeChecks`). Each check names two groups
of a `RuntimePointerChecking`; their bounds are expanded before `loc`
with the SCEV expander, and the groups conflict when
`start(A) <u end(B) && start(B) <u end(A)`. The conflicts are or-ed into
one `i1`. The diff form compares `(sink - src - 1) <u (VF * IC * size - 1)`
instead.

## Example
`a[i] = b[i]` over `n` trips: one check, `a < b + 4n && b < a + 4n`, true
when the two ranges meet.

## DEVIATE 28 (fixed vectors)
`addDiffRuntimeChecks` takes `VF` as a count, not an `ElementCount`: MIR
has no scalable vectors (DEVIATE 2). Exit: scalable types in MIR.

## Runs
Called by [`crate::loopversioning`](loopversioning.md); the vectorizer will call it too.
