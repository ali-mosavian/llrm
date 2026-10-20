> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/decide.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/decide.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/decide.rs sha256:affa89cb0194c780d56953100e8b1674f936d9c164f5f88adbdfc09a205ed12e -->

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
at -O0. `jumpthreading` and `dfajumpthreading` are the separate passes that
thread edges.

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
[`crate::cfg`](../analyses/cfg.md), [`crate::fold`](fold.md), [`crate::jumpthreading`](jumpthreading.md); LLVM
JumpThreading, SimplifyCFG.
