> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/jumpthreading.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/jumpthreading.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/jumpthreading.rs sha256:5c583160f5fcbf2b96b17ec7e3152f5238778be549907b9a6931b94472524da7 -->

# jumpthreading

Jump threading: a block that ends in a branch or switch on a condition some
of its predecessors decide is bypassed for those predecessors, which jump
straight to the target; as LLVM's `jump-threading`.

## How it works
A translation of LLVM `d1106deb71cc` `JumpThreading.cpp`, one Rust function
per C++ function, each linking its line. This is stage S1 of
`docs/architecture/jumpthreading-translation.md`: `runImpl`,
`findLoopHeaders`, `processBlock`, `computeValueKnownInPredecessors`,
`processThreadableEdges`, `tryThreadEdge`, `threadEdge`, `splitBlockPreds`,
`cloneInstructions`, `updateSSA` and `maybeMergeBasicBlockIntoOnlyPred`.
For each block the pass folds a terminator whose condition is a constant,
asks what the condition is on each edge into the block (a `phi` of
constants, `LazyValueInfo` on the edge, a compare of a `phi`, `and`/`or` of
known parts, a `select`), and copies the block (without its terminator)
for the predecessors that agree on where it goes next: the copy jumps to
that target and the predecessors jump to the copy.

```text
  p1: br b            p1: br b.thread       b.thread: ..; br t
  p2: br b      ==>   p2: br b              b: %c = phi [1, p1] [%x, p2]
  b:  %c = phi [1, p1] [%x, p2]                 br i1 %c, t, e
      br i1 %c, t, e
```

DEVIATE (J1..J9; stages S2-S4 are not ported yet, which is not a deviation
of the finished pass):
- J1. No `indirectbr` or `blockaddress` in MIR: `processBlock`'s
  `IndirectBrInst` arm, `hasAddressTakenAndUsed` and the block-address
  preference of `getKnownConstant` are absent.
- J2. No guards: `processGuards` is skipped; no input has
  `llvm.experimental.guard`.
- J3. No `!prof`, `BlockFrequencyInfo` or `BranchProbabilityInfo` updates
  (`updateBlockFreqAndEdgeWeight`, `getBPI`).
- J4. `DomTreeUpdater` is not used: the pass returns `none()` when it
  changed anything and the manager rebuilds the dominator tree. The
  reachable set at the start is a walk from the entry.
- J5. The sweep visits the blocks of the layout as it stood when the sweep
  began; a block made during it is met by the next sweep (LLVM's list
  iterator meets one placed after the current block in the same sweep).
- J6. `fcmp` is not looked through (`CmpInst` is integer compares only); MIR
  has no vectors.
- J7. Debug records are not retargeted (`DbgVariableRecord`s in `updateSSA`
  and `cloneInstructions`); `clone_instruction` carries the records of the
  original.
- J8. No `convergent` tokens (`HasLoopOrEntryConvergenceToken`), no noalias
  scopes to clone (`cloneNoAliasScopes`), no source atoms.
- J9. LVI is the pass's own `LazyValueInfo` (as `Cvp`'s), not the one
  function analysis the manager would share between the passes.
- Not yet ported (later stages): `tryToUnfoldSelect*`, `processBranchOnPHI`,
  `processBranchOnXOR`, `maybethreadThroughTwoBasicBlocks`,
  `duplicateCondBranchOnPHIIntoPred`, `processImpliedCondition`,
  `simplifyPartiallyRedundantLoad`.

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
