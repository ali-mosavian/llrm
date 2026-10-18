> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/decide.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/decide.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/decide.rs sha256:6a412e7491068d40754047ae4f293a6970408878b79c0fa31caed225ba2ebd79 -->

# decide

Branches whose way is known, decided: LLVM's JumpThreading and SimplifyCFG
on known conditions.

## How it works
`decided` runs three steps, then `cfg::merged` joins the chains that result.

1. Threading. A block holding only a `br` is bypassed (`_threaded`): each
   branch or switch edge to it goes to what it jumps to, where that has no
   phis to tell the edges apart. A loop's one entry edge, one latch and
   dedicated exits are kept, so the loop stays in simplify form. A
   conditional branch whose two arms go one way becomes a jump. Separately
   (`_phi_threaded`), a predecessor that only jumps to a block holding just
   a phi and a branch on it, and gives the phi a constant, goes straight to
   where that constant leads (LLVM's `ProcessBranchOnPHI`); a loop header
   keeps its edges.
2. Deciding. For each block ending in a `br i1` or a `switch`, the way is
   found from, in order: `consts`' answer for the condition, or for the two
   operands of the `icmp` it is, carried across loops by `constant_cycles`
   along only the edges that run; a pointer compared with null that
   points-to facts say is non-null; the intervals `ranges` has on each edge,
   if exactly one successor is possible; the guards that hold at the block
   and the loops around it (`guards::Given`, over `induction` terms). A
   switch is decided by a known condition with distinct cases.
3. The decided terminator is replaced by a `br` to the chosen block; blocks
   no longer reachable go and phis left with one value are replaced by it.


```text
   b0: %c = icmp ult i16 3, 5          b0: br b1
       br i1 %c, b1, b2          ==>   b1: ...
   b1: ...                             (b2 and its phi arms are gone)
   b2: ...
```

## Example
Before:

```mir
define i16 @f(i16 %x) {
b0:
  %k = add i16 2, 3
  %c = icmp slt i16 %k, 10
  br i1 %c, label %b1, label %b2

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
  %k = add i16 2, 3
  %c = icmp slt i16 %k, 10
  br label %b1

b1:
  ret i16 %x
}
```

`5 < 10` is known, so the branch is a jump to `b1` and `b2` is gone; `dead`
removes `%c` later.

From unit test `test_a_branch_on_known_numbers_is_a_jump` in
`decide_tests.rs`.

## Runs
Once per pass of the function fixed point, after `fold` and `ports` and
before `phiopt`. `Options::decide` turns it off (no `-f` flag); nothing runs
at -O0. `jumpthread` is the separate pass that threads a state variable's
edges.

## ScalarEvolution
A branch the other engines leave is asked of ScalarEvolution
(`isKnownPredicateAt` at the branch, then its inverse), then of the `guards`
engine for what it cannot prove (the partition index: not an add
recurrence). `LLRM_NO_SCEV_DECIDE` asks the `guards` engine alone.

## Needs
`consts`, `constant_cycles`, LazyValueInfo (a branch the dominating
branches, assumes and `!range`s settle), `ranges` (the counted loops' facts,
until indvars decides them as LLVM does), `guards`, `induction`, points-to
(`pointers`), the CFG shape.

## Keeps
Blocks and edges change: every analysis is invalidated when anything was
decided or merged.

## See also
[`crate::cfg`](../analyses/cfg.md), [`crate::fold`](fold.md), [`crate::jumpthread`](jumpthread.md); LLVM JumpThreading,
SimplifyCFG.
