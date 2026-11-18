> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopvectorize.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopvectorize.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopvectorize.rs sha256:a0c7f553dee22a8730589361ad1f18c83d4bf235e8dd7f264fada2a82502aedf -->

# loopvectorize

`LoopVectorizePass`: widens the loops the legality check accepts and the
planner prices, by the factors it chooses.

## How it works
A translation of LLVM `d1106deb71cc` `LoopVectorize.cpp`
(`processLoop`,
[L7714](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Vectorize/LoopVectorize.cpp#L7714))
over the translated parts: `can_vectorize` (`LoopVectorizationLegality`),
`plan` (`LoopVectorizationPlanner`) and the execution of the plan
(`VPlan::execute`, `InnerLoopVectorizer`). For each innermost loop of the
function the pass asks legality, extracts the recipes (the loop's memory
accesses and control), asks the planner for VF and IC and, if both agree,
rewrites the loop: each load becomes `IC` wide loads of `VF` lanes, each
store `IC` wide stores of the loaded values (the part `k` at
`k * VF` elements past the first), and the induction steps by `VF * IC`.

## Example
```text
loop: %i = phi [0, %ph], [%n, %loop]       loop: %i = phi [0, %ph], [%n, %loop]
      %v = load i16, gep %a, %i      =>          %wide.load = load <8 x i16>, gep %a, %i
      store i16 %v, gep %b, %i                   %g1 = gep (gep %a, %i), 8
      %n = add nuw nsw %i, 1                     %wide.load1 = load <8 x i16>, %g1 ...
      br (%n == 1920) ...                        %n = add nuw nsw %i, 16
```

## DEVIATE rows
- LVP1 the vector loop is made in place: the scalar induction is the vector
  loop's `index` (the pin makes `vector.ph`, `vector.body`, `middle.block`
  and keeps the scalar loop as the remainder). The remainder is a copy of
  the loop made first ([`crate::loopvectorizeskeleton`](loopvectorizeskeleton.md)); with a constant
  trip count VF x IC divides there is none, as in the pin after SimplifyCFG.
  Exit: the VPlan skeleton (VP1).
- LVP2 a loop whose induction value is used after the loop, or whose exit
  test is not `icmp eq`/`ne` of the stepped induction against a
  loop-invariant bound, is left alone (the induction may start anywhere: the
  pin's VPlan makes its own canonical IV). Exit: the middle block's fix-ups
  of live-outs (LVP5) and the exit tests `ult`/`slt` against a bound.
- LVP3 recipes are loads and stores of consecutive addresses and the loop
  control (VP2 of the planner): a stored value is a loaded value or a
  loop-invariant one (broadcast in the preheader, as the pin's
  `broadcast.splat`). Exit: reverse accesses.
- LVP5 recipes also include integer `add`, `sub`, `mul`, `and`, `or`, `xor`
  and shifts by a constant, on loaded and loop-invariant operands (the pin's
  `VPWidenRecipe`). LVP6 a shift by anything else is left scalar. An
  operation of the induction is widened on a `vec.ind` phi
  (`VPWidenIntOrFpInductionRecipe`): the integer induction read as a value,
  or its `trunc`; a floating-point induction is not.
- LVP7 recipes also include `icmp`, `fcmp` (read only by the condition of a
  select) and `select` by such a compare (`VPWidenRecipe`,
  `VPWidenSelectRecipe`), where the backend selects vector compares
  (`Tti::selects_vector_compares`). A select by a loop-invariant condition
  (a scalar `i1` in the pin) and a compare read otherwise (an `i1` vector
  `and`/`zext`) stay scalar. Exit: LV-S1 (owner llrm-arm64-vec, 2026-11-24).
- LVP4 the new instructions carry no metadata (`!tbaa`, `!alias.scope`).

## Runs
At `loop-vectorize` (l.1354 of `PassBuilderPipelines.cpp`), only where the
target takes vector values (`Machine::has_vector_mir`).
