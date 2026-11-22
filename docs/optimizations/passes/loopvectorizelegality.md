> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopvectorizelegality.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopvectorizelegality.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopvectorizelegality.rs sha256:d33072fa9d38f9fa63b3410e86bf4709865181ef29314374f800e6ff1432d94a -->

# loopvectorizelegality

`LoopVectorizationLegality`: whether an innermost loop may be vectorized.

## How it works
A translation of LLVM `d1106deb71cc` `LoopVectorizationLegality.cpp`
(`canVectorize`,
[L1889](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Vectorize/LoopVectorizationLegality.cpp#L1889)),
in the pin's order: the loop's CFG (`canVectorizeLoopNestCFG`, L1626), one
block (`canVectorizeWithIfConvert`, L1449), every instruction
(`canVectorizeInstrs`, L799: header phis are reductions, inductions or
refused; calls and unwidenable types are refused), a countable trip count,
then the memory accesses (`canVectorizeMemory`, L1193, from
`LoopAccessInfo`). The primary induction (`addInductionPhi`, L706) is the
widest integer induction that starts at 0 and steps by 1.

## Example
`loop: %i = phi [0, %ph], [%n, %loop]; %p = gep %a, %i; %v = load %p;
store %v, gep %b, %i; %n = add %i, 1; br (%n == 1920), exit, loop` is
legal with primary induction `%i`; the same loop with a `call` in it is not.

## DEVIATE rows (features the slice does not need; each exit is its piece)
- LV-D1 outer loops and the VPlan-native path (`canVectorizeOuterLoop`):
  only innermost loops.
- LV-D2 if-conversion (`canVectorizeWithIfConvert`,
  `blockNeedsPredication`): a loop of more than one block is legal only
  where every block runs on every trip (it dominates the latch), so no block
  is predicated. Exit: predicated blocks (`blockCanBePredicated`).
- LV-D3 early-exit loops (`isVectorizableEarlyExitLoop`): read-only loops
  whose loads are dereferenceable in the loop only; no stores (the pin's
  `-enable-early-exit-vectorization-with-side-effects` is off), no
  fault-only-first loads (`-enable-early-exit-with-ffload` is off), no
  SCEV predicates. The planner takes them through `loopvectorizeearlyexit`.
- LV-D4 calls: an intrinsic `getVectorIntrinsicIDForCall` takes is legal (a
  scalar operand loop invariant, L965); a call the `VFDatabase` or
  `isTLIScalarize` would take is refused. The transform widens fewer.
- LV-D5 fixed-order recurrences and the coerced-AddRec induction retry
  (`isFixedOrderRecurrence`, `PredicatedScalarEvolution`): such a phi is
  refused as unidentified.
- LV-D6 histograms (`EnableHistogramVectorization` is off in the pin) and
  non-temporal accesses (`isLegalNTLoad`/`isLegalNTStore`, metadata).
- LV-D7 `LoopVectorizeHints` (`llvm.loop.vectorize.*` metadata): every loop
  allows vectorization and reordering.

## Used by
Named by `loopvectorize`, `loopvectorizeearlyexit`, `loopvectorizeinterleave`.
