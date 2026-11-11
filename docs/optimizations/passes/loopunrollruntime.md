> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopunrollruntime.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopunrollruntime.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopunrollruntime.rs sha256:db870f8685f832ab5b77d3d3d99e1a88d96083734ee38d0dbf738fae45ea6301 -->

# loopunrollruntime

The remainder of `UnrollRuntimeLoopRemainder`
(`LoopUnrollRuntime.cpp` at `d1106deb71cc`): before a loop of a run-time
trip count is unrolled by `count`, a copy of the loop runs the `trip count %
count` iterations the unrolled loop leaves, after it (epilog) or before it
(prolog). The caller picks the form as `isEpilogProfitable` does: the epilog
where a header phi starts at an integer constant, else the prolog.

## How it works
The preheader computes the trip count `TC = BECount + 1` (SCEV expanded),
`xtraiter = TC % count`, and branches to the epilog when `BECount <u count -
1` (fewer iterations than one unrolled body runs), else into the loop, which
now counts `niter` up to `TC - xtraiter`. The loop's exit leaves for a
guard that skips the epilog when `xtraiter` is zero. The epilog is a copy of
the loop's blocks (`.epil`) that counts `epil.iter` up to `xtraiter`, and
its header and exit phis take the values the unrolled loop left.

The caller then unrolls the loop's body by `count` itself (the jam does).
With `count` 2 the epilog runs once: its backedge is broken and its latch
merged into its exit.

## Deviations from LLVM
- D2: only a loop with one exiting block and one exit block. The pin's
  multi-exit mode needs `UnrollRuntimeMultiExit` or a target that says it
  pays; the jam refuses such loops before it gets here.
- D3: no branch weights (`!prof` of the guards): the pass manager keeps no
  weights for the loops this runs on, and the jam's tests have none.
- D4: no `llvm.loop.unroll.disable` on the epilog (`setLoopAlreadyUnrolled`)
  and no loop estimated trip count: a function pass cannot add metadata
  nodes; llrm has no partial unroller to run on the epilog.
- D5: `LoopInfo`, the dominator tree and SCEV are made again by the caller
  after the call, as in [`crate::loopinterchange`](loopinterchange.md) D1.

## Example
A loop `for (i = 0; i < n; i++) body` unrolled by 4 becomes, in the
preheader, `xtraiter = (n) & 3`; the unrolled loop runs `n - xtraiter`
iterations; the epilog `body.epil` runs `xtraiter` times. As a prolog,
`body.prol` runs `xtraiter` times first, the unrolled loop starts from what
it left (`.unr` phis in `body.prol.loopexit`) and is skipped when the
prolog ran every iteration.

## See also
`UnrollRuntimeLoopRemainder` (L672), `ConnectProlog` (L82), `ConnectEpilog`
(L255), `CloneLoopBlocks` (L404), `CreateTripRemainder` (L607).

## Used by
Named by `loopunrollandjam`, `loopunrollpass`.
