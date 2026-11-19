> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopvectorizeinterleave.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopvectorizeinterleave.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopvectorizeinterleave.rs sha256:2b573d669616e43e73e8d13caac186aad6383e856d94585002f2db26f739643c -->

# loopvectorizeinterleave

Interleaving without vectorizing: `LoopVectorizePass` with VF 1 and IC > 1.

## How it works
A translation of the interleave-only branch of LLVM `d1106deb71cc`
`LoopVectorize.cpp` `processLoop`
([L8067](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Vectorize/LoopVectorize.cpp#L8067),
"interleaved loop (interleaved count: N)"): `computeBestVF` chose VF 1, and
`selectInterleaveCount` (L3705,
[`crate::loopvectorizeplan::choose_interleave`](loopvectorizeplan.md)) chose IC > 1.
`VPlanTransforms::unrollByUF` ([VPlanUnroll.cpp
L504](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Vectorize/VPlanUnroll.cpp#L504))
copies every recipe once per part; part `k` reads the induction as
`index + k`. Each reduction gets one phi per part, so the parts do not wait
for each other (`VPReductionPHIRecipe`), and the middle block folds the
parts' results (`bin.rdx`, `VPlanRecipes.cpp` L980). A remainder resumes the
reduction from `bc.merge.rdx`.

## Example
```text
loop: %i = phi [0, %ph], [%n, %loop]         loop: %i = phi [0, %ph], [%n, %loop]
      %s = phi [0, %ph], [%a, %loop]    =>         %s = phi [0, %ph], [%a, %loop]
      %v = load i16, gep %p, %i                    %s1 = phi [0, %ph], [%a1, %loop]
      %x = sext i16 %v to i64                      %v = load .. ; %i1 = add %i, 1 ; %v1 = load gep %p, %i1
      %a = add %s, %x                              %a = add %s, %x ; %a1 = add %s1, %x1
      %n = add %i, 1                               %n = add %i, 2
exit: phi [%a, %loop]                        middle: %bin.rdx = add %a1, %a
```

## DEVIATE rows
- LVI1 the loop is copied in place (as LVP1), not rebuilt from a VPlan; the
  scalar cost of an extension of a load is 0 for every target (the pin asks
  the target about the load's use). Exit: VPlan.
- LVI2 the reductions are the kinds with an identity (`add`, `mul`, `and`,
  `or`, `xor`) whose final value is the only value used after the loop.
  Exit: min/max (`select`/`icmp` chains), any-of and find-last.
- LVI4 the pin decides VF first (`computeBestVF`): a loop it vectorizes is
  not interleaved alone. The widening of casts and reductions is not
  translated (LV-X4), so the loop is interleaved only if no vector factor
  takes its loads, stores and arithmetic (an `i64` multiply has no NEON
  form); a cast or reduction a vector factor would also price is not priced.
  Exit: the vector cast and reduction costs with LV-X4.
- LVI3 the register pressure is counted on the loop's instructions in order,
  one register per live value (`calculateRegisterUsageForPlan`, L106, at VF
  1); invariants are the instructions defined outside and read inside.

## Runs
After the vector plan of [`crate::loopvectorize`](loopvectorize.md) did not widen the loop.
