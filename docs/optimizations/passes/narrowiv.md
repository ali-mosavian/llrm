> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/narrowiv.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/narrowiv.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/narrowiv.rs sha256:35a8c9fbc8a02ae28ae4681eb6e6d0d5672db5d641e043a70813ec1a9e819df3 -->

# narrowiv

A loop counter whose range provably fits a native integer width becomes a
counter of that width. Beyond the pin: IndVarSimplify only widens
counters, so `for (long i = 0; i < 12; ++i)` keeps four bytes on an 8-bit
target, a four-byte compare for each test of `i`.

DEVIATE IV-NARROW (owner z80-b, review 2026-12-20): no pin pass narrows a
counter; `WidenIV`
([SimplifyIndVar.cpp](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyIndVar.cpp#L1855))
only widens, and `opt -passes=default<O2>` on this IR at `n8:16` keeps the
`i32` phi. `TruncInstCombine` and instcombine's trunc demotion start from a
`trunc` and cannot reach a phi cycle that has other users.

## How it works
For each loop in LoopSimplify form, a header phi `%i` of an integer width
the datalayout does not hold natively (`legal_integer`, the test of LLVM's
`shouldChangeType`), with the preheader's start and `%next = add/sub %i, X`
from the latch (`X` loop-invariant), is narrowed when ScalarEvolution's
range of `%i` and of `%next` fits a native width `W` below it: unsigned
(`zext`) or signed (`sext`), whichever is narrower. The loop then carries a
phi and an increment of `W` bits; every use of `%i` and `%next` reads a
`zext`/`sext` of them, which the combiner folds into the uses.

## Example
`%i = phi i32 [0, %entry], [%next, %loop]`, `%next = add i32 %i, 1`, exit on
`%next == 12` (range 0..=12 for both) becomes
`%i.n = phi i8 [0, %entry], [%next.n, %loop]`, `%next.n = add i8 %i.n, 1`,
`%i = zext i8 %i.n to i32`, `%next = zext i8 %next.n to i32`.

## Used by
[`crate::indvars`](indvars.md) runs `narrowed_in` after LFTR where
`Tti::narrows_counters`.

## See also
[`crate::widen`](widen.md) (the other direction), `docs/architecture/scev-port.md`.
