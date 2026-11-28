> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/loopderef.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/loopderef.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/loopderef.rs sha256:08f1badd6ae1724d27f0b1ed081d517a41e2d6396fa261d62250911101dfc347 -->

# loopderef

Whether every access of a load in a loop stays inside memory the pointer's
base makes dereferenceable (`isDereferenceableAndAlignedInLoop`).

## How it works
A translation of LLVM `d1106deb71cc` `Loads.cpp`
(`isDereferenceableAndAlignedInLoop`, `isReadOnlyLoop`). A pointer that is
an affine add recurrence of the loop with a constant step no smaller than
the access walks `[start, end)` over the loop's symbolic-maximum trips; the
access is safe when that span, from a base of known dereferenceable size
and enough alignment, fits in it. A loop-invariant pointer is safe when its
own bytes are.

## Example
`a[i]` over `i < 1200` with `@a = [1200 x i8]`: start `@a`, end `@a + 1200`,
span 1200 <= the 1200 bytes of `@a`: safe, so a vector of 16 may load it
before the early exit decides.

## DEVIATE LD1 (no operand bundles, no predicates)
The pin's retained-knowledge path
(`isDereferenceableAndAlignedPointerViaAssumption`) and the SCEV predicates
are not translated: a base is dereferenceable by its own attributes or
object size only. Exit: `llvm.assume` bundles in MIR.

## Used by
Named by `loopvectorizelegality`.
