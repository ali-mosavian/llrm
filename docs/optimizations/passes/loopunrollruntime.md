> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopunrollruntime.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopunrollruntime.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopunrollruntime.rs sha256:dc75d61864ca3900a3358d314b0ad4bf7029b23f0ec7f1de35c08192b3e818b9 -->

# loopunrollruntime

The epilog remainder of `UnrollRuntimeLoopRemainder`
(`LoopUnrollRuntime.cpp` at `d1106deb71cc`): before a loop of a run-time
trip count is unrolled by `count`, a copy of the loop runs the `trip count %
count` iterations the unrolled loop leaves.

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
- D1: only the epilog form: the prolog is for `-unroll-allow-remainder`
  loops the unroller's runtime mode makes, which llrm has no pass for.
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
iterations; the epilog `body.epil` runs `xtraiter` times.

## See also
`UnrollRuntimeLoopRemainder` (L672), `ConnectEpilog` (L255),
`CloneLoopBlocks` (L404), `CreateTripRemainder` (L607).

## Used by
Named by `loopunrollandjam`.
