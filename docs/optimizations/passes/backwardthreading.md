> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/backwardthreading.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/backwardthreading.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/backwardthreading.rs sha256:ac70437815d1593ae5d33c18c6f83a330aa32671863898c856f9c818aa7f73c2 -->

# backwardthreading

Backward jump threading: a
branch whose outcome one path into its block decides is threaded for that
path, though the block's other paths leave it unknown.

GCC's
[`tree-ssa-threadbackward.cc`](https://gcc.gnu.org/git/?p=gcc.git;a=blob;f=gcc/tree-ssa-threadbackward.cc)
walks back from a conditional branch along a path until the branch's inputs
are pinned down, then copies the path's blocks so that path jumps straight
to the arm. LLVM's `JumpThreading` asks LVI about each edge into the
branch's block, which merges all paths into the predecessor, so a
predecessor that is a join of a known and an unknown path is never threaded.

Here a path is a predecessor `q` of a join `p` that leads to `bb` (through
up to `MAX_CHAIN` blocks). The facts at `q` (the branches that dominate it,
what every way into a block knows, and its own edge to `p`) are read as
ranges of the values `bb`'s compare reads; when they decide the compare and
the facts at `p` do not, the blocks and `bb` are copied for the edge `q ->
p`, the copy of `bb` jumps to the arm, and SSA form is restored.

```text
  q: ...  (knows x > 0)             q: ...
  p: join of q and r                p': copy of p        p: join of r alone
  bb: br (x > 0), A, B       ==>    bb': copy of bb, jumps to A
```

Relations (`Tuning::threads_by_relation`, DEVIATE BACKTHREAD-REL): the
compares of one pair of names that hold on a path decide the others of that
pair, as GCC's relation oracle (`value-relation.cc`, read by
`gimple-range-path.cc` in `pass_thread_jumps_full`) does: `a < b` taken
makes `a <= b` taken and `a > b` not (`Orders`; signed and unsigned orders
apart). A join that holds the branch itself is threaded for such a path
(`join_decided`), which collapses a ladder of compares of one pair. The pass
also runs before the first SimplifyCFG that makes selects of the branches
(`backthread@1243`).

DEVIATE (no LLVM pass; GCC `tree-ssa-threadbackward.cc`): a path is `q`,
at most `MAX_CHAIN` (3) blocks and `bb` (GCC's `max-fsm-thread-length` is
10); the facts are the ranges of branch edges that dominate `q` and what
every way into a block agrees on (`entered_with`; GCC asks ranger); the path
may not copy more than `max-jump-thread-duplication-stmts` (15)
instructions; a function with `optsize` or `minsize` is left alone.

## Example
```text
  q: x > 0 known; p: join of q and r; bb: br (x > 0), A, B
  =>  q jumps through the copies p', bb' straight to A; r still goes through p and bb
```

## Used by
Named by `pipeline`.
