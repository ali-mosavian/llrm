> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dfajumpthreading.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dfajumpthreading.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dfajumpthreading.rs sha256:0d2e98ed60300f975ae7f9cf266b6899ebdf47ca465bea996388e20134f48c41 -->

# dfajumpthreading

DFA jump threading: a loop whose `switch` runs on a state variable that
each path of the loop sets to a constant, as the body of a state machine
does, is threaded: every such path gets its own copy of the blocks from
where the state is set to the `switch`, and the copy of the `switch` is a
jump to the case the constant picks; as LLVM's `dfa-jump-threading`.

## How it works
A translation of LLVM `d1106deb71cc` `DFAJumpThreading.cpp`, one Rust
function per C++ function, each linking its line; the plan and the stages
are `docs/architecture/dfajumpthreading-translation.md`. This is stage S1.
[`mainswitch`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dfajumpthreading/mainswitch.rs) decides whether a `switch` is a candidate (`MainSwitch`):
its condition is a `phi` in a loop and what flows into it is constants or
values nothing predicts. [`paths`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dfajumpthreading/paths.rs) enumerates the paths that set the state
to a constant (`AllSwitchPaths`), bounded by the pin's three caps.
[`transform`](transform.md) decides whether copying is legal and profitable and does it
(`TransformDFA`): the blocks of each path from the determinator on are
cloned per exit value, the predecessors are redirected to the clones, and
SSA is restored.

```text
  p1: s = 1 --\                       p1: s = 1 --> sw.jt1: br c1
  p2: s = 2 ---+--> det: phi s        p2: s = 2 --> sw.jt2: br c2
                    sw: switch s      det, sw stay for the paths nobody decided
```

DEVIATE (DF1..DF3; DF5 is a parity note):
- DF1. A `select` in the use-def chain of the state rejects the switch
  (`isCandidate` would accept it and `unfold` it into control flow):
  `unfold` is stage S2.
- DF2. No `DomTreeUpdater`: the pass returns `none()` when it threaded
  anything and the manager rebuilds the dominator tree and the loop info
  (the pin keeps the dominator tree, and the loop info unless it cloned);
  `updateSSA` reads a dominator tree built after the threading. No noalias
  scopes to clone (J8 of `jumpthreading.rs`), no optimization remarks,
  `!prof` is not carried.
- DF3. A path through a block that starts with a `landingpad` is not
  threaded: the copy would be a second pad, and the one module handler
  (`ehprepare`) cannot choose between two (J10 of `jumpthreading.rs`).
- DF5 (parity, not a deviation). `x_switch` m16 -O2/-O3 is +17 B (320 vs
  303) and 70 vs 63 asm lines: the pass threads the state cycle and peels
  two iterations, as `opt -passes=dfa-jump-threading` does on the same
  entering IR (m16 target, identical per `difftest.py dumps`). The source is
  UB at m16 (`r >> 16` on 16 bits); -Os is identical (size gate).

## Example
The state is set to 1 on one path and to 2 on the other, and the `switch`
after the join picks the case from it:
```mir
entry:
  br i1 %c, label %p1, label %p2
p1:
  br label %det
p2:
  br label %det
det:
  %s = phi i16 [ 1, %p1 ], [ 2, %p2 ]
  switch i16 %s, label %d [ i16 1, label %c1 i16 2, label %c2 ]
```

## Used by
`pipeline` (`dfajumpthreading@806`), a `Partial` slot of the parity
registry; `llrm-pass dfajumpthreading`.

## See also
[`DFAJumpThreadingPass::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/DFAJumpThreading.cpp#L1561).
