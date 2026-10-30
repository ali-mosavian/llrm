> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/decide.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/decide.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/decide.rs sha256:7471fa079f8be976d5e8af3111779ea2ceeb950b0a9ec6258ede4200c20573c1 -->

# decide

Branches whose way is known, decided: LLVM's JumpThreading and SimplifyCFG
on known conditions.

## How it works
`decided` runs two steps. Threading is `jumpthreading`'s and
`simplifycfg`'s.

1. Deciding. For each block ending in a `br i1` or a `switch`, the way is
   found from, in order: `consts`' answer for the condition, or for the two
   operands of the `icmp` it is, carried across loops by `constant_cycles`
   along only the edges that run; a pointer compared with null that
   points-to facts say is non-null; the intervals `ranges` has on each edge,
   if exactly one successor is possible; the guards that hold at the block
   and the loops around it (`guards::Given`, over `induction` terms). A
   switch is decided by a known condition with distinct cases.
2. The decided terminator is replaced by a `br` to the chosen block; blocks
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
  %c = icmp slt i16 5, 10
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
  %c = icmp slt i16 5, 10
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
Once per pass of the function fixed point, after `fold` and `ports`.
`Options::decide` turns it off (no `-f` flag); nothing runs
at -O0. `jumpthreading` and `dfajumpthreading` are the separate passes that
thread edges.

## ScalarEvolution
None. A branch only a loop's trip count settles is `indvars`'s
(`eliminateIVComparison`, on the loops an unroll made too: `indvars@753`),
as in LLVM; the `guards` engine answers what it can from the guards over
the branch (the partition index: not an add recurrence).

## Needs
`consts`, `constant_cycles`, LazyValueInfo (a branch the dominating
branches, assumes and `!range`s settle), `ranges` (the counted loops' facts,
until indvars decides them as LLVM does), `guards`, `induction`, points-to
(`pointers`), the CFG shape.

## Keeps
Blocks and edges change: every analysis is invalidated when anything was
decided or merged.

## See also
[`crate::cfg`](../analyses/cfg.md), [`crate::fold`](fold.md), [`crate::jumpthreading`](jumpthreading.md); LLVM
JumpThreading, SimplifyCFG.
