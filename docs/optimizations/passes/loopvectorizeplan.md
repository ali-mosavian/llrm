> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/loopvectorizeplan.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/loopvectorizeplan.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/loopvectorizeplan.rs sha256:04ee481bad523e1ae55cafc89cebdf5639a0c011683d1f75f8d92e540a4ec9c2 -->

# loopvectorizeplan

The loop vectorizer's planner: which vectorization factor (VF) and
interleave count (IC) a legal loop gets, and the plan the transform runs.

## How it works
A translation of LLVM `d1106deb71cc` `LoopVectorizationPlanner.cpp` and
`LoopVectorize.cpp`, in the pin's order. The plan is the loop's widenable
instructions (`Recipe`). The widest VF comes from the registers
(`getMaximizedVFForTarget`, L235: the register width over the widest type,
or over the smallest where the target maximizes bandwidth, clamped to the
safe dependence distance and the trip count). Each power of two up to it is
priced with the target's throughput costs, per lane; the cheapest wins and
a tie goes to the wider (`isMoreProfitable`, L709). The interleave count
(`selectInterleaveCount`, LoopVectorize.cpp#L3705) divides the registers by
the loop's register pressure, caps at the target's maximum and the trip
count, then interleaves small loops until the loop overhead is about 5 %.

## Example
A copy of `i16`s over 1920 trips on a 128-bit target: widest VF `128 / 16`
= 8; VF 8 costs `(load 1 + store 1 + induction + branch) / 8` per lane
against 6 for the scalar loop; IC = 2 (`getMaxInterleaveFactor` 2, the
trip count allows 120 trips of 16).

## DEVIATE rows
- VP1 the plan is one flat list of recipes of the loop's single block: no
  VPlan CFG or regions, so no VPlan transforms (`optimizeForVFAndUF`, tail
  folding, epilogue vectorization). Exit: the VPlan data structure.
- VP2 recipes are the memory accesses, integer binary operations, integer
  casts, and the loop-control instructions; every other instruction makes
  the loop unplannable (the pin widens or replicates it). Exit: one recipe
  kind per row of the card (reductions, floating point, select, compare).
- VP3 the remainder is a scalar loop only (`CM_EpilogueAllowed` with a
  scalar epilogue): the pin also vectorizes it (`vec.epilog.*`, the epilogue
  VF of `EpilogueVectorizationMinVF`), which the plan does not price. Exit:
  epilogue vectorization (VEC3).
- VP4 the scalar cost is the recipe prices at VF 1, not the legacy
  `CM->expectedCost`; the two agree for this recipe set (the loop-control
  instructions are priced alike).
- VP5 register pressure (`calculateRegisterUsageForPlan`) is the live vector
  values of the one block, not the interval walk over a VPlan;
  loop-invariant registers are the splatted operands.
- VP6 `-Os`/`-Oz` and a constant trip count under 16
  (`TinyTripCountVectorThreshold`): no epilogue is allowed there, so only a
  trip count VF x IC divides is planned (`CM_EpilogueNotAllowedOptSize`,
  `CM_EpilogueNotAllowedLowTripLoop`); IC stays 1 there (L3705). The pin's
  other low-trip branches (`MaxVFForTC`, trip = a power of two + 1) and the
  divisibility test against the largest VF before any smaller one are not
  translated: the divisibility is tested on the chosen VF x IC. Exit: VEC3
  (LV-X3, 2026-11-20).
- VP9 DEVIATE (owner arm64-vec, exit: the pin's -O2 cost model gains it, or
  2026-12-31): a target's driver row may turn on gcc's
  `vect-cost-model=very-cheap` at -O2 (`-fllrm-vect-very-cheap`): a loop of
  an unknown trip count is planned only if VF x IC divides it, which nothing
  says, so none is: no scalar epilogue, as for `optsize` (VP6), where the
  pin's -O2 leaves one. A known trip count keeps the pin's plan: gcc
  vectorizes those (particle -O2: 9,311 clocks vectorized, 73,733 with the
  101 trips left scalar).
- VP8 closed: the loop-control prices are the pin's. The scalar loop pays
  its `add` and exit `icmp` (the phis, the branch and the address
  computations are free at throughput), the vector loop its
  `branch-on-count` compare, its canonical `add` and, where an access is
  scalarized, one `SCALAR-STEPS` add (`LV: Found an estimated cost of N for
  VF 1` and `Cost of N for VF k` of the pin's `-debug-only=loop-vectorize`).
- VP7 no hints (LV-D7 of the legality translation), no scalable VFs (NEON
  only). No tail folding: `preferTailFoldingOverEpilogue` is false without
  SVE
  ([L7378](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Target/AArch64/AArch64TargetTransformInfo.cpp#L7378)),
  so the pin's NEON loops end in an epilogue, as here.

## Used by
Named by `loopvectorize`, `loopvectorizeearlyexit`, `loopvectorizeinterleave`.
