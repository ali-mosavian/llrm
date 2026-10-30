> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/inlinecost.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/inlinecost.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/inlinecost.rs sha256:ed361ca41374374677a005673852f5c63746b268582917e53ae1ed445f877d95 -->

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
- D1: block frequencies: `BlockFrequencyInfo` over `BranchProbabilityInfo`,
  both translated (`llrm_analysis::blockfreq`, `llrm_analysis::bpi`). The
  caller passes the site's frequency relative to its entry
  (`Site::relative_frequency`), a ratio where LLVM compares
  `BlockFrequency`s.
- D2: `TargetTransformInfo`: `getInstructionCost`, `isLoweredToCall`,
  `getFPOpCost`, `getEstimatedNumberOfCaseClusters` are the generic
  (BasicTTIImpl) answers with the x86 legal addressing modes, in
  `free_instruction`, `lowered_to_call` and `basic_case_clusters`
  (`tti.rs`).
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
- D11: at -Os a callee that stays is inlined only if it does not grow the
  code (`Tti::opt_size_inline_by_code_size`, X86: yes): the target's
  `CodeSize` of the callee's instructions that the walk did not fold must
  not exceed the `CodeSize` of the call and the pushes of its arguments.
  LLVM's `OptSizeThreshold` 50 allows growth; a fixed bound (25) copied a
  14-instruction loop into ten sites (`idioms.bas`, +690 B). The last call
  of a local callee, which goes with it, is the pin's. No shape of callee
  but the loop and the one smaller than its call is tested.
- D12 (SZ2): a call the -Os bound (D11) alone refuses, that passes the
  address of a stack slot or a global the caller stored a constant to and
  whose callee loads through that parameter, is inlined if the caller,
  simplified again with the call inlined, is smaller by the target's size
  cost (`walk.rs`, callers of at most 300 instructions; the sites of a
  private callee that dies with its last call are tried together, its body
  credited; so are the sites of a private callee called only from this
  caller, whatever they pass). The pin's cost does not follow a load through
  a pointer argument, so it never sees the loop or the arithmetic fold.
- D9: `noipa` (GCC's; LLVM has no IR form) is `Never` before any other
  attribute decision.
- D7: the minsize loop penalty counts retreating-edge headers, not
  `LoopInfo`.
- D10 (`-finline-recursive`, off by default): a callee that calls itself is
  priced as any (LLVM: "recursive", `IsRecursiveCall`, unless
  `AllowRecursiveCall`), and `decide` leaves a call of the caller itself to
  the copies `walk::SelfInline` makes (gcc's `recursive_inlining`,
  ipa-inline.cc:1645: depth `MAX_INLINE_RECURSIVE_DEPTH` 3 against gcc's 8,
  `MAX_INLINE_INSNS_RECURSIVE` 150 against 450, the call's frequency above
  10%; none at -Os; calls of a function that reads and writes nothing with
  the same arguments count once; only a tree, at least one self-call per
  call). They follow ipa-cp's second round, as the pin's specializer
  precedes its inliner (PassBuilderPipelines.cpp:1228 against :1331), and so
  does the tail-call elimination of such a function (the pin: inside its
  function simplification, :688). Why: -O2 clocks/bytes, main | this: fib
  227171/537 | 154398/392, rectwo 612458/538 | 597725/552, recchop -O3
  5946/1450 | 5932/5113; gcc's 8 and 450 cost 3-10x main's bytes. Owner:
  w-6. Exit: the copies chosen by gcc's badness heap with its growth limits,
  tail-call elimination at the pin's slot; open rows hanoi, bintree,
  quicksort, x_mergesort, recsum (docs/architecture/recursion-rows.md). A
  recursive callee inlined into another function is also peeled
  (ipa-inline.cc:1033): its own calls inlined in the copy while the calls of
  it to itself per call, to the power of the copies deep, are below 1 - 1/8
  (squared per copy where the body has more than one call to itself), a copy
  adds at most 15 `gcc_insns` and the copies of a call at most 450; a tree
  is never peeled; not at -Os.

## See also
[`crate::inline`](inline.md), the decision this is to replace; the plan is
the cgscc plan page in docs/architecture.
