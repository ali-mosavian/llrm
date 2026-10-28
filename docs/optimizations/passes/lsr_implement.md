> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/lsr_implement.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/lsr_implement.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/lsr_implement.rs sha256:9980e10727b6036c01bcd5ba9559dec19346821b134f5aa74ea4ce27902de8f3 -->

# lsr_implement

Making LLVM's loop strength reduction's solution (`Expand`, `Rewrite`,
`RewriteForPHI`, `AdjustInsertPositionForExpand`, `HoistInsertPosition` and
`ImplementSolution`; LoopStrengthReduce.cpp at `d1106deb71cc`, l.5712-6306).

## How it works
For each fixup of each use, the chosen formula is expanded by the SCEV
expander into instructions, at a point that every operand dominates and that
is as high in the dominator tree as the loop nest allows (so expansions
share), and the fixup's operand is replaced by it. A compare against zero
has its other operand set to the negated offset or scaled register. A phi's
operand is rewritten in its incoming block. What the replaced operands leave
unread goes; increments of induction variables LSR reused move to the
latch.

## Example
The store's operand `getelementptr i32, %buf, %i` becomes a phi of a pointer
that advances by 4; the compare's `%next == %n` becomes `%counter == 0` on a
counter that falls by 1.

## Runs
Behind `LLRM_EXP_LSR` ([`crate::lsr_pass`](lsr_pass.md)).

## Limits
DEVIATE: the critical edges to a phi that a fixup rewrites are split before
the loop is analysed (`split_phi_edges`), not in `RewriteForPHI`: the
analyses hold the CFG while they are read. Post-indexed addressing and IV
chains are not ported (see [`crate::lsr_collect`](lsr_collect.md)).

DEVIATE LU5: an exit compare's limit is an existing narrow value whose
zero-extension is the register the formula names (`narrow_existing`, the
offset-free case: `narrow_original`), not the wide register expanded and
truncated back; lcexit.bas m16 -Os -21 B, one other of 258 programs changes.
