> Generated from the `//!` header of [`crates/opt/llrm-analysis/src/reachability.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-analysis/src/reachability.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-analysis/src/reachability.rs sha256:db92721ee25830d0f2862d31bb04f75aa423b6620acd69d3eafe929906789b8c -->

# reachability

`isPotentiallyReachable` of `CFG.cpp` at d1106deb71cc: whether a path may
lead from one instruction or block to another.

## How it works
Within one block, `a` reaches `b` if it comes first. Otherwise the walk
goes from `a`'s successors (or from `a`'s block) over the graph and answers
true at `b`'s block, or at a block that dominates it. The walk visits at
most `MAX_BBS_TO_EXPLORE` blocks and then answers "maybe", so a caller
that asks of every capture pays a bounded price for each.

DEVIATE R1: no `LoopInfo`, `CycleInfo` or exclusion set. A block in a cycle
reaches an earlier instruction of itself only through the walk, which is
the same answer, found by exploring. MIR here has no `CycleInfo`; the
dominator tree is the only extra input.

## Example
In a diamond, neither arm reaches the other; in a loop the body reaches
itself:

```mir
define void @f(i1 %c) {
entry:
  br i1 %c, label %left, label %right
left:
  br label %join
right:
  br label %join
join:
  ret void
}
```

From unit test `a_diamond_arm_does_not_reach_the_other_arm` in
`reachability_tests.rs`.

## Provides
`is_potentially_reachable`, `is_potentially_reachable_blocks`,
`is_potentially_reachable_from_many`.

## See also
LLVM `CFG.cpp`, [`crate::capturetracking`](capturetracking.md).
