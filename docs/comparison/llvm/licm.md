---
architecture-index: LLVM's LICM against llrm's `hoist`
architecture-order: 360
---
# LLVM's LICM against llrm's `hoist`

TL;DR: LLVM hoists an instruction when its operands are invariant and it is safe, with no price; llrm prices every run it would move with the work saved and a whole-function spill forecast: `hoist` is 190.4 M (5.9%) of the d_faces `-O2` compile against LICM's 7.3 M (1.8%), and about 60 M of it is the forecast.

## LLVM

`hoistRegion` walks the loop's blocks and hoists each instruction whose operands are loop invariant, that `canSinkOrHoistInst` allows and that is safe to run unconditionally. Memory queries are capped by `licm-mssa-optimization-cap` (100) and promotion by `licm-mssa-max-acc-promotion` (250) ([`LICM.cpp#L149-L160`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L149-L160)) [`llvm/lib/Transforms/Scalar/LICM.cpp#L662-L680`](https://github.com/llvm/llvm-project/blob/d1106deb71cc81016406a1917d0508d2df296266/llvm/lib/Transforms/Scalar/LICM.cpp#L662-L680):

```cpp
  for (BasicBlock *BB : Worklist) {
    // Only need to process the contents of this block if it is not part of a
    // subloop (which would already have been processed).
    if (!LoopNestMode && inSubLoop(BB, CurLoop, LI))
      continue;

    for (Instruction &I : llvm::make_early_inc_range(*BB)) {
      // Try hoisting the instruction out to the preheader.  We can only do
      // this if all of the operands of the instruction are loop invariant and
      // if it is safe to hoist the instruction.
      if (CurLoop->hasLoopInvariantOperands(&I) &&
          canSinkOrHoistInst(I, AA, DT, CurLoop, MSSAU, true, Flags, ORE) &&
          isSafeToExecuteUnconditionally(I, DT, TLI, CurLoop, SafetyInfo, ORE,
                                         Preheader->getTerminator(), AC,
                                         AllowSpeculation)) {
        hoist(I, DT, CurLoop, Preheader, SafetyInfo, MSSAU, SE, ORE);
        Changed = true;
        continue;
      }
```

## llrm

`_affordable` finds a loop's invariant run, then prices the function with the run moved: `work` at the block frequencies, plus `spill_forecast_after` (the forecast of the whole function from the base `Pressure` and the change log), plus a release for each float the run holds across the loop. The run moves only if the price after, less what is free, is not above the price before (`moved - uncounted <= kept`, [`hoist.rs#L471`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/opt/llrm-transforms/src/hoist.rs#L471-L471)) [`crates/opt/llrm-transforms/src/hoist.rs#L379-L397`](https://github.com/ali-mosavian/llrm/blob/21f5c9fc8e67b5bf405cf9ecdaf65247a26c0450/crates/opt/llrm-transforms/src/hoist.rs#L379-L397):

```rust
        let across = |inst| crate::spill::kept_across(outer, unit.context, hoisted, inst);
        let forecast = match (&moved, &base) {
            (Some((_, changes)), Some(base)) => profit::spill_forecast_after(
                unit.context,
                unit.layout,
                hoisted,
                base,
                changes,
                costs,
                room,
                &across,
                frequency,
            ),
            _ => profit::spill_forecast(unit.context, unit.layout, hoisted, costs, room, &across, frequency),
        };
        let Some(forecast) = forecast else { return run };
        let moved = work
            + forecast.cost
            + floats.len() as i64 * costs.float_release * frequency.get(&into).copied().unwrap_or(1);
```

## Divergence

LLVM decides locally and leaves register pressure to the machine passes (`MachineLICM`, 8.5 M, and the allocator's splitting); llrm decides with the allocator's cost model in the optimizer, so a hoist that would spill is refused before it is made. The model is the cost: the forecast sweeps every block, so a loop that moves nothing still pays for the function. At `-Os` the frequency is flat (one per block, bytes counted), so 34 of 36 priced candidates differ only by the float-release term.

## Measured

| | LLVM (clang 24, cc1) | llrm |
|---|---:|---:|
| pass, inclusive | `LICMPass::run` 7.29 M (1.76%), 70 loop runs | `Hoist::run` 190.4 M (5.91%), 35 runs; `LoopMotion` 28.3 M (0.88%) |
| pricing | none | `spill_forecast_after` 44.8 M (3 calls) + `View::forecast` 10.0 M + `Pressure` 5.5 M = 60.3 M (1.87%) |
| memory structure | shared `MemorySSA` (27 builds in all, 2.5 M) | `Accesses::managed` 58.4 M (9 builds under `hoist`) |
| invariant runs found | in the walk | `_invariant_run` 37.5 M (43 calls), `_frequency` 16.1 M |

Input `render/d_faces.c`, `-m32 -march=i486 -fno-inline-functions -O2`. LLVM: callgrind on `clang -cc1` of d1106deb71cc, total 415.4 M. llrm: callgrind on the release `llrm-cc` of 21f5c9fc8, total 3,224 M. The `-Os` figure is from `LLRM_DEBUG=hoist` on commit 954bc48fa.

## Proposed change

- **Files**: `crates/opt/llrm-transforms/src/hoist.rs` (`_affordable`), `crates/opt/llrm-transforms/src/spill.rs` (`View::forecast`, `Pressure::updated`), `crates/opt/llrm-transforms/src/gvn.rs` (`crossing_price`).
- **What**: price a run by its cheap exact terms first and run the whole-function forecast only when the verdict is open; or forecast the affected region only.
- **Expected saving**: the first is small and not exact: at `-Os` the frequency is flat, so 34 of 36 priced candidates differ only by the float-release term, and the forecast cost is 0 before and after; a rejection still needs the forecast before. The second, an incremental forecast sweep, re-sweeps 2 to 144 of about 216 blocks per candidate at `-Os` and 204 to 213 of 228 at `-O2`: a cell's price is its traffic over the whole function, so a hoist reprices cells live across hundreds of blocks. Ceiling about 1.5% at `-Os`, about 0 at `-O2`.
- **Status**: measured and not kept ([compile-time-spec section 8](../../architecture/compile-time-spec.md), "Hoist's per-block sweep memo"); the forecast-first pricing was sized and not built.
