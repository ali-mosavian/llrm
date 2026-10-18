> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inlinecost.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inlinecost.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inlinecost.rs sha256:53501a3abedfe11b6707bd7f648e0110a00ea8a12018f7f1763e7be07e0a07a6 -->

# inlinecost

LLVM's inline cost: what inlining one call site costs, and the threshold
it is held to, translated from `InlineCost.cpp` and `InlineAdvisor.cpp`.

## How it works
`inline_cost` is `getInlineCost`: a callee's attributes decide first
(`attribute_decision`); else `CallAnalyzer` walks the callee's live blocks
from the entry, breadth first, as the call site would leave them (a
conditional branch on a constant argument has one live successor, and the
blocks only the other reached are dead), adds the cost of every instruction
that does not simplify away, and compares it with the threshold:

- an instruction costs `INSTR_COST` (5) unless the walk proves it free: it
  folds to a constant on the site's arguments, is a free cast or address, a
  branch that folds, the first `ret`, a load or store that SROA of a caller
  alloca removes, a load of an address loaded before;
- a call it cannot fold costs its argument setup and `CALL_PENALTY` (25);
- the threshold starts at 225 (`-O2`; 250 `-O3`) and is raised by
  `inlinehint` (325), lowered at an `optsize` caller (50) or `minsize` (5)
  or a cold site (45), raised at a hot one; half of it again is a bonus
  while the callee stays one block, and the last call to a local function
  takes 15000 off the cost;
- the call and its arguments disappear: `callsite_cost` is taken off.

A site is inlined where `cost < max(1, threshold)`. The caller of this
module asks `inline_cost` and acts on the verdict; nothing is changed
here.

## Example
A callee of one block, `%a = load @g; %b = add %a, %x; store %b, @g; ret
%b`, called from `@f` (not the only call): the single-block bonus raises the
threshold to 337 and the load, add and store cost 5 each less the call
they replace, which comes to -20, as `opt -passes=inline` says.

From unit test `test_a_single_block_callee_costs_what_opt_says` in
`inlinecost_tests.rs`.

## Runs
Off by default. With `LLRM_EXP_INLINECOST` the interprocedural stage prints
each site's cost and threshold beside the existing decision; with
`LLRM_EXP_INLINEDECIDE` it decides each call site by them.

## Deviations from LLVM (numbered, measured when the diff against
`opt -passes=inline` has rows)
- D1: block frequencies: LLVM's `BlockFrequencyInfo` decides a cold or hot
  site; the caller passes the site's frequency relative to the caller's
  entry (`Site::relative_frequency`), from `profit`'s.
- D2: `TargetTransformInfo`: `getInstructionCost`, `isLoweredToCall`,
  `getFPOpCost`, `getEstimatedNumberOfCaseClusters` are the generic
  (BasicTTIImpl) answers with the x86 legal addressing modes, in
  `free_instruction`, `lowered_to_call` and `case_clusters`.
- D3: constant folding is integer, float and `select`; vectors and constant
  expressions are not folded.
- D4: no PGO, ThinLTO, ML advisor, `optnone`, `flatten`, coroutines, strict
  FP or target-feature checks, no `call-inline-cost` style string
  attributes, no cost-benefit analysis (off by default in LLVM).
- D5: ephemeral values (operands only an `llvm.assume` reads) are not
  skipped; assumes are free instructions.
- D6: `simplifyCmpInstForRecCall` and `llvm.objectsize` / `llvm.is.constant`
  are not translated (a missed simplification costs `INSTR_COST`).
- D8: -Os and -Oz share one pipeline here, so every caller at -Os is
  `optsize` (clang's `OptimizeForSize`, InlineCost.cpp:2125) and none
  `minsize` (:2117).
- D9: `noipa` (GCC's; LLVM has no IR form) is `Never` before any other
  attribute decision.
- D7: the minsize loop penalty counts retreating-edge headers, not
  `LoopInfo`.

## See also
[`crate::inline`](inline.md), the decision this is to replace; the plan is
the cgscc plan page in docs/architecture.
