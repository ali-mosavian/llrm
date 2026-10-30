> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/jumpthreading.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/jumpthreading.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/jumpthreading.rs sha256:4f627982a1c3000c27b442482ab0ad4c5554ffec58a7d0db696a5cdc15569243 -->

# jumpthreading

Jump threading: a block that ends in a branch or switch on a condition some
of its predecessors decide is bypassed for those predecessors, which jump
straight to the target; as LLVM's `jump-threading`.

## How it works
A translation of LLVM `d1106deb71cc` `JumpThreading.cpp`, one Rust function
per C++ function, each linking its line. This is stages S1 to S4 of
`docs/architecture/jumpthreading-translation.md`: `runImpl`,
`findLoopHeaders`, `processBlock`, `computeValueKnownInPredecessors`,
`processThreadableEdges`, `tryThreadEdge`, `threadEdge`, `splitBlockPreds`,
`cloneInstructions`, `updateSSA`, `maybeMergeBasicBlockIntoOnlyPred`,
`processBranchOnPHI`/`XOR`, `duplicateCondBranchOnPHIIntoPred`,
`maybethreadThroughTwoBasicBlocks`, the select unfolding and
`processImpliedCondition` and `simplifyPartiallyRedundantLoad`
(`loadpre.rs`). For each block the pass folds a terminator whose condition
is a constant, asks what the condition is on each edge into the block (a
`phi` of constants, `LazyValueInfo` on the edge, a compare of a `phi`,
`and`/`or` of known parts, a `select`), and copies the block (without its
terminator) for the predecessors that agree on where it goes next: the copy
jumps to that target and the predecessors jump to the copy.

```text
  p1: br b            p1: br b.thread       b.thread: ..; br t
  p2: br b      ==>   p2: br b              b: %c = phi [1, p1] [%x, p2]
  b:  %c = phi [1, p1] [%x, p2]                 br i1 %c, t, e
      br i1 %c, t, e
```

DEVIATE (J1..J15):
- J1. No `indirectbr` or `blockaddress` in MIR: `processBlock`'s
  `IndirectBrInst` arm, `hasAddressTakenAndUsed` and the block-address
  preference of `getKnownConstant` are absent.
- J2. No guards: `processGuards` and `threadGuard` are skipped. A guard call
  needs exactly one `"deopt"` operand bundle (`opt`'s verifier rejects it
  without), MIR has no operand bundles, and no front end emits the
  intrinsic: a well-formed guard cannot reach the pass, and `opt` has
  nothing to compare a translation with.
  `DuplicateInstructionsInSplitBetween` is not translated either.
- J3. No `!prof`, `BlockFrequencyInfo` or `BranchProbabilityInfo` updates
  (`updateBlockFreqAndEdgeWeight`, `updatePredecessorProfileMetadata`,
  `getBPI`, `getBFI`). The pin keeps BPI and BFI up to date as it threads
  and does not preserve them; what outlives the pass is the branch weights
  it writes, and only where a threaded block's terminator has valid ones. No
  front end writes `!prof`; `loopversion`'s dispatch branch (-O3) does,
  `window` after this pass. `difftest.py` leaves metadata out of the
  comparison, so the weights cannot be read against `opt` there.
- J4. `DomTreeUpdater` is not used: the pass returns `none()` when it
  changed anything and the manager rebuilds the dominator tree. The
  reachable set at the start is a walk from the entry.
- J5. The sweep walks the live layout as LLVM walks its block list: a block
  placed after the current one is met in the same sweep, and the walk finds
  its place again by position (`next_in_layout`) when a block is removed.
- J6. `fcmp` is not looked through (`CmpInst` is integer compares only); MIR
  has no vectors.
- J7. Debug records are not retargeted (`DbgVariableRecord`s in `updateSSA`
  and `cloneInstructions`); `clone_instruction` carries the records of the
  original.
- J8. No `convergent` tokens (`HasLoopOrEntryConvergenceToken`), no noalias
  scopes to clone (`cloneNoAliasScopes`), no source atoms.
- J9. LVI is the pass's own `LazyValueInfo` (as `Cvp`'s), not the one
  function analysis the manager would share between the passes.
- J10. A block that starts with a `landingpad` is not threaded: the copy
  would be a second pad, and the one module handler (`ehprepare`) cannot
  choose between two.
- J11. `LVI->getConstantOnEdge` answers that are not integers are dropped
  (`known_from_lattice`), where `evaluateOnPredecessorEdge` would pass a
  pointer constant on to `ConstantFoldCompareInstOperands`.
- J12. Switched on, `recpow.c` m16 -O2 compiles 89.26 -> 105.41 M
  instructions:u (+18.1%, A/A repeatable): the pass is about 1 M of the
  extra work, the rest is a bigger body the old clocks inliner
  (`ipa-recursive.`, `ipa-peel.`) chooses to inline. Exit: W2's
  `inlinecost::decide` lands.
- J13. `queens` -O3 runs 96949 -> 123141 clocks (+27%; instructions 78977 ->
  106279, memory ops 12911 -> 25877). Our output equals `opt`'s. A/B on the
  #2828 head, queens -O3 m32: forcing every ipa-cp candidate to clone (a
  throwaway switch skipping the verdict test in `ipacp.rs`) with it on gives
  93729 clocks / 80393 instructions, below the off figure: the clone
  decision is the lever. `_place` is 24 (on) vs 26 (off) in size; the
  call-site frequency per clone level is 9920, 36448, 337548, 3254456,
  31512005 off (about x10 each) and 12124, 13976, 6377, 481, 8 on; the test
  `benefit * 1000 >= EVAL_THRESHOLD (500) * size` fails at the sixth clone
  (benefit 2, 12 needed), so `.6`..`.8` are never made. Our block
  frequencies equal `opt`'s `print<block-freq>` on the same IR, on and off
  (stage 11 and the stage-12 clones): the estimator is not at fault, the
  threaded bodies make the call sites cold. Owner: cycles-queue (ipa-cp
  clone verdict). Exit: W2's `inlinecost::decide` lands.
- J14. `hanoi` -O2 runs 109 -> 114 clocks. Our output equals `opt`. With it
  on, `_hanoi` does `push ecx; mov ecx, eax` before the `n == 0` test, so
  the early return pays frame setup (shrink-wrap) and the constant copy sits
  before the test (pre-RA MachineSink). Exit: the ShrinkWrap port
  (backend-two) and the MachineSink translation (lir-cleanup-port).
- J15. `simplifyPartiallyRedundantLoad`: a load is never atomic in MIR
  (`isUnordered` is `!volatile`), debug locations are not set on the new
  instructions (J7), and `CanSpeculateInto` asks the LICM translation of
  `isSafeToSpeculativelyExecute` (a load is safe where its bytes are
  dereferenceable or the target does not trap), which does not read the
  context instruction (`Pred->getTerminator()`).

Rows pair at S3, before the load PRE, one binary, on against off
(feat/parity 846c0c0c6): clocks geomean llrm/min(gcc,clang) O2 .9149 ->
.9161 (+0.1%), O3 .8851 -> .8889 (+0.4%), Os .9280 -> .9333 (+0.6%); 13 rows
worse, 16 better. Code bytes m16 -184/-286/-401 (O2/O3/Os), m32
-470/-890/-152.

## Example
`%c` is `true` from `p1`, so `p1` skips `b`:
```mir
p1:
  br label %b
b:
  %c = phi i1 [ true, %p1 ], [ %x, %p2 ]
  br i1 %c, label %t, label %e
```

## Used by
`pipeline` (`jumpthreading@668`, `jumpthreading@809`), the `Partial`
slots of the parity registry.
