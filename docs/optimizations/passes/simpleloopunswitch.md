> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/simpleloopunswitch.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/simpleloopunswitch.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/simpleloopunswitch.rs sha256:e64b0d32dcb38d54c2ce739e383d5401c3d24ef6623b47e1606086aab4d81f67 -->

# simpleloopunswitch

`SimpleLoopUnswitch`: a branch or switch in a loop on a loop-invariant
condition, one side of which leaves the loop, is decided before the loop
instead of on every trip (trivial). With `non_trivial`, a condition whose
sides both stay in the loop is decided there too, at the price of a copy of
the loop for each way (`nontrivial.rs`, chosen by `candidates.rs`).

## How it works
`unswitch_all_trivial_conditions` walks from the loop header along the
blocks the loop would run first. A block with a side effect ends the walk.
A conditional branch whose condition (or, for an `and`/`or` of conditions,
some of its inputs) is loop-invariant, and one of whose successors is an
exit, is unswitched by `unswitch_trivial_branch`; so is a `switch` on an
invariant, by `unswitch_trivial_switch`.

Unswitching splits the preheader, so that a new preheader holds the
loop-invariant test: the branch itself moves there, with the exit as one
successor and the new preheader as the other, and in the loop it becomes a
jump. The exit block is split where other edges reach it, and the phis of
the exit take the value they had on entry. Uses of the condition in the loop
become the constant it has when the loop is entered. Nothing is copied.

```text
  pre -> H(br c, exit, B) -> B -> H        pre(br c, exit, pre') -> H(br B) -> B -> H
                 \-> exit                                    \-> exit
```

A loop whose latch is reached from the branch by a latch with no side
effect and whose bound ScalarEvolution knows has the branch's latch edge
sent to the latch's exit as well.

## Deviations
- SU1 `revisitCurrentLoop` is not translated (the pipeline is a flat list of
  passes): after a loop is unswitched it is looked at again here.
- SU2 No `DominatorTree`, `LoopInfo` or `MemorySSA` update (D8 of the plan
  page): the analyses are derived again only where the nest changed, which
  is when a hoisted loop is put back in loop-simplify and LCSSA form for the
  whole function (`hoistLoopToNewParent`, SU3).
- SU3 New blocks go at the end of the layout, not beside their neighbours.
- SU4 No profile estimate for a partial branch (`EstimateProfile`), no debug
  locations, no convergent-call test (MIR has none).
- SU5 `isGuaranteedNotToBeUndefOrPoison` of a frozen invariant is the
  poison-only form with no dominator tree: MIR has no `undef`.
- SU6 ScalarEvolution is made for the one query, not kept and forgotten.
- SU7, SU8, SU10, SU11 are the non-trivial form's: see `nontrivial.rs`.
- SU9 No partially invariant conditions (`hasPartialIVCondition` and
  `buildPartialInvariantUnswitchConditionalBranch`: a loop-header test of
  loads MemorySSA shows unchanged on a path), no injected invariant
  conditions, no guards (MIR has none). Costs 2 of the 21 corpus functions
  `opt` unswitches (4 of 20 rotated), which the oracle classifies. Exit: the
  MemorySSA migration lands (mssa-green-2127, 2026-10-29).

## Example
Before:
```mir
b0:
  %z = icmp eq i16 %n, 0
  br label %head
head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  br i1 %z, label %out, label %body
```
After, the test once, in the old preheader:
```mir
b0:
  %z = icmp eq i16 %n, 0
  br i1 %z, label %out, label %b0.head
```

## Runs
In the `trivialunswitch@732` slot of LLVM's order, LPM1 after the second
LICM. Non-trivially at -O3 and -Omax, where the pin builds
`SimpleLoopUnswitchPass(NonTrivial = Level == O3)`; otherwise trivially
only.

## See also
[`unswitchTrivialBranch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimpleLoopUnswitch.cpp#L580),
[`unswitchTrivialSwitch`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimpleLoopUnswitch.cpp#L867),
[`unswitchAllTrivialConditions`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimpleLoopUnswitch.cpp#L1172),
[`unswitchLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/SimpleLoopUnswitch.cpp#L3431).
