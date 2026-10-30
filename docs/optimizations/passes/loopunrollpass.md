> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopunrollpass.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopunrollpass.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopunrollpass.rs sha256:f67654875dfc727d3c653b04a210c63068c3c9cfff5efa804130a2c77b39be47 -->

# loopunrollpass

`LoopFullUnrollPass`: a loop whose trip count is known and small enough, or
whose unrolled form simplifies enough, is copied out completely.

## How it works
For each loop, innermost first, `try_to_unroll_loop` gathers the unrolling
preferences (the thresholds, as the target and the optimization level set
them), prices the loop with `CodeMetrics` (`UnrollCostEstimator`), reads the
trip count from ScalarEvolution (the smallest exact trip count of any
exiting block, else the maximum), and asks `compute_unroll_count` for a
count. The count is the trip count when
- the unrolled size is under the threshold (`should_full_unroll`), or
- it is not, but `analyze_loop_unroll_cost` simulates every iteration with
  [`UnrolledInstAnalyzer`](../analyses/loopunrollanalyzer.md) and finds the
  unrolled cost under the threshold boosted by the dynamic cost saved.

A count that is not the whole trip count would be partial or runtime
unrolling, which this pass (`OnlyFullUnroll`) does not do. The loop is then
unrolled by [`crate::loopunroll::unroll_loop_blocks`](loopunroll.md).

## Deviations
- U3 `computePeelCount` is `looppeel.rs` (S2); its deviations are PL1 to PL5
  there.
- U4 The pragma metadata (`llvm.loop.unroll.*`) is read from the loop ID on
  the latch's branch; no frontend of llrm writes it yet. Remarks (the
  `OptimizationRemarkEmitter`), `UniformityInfo` and profile data are left
  out: D3 of the plan page, and MIR has no convergence tokens or PSI.
- U5 `Function::hasOneLiveUse` of an internal callee is found by scanning
  the module for the callee's uses; no use list is kept for globals.
- U6 The loop pass manager's update of loops (`addSiblingLoops`) is the
  driver's: the loops a complete unroll makes of the inner loops are visited
  next, in program order.
- U8 `simplifyLoopAfterUnroll` simplifies without a dominator tree or
  assumptions.
- U9 Blocks an unroll merges away are erased once, when the pass ends.
- U1 At `optsize` a loop of at most the target's few trips is unrolled under
  the target's bar (`Tti::unroll_size_few_trips`; m16: 3 trips, 30) where
  the pin's `OptSizeThreshold` is 0 on x86. float_in_place_update.c at -Os
  m16: 132 bytes to 14 (main 76); the corpus of 258 programs at m16 -Os
  changes in no other. logical_value.c (main 25, here 155), split_loops.c
  and int64_division.c need the cost analysis held to a bar too, which grew
  matmul.nib 390 to 562 bytes: open.

## Example
A loop of four trips becomes four copies; the induction variable's
arithmetic on constants is simplified away.
```mir
b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 4
  br i1 %go, label %b1, label %b2
```
becomes, after simplification, copies of the body on `0`, `1`, `2` and `3`
with no `phi`, no `icmp` and no backedge.

## See also
[`tryToUnrollLoop`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L1213),
[`computeUnrollCount`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L1016),
[`analyzeLoopUnrollCost`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L336),
[`LoopFullUnrollPass::run`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LoopUnrollPass.cpp#L1640).

## Used by
Named by `loopunrollandjam`, `pipeline`.
