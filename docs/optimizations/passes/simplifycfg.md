> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/simplifycfg.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/simplifycfg.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/simplifycfg.rs sha256:692039155fe4bfd4f977a4421bd0d24f99b7c7bf905a4fca0a2661740c03211b -->

# simplifycfg

A terminator on a constant becomes a jump, the blocks nothing reaches go,
and what that leaves is merged: LLVM's `SimplifyCFGPass` and the
`ConstantFoldTerminator`, `removeUnreachableBlocks` and
`BasicBlock::removePredecessor` it is made of.

## How it works
`simplified` is `simplifyFunctionCFGImpl`:

1. `unreachable_removed` walks the body from the entry. Each block it
   reaches has its terminator folded first (`terminator_folded`), so only
   the successor that still runs is followed; the blocks it did not reach
   are deleted, with their edges out (`removePredecessor`) and every use of
   what they defined replaced by poison.
2. `iteratively_simplified` visits the blocks in layout order until a visit
   changes none (`iterativelySimplifyCFG`). A visit (`block_simplified`,
   LLVM's `simplifyOnce`) deletes a block with no predecessor (or only
   itself), folds its terminator, removes the duplicate phis of the block
   and merges it into its one predecessor.
3. When step 2 changed anything, unreachable blocks are removed again and,
   if that removed any, step 2 and the removal alternate until neither does.

`terminator_folded` replaces

- `br i1 %c, X, X` by `br X`, taking one phi input of `X` away;
- `br i1 true/false, A, B` by a jump to the arm that runs;
- a `switch` on a constant by a jump to the case it takes (or the default),
  a `switch` whose cases all go to one block by a jump, and a case that goes
  where the default goes is dropped; a switch left with one case is a `br`
  on an `icmp eq`.

The condition that nothing reads afterwards is deleted with what only it
read. The new `br` carries `!llvm.loop`.

```text
  b0: br i1 true, b1, b2         b0: br b1
  b1: ...                  ==>   b1: ...        (b2, and its phi inputs
  b2: %p = phi [%x, b0], ..                      from b0, are gone)
```

## Example
Before:
```mir
define i16 @f(i16 %x) {
b0:
  br i1 true, label %b1, label %b2

b1:
  ret i16 %x

b2:
  ret i16 0
}
```
After:
```mir
define i16 @f(i16 %x) {
b0:
  ret i16 %x
}
```
`b2` is no longer reached, so it goes; `b1` has one predecessor, so it is
merged into `b0`.

From unit test `test_a_branch_on_true_is_a_jump_and_the_dead_arm_goes` in
`simplifycfg_tests.rs`.

## LLVM structure
[`SimplifyCFGPass`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L272)
with the options of the early function pipeline slot
([`PassBuilderPipelines.cpp`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Passes/PassBuilderPipelines.cpp#L1178):
the defaults). `MIRROR` unless a line below says `DEVIATE`.

| LLVM | here | |
|---|---|---|
| [`simplifyFunctionCFGImpl`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L272) | `simplified` | MIRROR |
| [`iterativelySimplifyCFG`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimplifyCFGPass.cpp#L229) | `iteratively_simplified` | MIRROR; D1 |
| [`SimplifyCFGOpt::simplifyOnce`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/SimplifyCFG.cpp#L9303) | `block_simplified` | partly: the steps up to the merge; the rest is not ported yet (D2) |
| [`removeUnreachableBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L2912) and `markAliveBlocks` | `unreachable_removed` | the terminator folding and the deletion; D3 |
| [`ConstantFoldTerminator`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L133) | `terminator_folded` | MIRROR; D4 |
| [`BasicBlock::removePredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/IR/BasicBlock.cpp#L446) | `predecessor_removed` | MIRROR (one phi input an edge) |
| [`DeleteDeadBlocks`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L94) | `dead_blocks_deleted` | MIRROR |
| [`EliminateDuplicatePHINodes`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/Local.cpp#L1527) | `transform::duplicate_phis_in` | MIRROR |
| [`MergeBlockIntoPredecessor`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/BasicBlockUtils.cpp#L237) | `cfg::merged_into_predecessor` | MIRROR |

- D1. LLVM's `LoopHeaders` (the targets of back edges, found once) are not
  computed: nothing ported so far reads them. They come with the first step
  that does (`simplifyUncondBranch`, `NeedCanonicalLoop`).
- D2. Not ported yet, each a later change of this series:
  `tailMergeBlocksWithSimilarFunctionTerminators`,
  `removeUndefIntroducingPredecessor`, `sinkCommonCodeFromPredecessors` (off
  at this slot), `foldTwoEntryPHINode` (`phiopt` meanwhile), and the
  terminator simplifications `simplifyUncondBranch`, `simplifyCondBranch`,
  `simplifySwitch`, `simplifyUnreachable`.
- D3. `markAliveBlocks` also turns what follows a call of a `noreturn`
  function, `llvm.assume(false)` and a store to `undef` into `unreachable`
  (`FoldInstsToUnreachable`): not ported yet. It reads the target's
  `NullPointerIsDefined`, which the MIR has no flag for. `invoke`,
  `catchswitch` and `indirectbr` do not exist in the MIR.
- D4. No branch weights: the MIR has none, so the case weights
  `ConstantFoldTerminator` merges and the `!prof` it sets are not kept.
  `!make.implicit` is not a MIR kind. There is no `DomTreeUpdater`; as at
  LLVM's default (`simplifycfg-require-and-preserve-domtree` off, where the
  pass is given no `DominatorTree`) the analyses are invalidated when
  anything changed: that is a MIRROR.

## Runs
Once per round of the function fixed point, before `decide`: the branches
on a literal that `fold` just made are jumps before `decide` asks for the
facts of the body. Nothing runs at -O0.

## Measured
The C corpus (`identical.py`: 395 sources, -O1/-O2/-Os/-Omax, 1,580
compiles): `decide` decides 646 branches instead of 3,002; the ones on a
literal or a constant `switch` go from 2,464 to 153 (the rest are made by
`jumpthread`'s own cleanup, after this pass), and it runs 32,640 times
instead of 33,932. Of the 1,583 objects 40 differ in bytes, 36 only in label
numbers; of the rest, `bintree` -O3 is 7 instructions longer (twice) and
`switch_empty_cases` 2 shorter (twice). The 140 bench rows at -O2 and -Os of
the 14 programs that differ have every counter unchanged.

## Keeps
Blocks and edges change: every analysis is invalidated when anything
changed.

## See also
[`crate::cfg`](../analyses/cfg.md), [`crate::decide`](decide.md).
