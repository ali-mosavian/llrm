> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/dfajumpthreading.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/dfajumpthreading.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/dfajumpthreading.rs sha256:d4ce396b20004fd02f1a5f8534516af84351a3b1b87230a8cb8c29dedf966618 -->

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

DEVIATE (DF2, DF3; DF1, a `select` in the chain rejecting the switch, was
closed by S2, and DF4, the loop info rebuilt after the unfolding, by S3; DF5
is a parity note):
- DF2. No `DomTreeUpdater`: the pass returns `none()` when it threaded
  anything and the manager rebuilds the dominator tree and the loop info
  (the pin keeps the dominator tree, and the loop info unless it cloned);
  `updateSSA` reads a dominator tree built after the threading. No noalias
  scopes to clone (J8 of `jumpthreading.rs`), no optimization remarks, and
  no `!prof` or debug location is carried to the new branches.
- DF3. A path through a block that starts with a `landingpad` is not
  threaded: the copy would be a second pad, and the one module handler
  (`ehprepare`) cannot choose between two (J10 of `jumpthreading.rs`).
- DF5 (parity at the IR, a row in the lowering). On the kernel written for
  32 bits (`i32` is `long`, #3535) `x_switch` m16 -O2/-O3 is 709 B with the
  pass and 629 B without (+80 B); -O1 and -Os are identical (size gate). The
  pass's decision is the pin's: `bench_x_switch` is 86 IR instructions with
  it and 50 without, and `opt -passes='default<O2>'` (m16 target,
  `-enable-dfa-jump-thread` true/false) gives 93 and 48. The loss is in
  lowering the threaded state machine: 198 asm instructions with the pass
  and 178 without, where the pin's `llc` has 108 and 131 (the pin gains, we
  lose). Owner w-4, exit 2026-11-09. The earlier +17 B was UB code.

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
