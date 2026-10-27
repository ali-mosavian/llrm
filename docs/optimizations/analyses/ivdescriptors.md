> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/ivdescriptors.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/ivdescriptors.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/ivdescriptors.rs sha256:a1b5c98349593cf1cbd3d61a5bbf04b659a34bd3d30a049cfb2b6c292b4eb803 -->

# ivdescriptors

`IVDescriptors`: the recurrences (reductions) and induction variables of a
loop's header phis, `IVDescriptors.cpp` at `d1106deb71cc`.

## How it works
`is_reduction_phi` tries each recurrence kind in LLVM's order (`Add`,
`Sub`, `AddChainWithSubs`, `Mul`, `Or`, `And`, `Xor`, the min/max
intrinsics, `AnyOf`, `FindLast`, `FMul`, `FSub`, `FAdd`,
`FAddChainWithSubs`, `FMulAdd`) and answers the first
`RecurrenceDescriptor` that fits: the walk from the phi through its users
must come back to it, every value in the cycle used once by the cycle (apart
from phis and conditional patterns), at most one value used outside the
loop, and each instruction a recurrence instruction of the kind
(`is_recurrence_instr`). `is_induction_phi` asks SCEV for an affine
recurrence of the loop whose start is the preheader's value and gives its
step; a floating-point phi is an induction by its `fadd`/`fsub` of an
invariant.

## Deviations from LLVM
- D1: `DemandedBits`, `AssumptionCache` and `DominatorTree` are not
  parameters: with none of them `computeRecurrenceType` is the type's width
  rounded up to a power of two, and that is all this port computes. The only
  caller (`LoopInterchange`) passes none of them.
- D2: `PredicatedScalarEvolution`: `is_induction_phi` takes the
  `ScalarEvolution` overload only; the phi's SCEV is not forced into an add
  recurrence under predicates.
- D4: an FP induction's step is its addend (`InductionStep::Fp`); SCEV has
  no floating-point type to hold `getUnknown(Addend)`.
- D3: `isFixedOrderRecurrence` and the conditional induction are not
  translated: nothing here asks for them.

## Example
`%s = phi [0, %pre], [%s.next, %latch]` with `%s.next = add %s, %x` and one
use of `%s.next` after the loop is an `Add` reduction starting at 0;
`%i = phi [0, %pre], [%i.next, %latch]` with `%i.next = add nuw %i, 1` is an
integer induction of step 1.

## Used by
Named by `canonfreeze`, `exitvalues`, `loopinterchange`.
