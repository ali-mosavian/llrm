> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/loopaccess.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/loopaccess.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/loopaccess.rs sha256:4e728bf1adc26dfa66d79f012ce50ecc280a48cc5086ab69cee7ae1c3b2309d0 -->

# loopaccess

`getPointersDiff`, `isConsecutiveAccess`, `getPtrStride` and the pointer
walks `LoopAccessInfo` is built on.

## How it works
A translation of LLVM `d1106deb71cc` `LoopAccessAnalysis.cpp`: the
distance between two pointers in units of an element, from their constant
offsets off a common base, else from ScalarEvolution's constant
difference of the two; the constant stride of a pointer that is an affine
add recurrence of the loop, and whether it can wrap; the pointers a
phi or select forks an access into (`visitPointers`, `findForkedSCEVs`).

## DEVIATE 24 (no predicated SCEV), 25 (no symbolic strides)
There is no `PredicatedScalarEvolution`: `getPtrStride` and `isNoWrap`
never add a predicate (their `Predicates` argument is null), and an
overflow they would have assumed is a pointer that cannot be checked. No
`Stride == 1` is ever speculated: `SymbolicStrideMap` stays empty
(`collectStridedAccess` records nothing) and `replaceSymbolicStrideSCEV` is
`getSCEV`. Exit: PredicatedScalarEvolution.

## Example
`getelementptr i32, ptr %p, i64 1` is consecutive after `%p`.

## Used by
Named by `accessanalysis`, `driver`, `loopaccessinfo`, `looploadelim`, `memdepchecker`, `runtimecheck`.
