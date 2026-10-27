> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopdeletion.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopdeletion.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopdeletion.rs sha256:75662ef99a01cb508008ecff76125ab7901da5b2955823cb6f0f2d31424ac10e -->

# loopdeletion

`LoopDeletion`: a loop that computes nothing the rest of the function reads
is removed, and a loop whose backedge is never taken is made straight code.

## How it works
For each loop, innermost first, `delete_loop_if_dead` needs a preheader and
dedicated exits. A loop the preheader's predecessors never enter (each ends
in a constant branch away from it) is deleted at once, its exit phis made
poison. Otherwise the loop must have at most one exit block and be dead
(`is_loop_dead`): the values its exit phis take are the same from every
exiting block and loop-invariant (they are hoisted to the preheader where
they can be), no instruction has a side effect, and the loop ends: the
function is `mustprogress`, or each loop in it is marked so or has a
constant bound on its backedge count from ScalarEvolution.

Deleting makes the preheader jump to the exit, gives each exit phi the one
value from the preheader, and erases the loop's blocks.

A loop that stays gets `break_backedge_if_not_taken`: if its backedge count
is zero, or its first iteration provably leaves
(`can_prove_exit_on_first_iteration`: the blocks and edges live on the
first iteration, found by evaluating the header phis on entry), the latch no
longer jumps back, and the loop is straight code.

## Deviations
- LD1 `makeLoopInvariant` keeps an instruction's `prof` metadata wherever it
  was, where LLVM keeps it only for an instruction of the header
  ([`crate::twoentryphi::metadata_dropped`](twoentryphi.md)).
- LD2 No `MemorySSA`, optimization remarks or debug records: the records of
  a deleted loop are dropped, not moved to the exit.
- LD3 After a loop with an outer loop loses its backedge, the whole function
  is put in LCSSA form again, where LLVM does `formLCSSARecursively` of the
  outermost loop.
- LD5 The blocks of deleted loops are erased together when the pass ends.
- LD4 The analyses are derived again after a change that alters the loops
  around it; after the deletion of a loop with no loop around or inside it
  they are kept (made again after each, `n` deletions in a function were
  `n^2`).

## Example
A loop that only counts, read by nobody:
```mir
b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 10
  br i1 %go, label %b1, label %b2
```
becomes a jump from `b0` to `b2`.

## See also
[`LoopDeletionPass::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopDeletion.cpp#L516),
[`deleteLoopIfDead`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopDeletion.cpp#L436),
[`deleteDeadLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L521),
[`breakLoopBackedge`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopUtils.cpp#L746).

## Used by
Named by `loopinstsimplify`, `loopsimplifycfg`, `nontrivial`, `pipeline`.
