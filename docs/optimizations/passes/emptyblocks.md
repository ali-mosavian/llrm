> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/emptyblocks.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/emptyblocks.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/emptyblocks.rs sha256:2a67fc1ad80e2ef603ad7a40a65e9dfcb1e984e70aa3a53cad66d49d06a02503 -->

# emptyblocks

CodeGenPrepare's `eliminateMostlyEmptyBlocks`: a block of phis and a jump
is merged into its successor before isel
([CodeGenPrepare.cpp#L939](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/CodeGen/CodeGenPrepare.cpp#L939)).

## How it works
Passes before isel (LSR, loop-simplify) split edges in ways that cost a
jump each; SimplifyCFG keeps those that hold a loop's simple form up
(`loop_form_blocks`), and CodeGenPrepare takes them out again just before
isel. For each block but the entry (`findDestBlockOfMergeableEmptyBlock`):
it ends in an unconditional jump to another block, holds nothing but phis
before it, and merges with that block without a phi conflict
(`canMergeBlocks`: a block both reach must give the successor's phis one
value). `isMergingEmptyBlockProfitable` keeps a loop's preheader whose
predecessor has other exits, and the target of a `callbr`. The merge is
`eliminateMostlyEmptyBlock`: the successor's phis take the block's
predecessors in its place, and the block goes.

```text
 b1: ...; br label %b2            b1: ...; br label %b3
 b2: br label %b3          ==>    b3: %p = phi [ %x, %b1 ], ...
 b3: %p = phi [ %x, %b2 ], ...
```

MIRROR: the dead-phi sweep, `findDestBlockOfMergeableEmptyBlock`,
`canMergeBlocks`, the preheader rule and the `callbr` rule of
`isMergingEmptyBlockProfitable`.

DEVIATE ECB1: the last rule of `isMergingEmptyBlockProfitable` (a switch or
indirect jump's only successor to a phi block, kept by the ratio of block
frequencies) always merges: MIR has no block frequency here. Exit: the
frequency analysis (BlockOdds), owner w-4, 2026-12-02.

DEVIATE ECB2: the merge itself is `simplifycfg::empty_block_removed`
(`TryToSimplifyUncondBranchFromEmptyBlock`), which does the same to the
phis; it refuses a merge that would add more than 1000 phi entries and one
whose loop metadata the predecessors already carry, which
`eliminateMostlyEmptyBlock` does not. Exit: none.

## Example
`test_an_empty_block_before_a_join_is_merged_into_the_join` in
`emptyblocks_tests.rs` is the sketch above.

## Runs
First in `cgpaddr`'s run, before the sinks: it changes the CFG.
