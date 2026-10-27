> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/looppeel.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/looppeel.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/looppeel.rs sha256:1b4dfe736b9d35fafed9f4f85aa16a07c22bf6ac2e04c4500ff5ed9ded3ca237 -->

# looppeel

How many iterations of a loop to peel off its front, from `LoopPeel.cpp`'s
`computePeelCount`: the iterations after which its header phis, or the
compares in its body, are decided.

## How it works
`compute_peel_count` is asked by the loop unroller after the unroll count
it would choose has failed. A loop must be in simplified form, an
innermost loop (unless nests are allowed), and at most
`Threshold / LoopSize - 1` iterations (and 7, `unroll-peel-max-count`) are
peeled, counting those already peeled. The count is the largest of
- the iterations after which every header phi is the same on every trip
  (`PhiAnalyzer`: a phi takes its back-edge value's count plus one, an
  operation the largest of its operands', a loop-invariant value none), and
- the iterations after which a compare in the body is decided
  (`countToEliminateCompares`): an `icmp` of an affine recurrence of the
  loop against an invariant, under a branch, a `select`, or the `and`/`or`
  of such compares, is stepped through the iterations while it holds, and
  the count is the iterations until it flips; `min`/`max` on a recurrence
  likewise.

```text
  for (i = 1; i <= n; i++) { ...; if (i > 1) f(i); }     peel 1:  body without f; for (i = 2; ...) { ...; f(i) }
```

The peeling is [`crate::loopclone::peeled_any_latch`](loopclone.md): the iterations are
copied ahead of the loop, each keeping its exits.

## Deviations
- PL1 `llvm.loop.peeled.count` is not written (a pass cannot add a metadata
  node): the caller keeps the count for the header in the run, so a loop
  revisited in the same run is bounded as the pin's is. Exit: metadata nodes
  made by function passes.
- PL2 `isDereferenceablePointer` of
  `peelToTurnInvariantLoadsDereferenceable` is
  `valuetracking::dereferenceable`: an argument attribute, an alloca or a
  global of known size; not the pin's walk with the dominator tree and the
  assumptions, so a load the pin proves dereferenceable by those is peeled
  for here. Exit: that walk.
- PL3 Peeling the last iteration (`PeelLast`, `canPeelLastIteration`,
  `shouldPeelLastIteration`) is not translated: no loop is peeled from the
  end, and a count that would have been is dropped.
- PL4 Profile-based peeling is not translated (no profile data).

## Example
The loop of `bintree` (`for (i = 1; i <= 300; ++i) { t[3*i] = ...; if (i >
1) insert(t, 1, i); }`) tests `i > 1` on each trip; `i` is 1 on the first
and more after, so one iteration is peeled and the test is gone from both.

## Runs
Inside [`crate::loopunrollpass`](loopunrollpass.md), in LLVM's order after full and bounded
unrolling and before partial unrolling.

## See also
[`computePeelCount`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopPeel.cpp#L753),
[`countToEliminateCompares`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopPeel.cpp#L549),
[`PhiAnalyzer`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Utils/LoopPeel.cpp#L195).
