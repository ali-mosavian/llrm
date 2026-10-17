# lsr against LLVM's LoopStrengthReduce

TL;DR: `lsr` is 148 M of `d_faces` -O2 (2,689 M), LLVM's `LoopStrengthReduce` 10.4 M. The structure differs by design (gcc's ivopts search with a per-loop spill forecast against LLVM's register-count cost), and the forecast is worth its price: switching it off changes 425 of 1,583 objects and grows the bench code. What is left to take is mechanism-identical and small; none of it is a slope.

## 1. Where the 148 M goes

`d_faces` -O2, 19 `reduced` asks, 11 functions; CI-mode builds (`CARGO_INCREMENTAL=0`, fresh target per SHA), callgrind inclusive, #2008 and #2082 applied.

| part | M | what |
|---|--:|---|
| `Problem::solved` | 64.1 | 1,082 `total` asks, 411 distinct candidate sets priced, 329 `Run::fit` sweeps of about 190 K each (61.9 M) |
| building the problem | 59.5 | `Products` 16.2, `TrafficBase::of` 7.6, `_priced` (1,000 calls) 6.7, `_run` 6.4, `Counted` 8.3, `induction::users` 3.6, the rest in pieces under 3 M |
| `Registers` | 5.0 | 30 asks of the interval facts (the analysis' own run, 27.1 M over the whole compile, is mostly asked elsewhere) |
| `Pressure` (after #2082) | 15.5 | 19 asks |

## 2. LLVM's structure, per row

| row | LLVM | here |
|---|---|---|
| candidate generation | per use, formulae by `GenerateAllReuseFormulae`, pruned by `NarrowSearchSpaceUsingHeuristics` to a complexity limit | gcc's candidate set per loop (`_candidates`), `iv-max-considered-uses` as the cap |
| search | `SolveRecurse` on a `Cost` of registers, adds, setup and immediates | `improved`: add, remove, swap one candidate while it lowers the total (gcc `iv_ca`) |
| price of a set | `Cost` counts registers used (`RegUseTracker`) against `TTI.getNumberOfRegisters`, plus a penalty past it | `total_of`: work per trip plus `Run::fit`, the spill the set causes at every instruction of the loop |
| facts | `ScalarEvolution`, `LoopInfo`, `DominatorTree`, all preserved | `Registers`, `Pressure`, `Products`, `TrafficBase`: asked per loop, updated per edit |

Mirror by default would replace the price of a set by the register count. Probe 1 says what that costs.

## 3. Probes

- Probe 1, the forecast off (`LLRM_LSR_NOSPILL`, `fit` returns 0, throwaway): 425 of 1,583 corpus objects differ (identical.py, every `LLRM_CHECK_` on). bench -O2 code, instruction lines: matmul 89 to 105, queens 83 to 90, mandel 56 to 60, stft_single 260 to 271, jpeg_fixed 1,129 to 1,121; total 5,627 to 5,666 (+0.7%). Killed: the forecast decides real choices, in the direction of better code.
- Probe 2, a register-count proxy in the forecast's place: not built. A proxy that ranks like the sweep needs the peak point of each block run, which is what the sweep finds; the saving it can reach is the 62 M of sweeps, 2.3% of the compile, for a change of choice in an unknown share of the 425.
- The sweep is already incremental over the sets of one problem (#2008: the delta protocol). Each distinct set still visits every step of the loop.

## 4. What is left, and the order

1. The fixed cells of a loop (what it keeps in registers besides its candidates) are the same in every set; the sweep re-adds them per step per set. Ranking them once per loop and layering only the candidate-dependent residents per set is mechanism-identical. Hypothesis, not measured: the residents that vary per set are a small share of what a sweep handles. Pass: objects identical, `fit` under 35 M on `d_faces`. Kill: under 20 M saved.
2. Problem construction (59.5 M) is many pieces under 3 M each; no row above 1% except `Products` (16.2 M, 0.6%): bundled with the next lsr change, not alone.

Not done: dropping or replacing the forecast. A change of choice goes through the rows rule with the bench code size, not through this page.
