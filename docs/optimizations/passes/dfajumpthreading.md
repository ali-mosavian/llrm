> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dfajumpthreading.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dfajumpthreading.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dfajumpthreading.rs sha256:a3b3d2bb4211be6aa2a2060d2559d1e726dcdecd053f106f1bb2ff2939bf1c3f -->

# dfajumpthreading

DFA jump threading: a loop whose `switch` runs on a state variable that
each path of the loop sets to a constant, as the body of a state machine
does, is threaded: every such path gets its own copy of the blocks from
where the state is set to the `switch`, and the copy of the `switch` is a
jump to the case the constant picks; as LLVM's `dfa-jump-threading`.

## How it works
A translation of LLVM `d1106deb71cc` `DFAJumpThreading.cpp`, one Rust
function per C++ function, each linking its line; the plan and the stages
are `docs/architecture/dfajumpthreading-translation.md`. This is stages S1
and S2. [`mainswitch`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dfajumpthreading/mainswitch.rs) decides whether a `switch` is a candidate
(`MainSwitch`): its condition is a `phi` in a loop and what flows into it is
constants, `select`s that can be unfolded or values nothing predicts.
[`unfold`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dfajumpthreading/unfold.rs) turns those `select`s into control flow (a work list), so each
value of the state is set on a path of its own; the pin counts that as a
change even when nothing is threaded after it. [`paths`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dfajumpthreading/paths.rs) enumerates the
paths that set the state to a constant (`AllSwitchPaths`), bounded by the
pin's three caps. [`transform`](transform.md) decides whether copying is legal and
profitable and does it (`TransformDFA`): the blocks of each path from the
determinator on are cloned per exit value, the predecessors are redirected
to the clones, and SSA is restored.

```text
  p1: s = 1 --\                       p1: s = 1 --> sw.jt1: br c1
  p2: s = 2 ---+--> det: phi s        p2: s = 2 --> sw.jt2: br c2
                    sw: switch s      det, sw stay for the paths nobody decided
```

DEVIATE (DF2..DF4; DF1, a `select` in the chain rejecting the switch, was
closed by S2; DF5 is a parity note):
- DF2. No `DomTreeUpdater`: the pass returns `none()` when it threaded
  anything and the manager rebuilds the dominator tree and the loop info
  (the pin keeps the dominator tree, and the loop info unless it cloned);
  `updateSSA` reads a dominator tree built after the threading. No noalias
  scopes to clone (J8 of `jumpthreading.rs`), no optimization remarks,
  `!prof` is not carried.
- DF3. A path through a block that starts with a `landingpad` is not
  threaded: the copy would be a second pad, and the one module handler
  (`ehprepare`) cannot choose between two (J10 of `jumpthreading.rs`).
- DF4. The loop info is rebuilt after the `select`s are unfolded (the pin
  adds the new blocks to the loop of the start block): `LoopInfo` has no
  `addBasicBlockToLoop` (T3, S3). Once per switch with `select`s, linear in
  the function. No `!prof` or debug location is carried to the new branches.
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
