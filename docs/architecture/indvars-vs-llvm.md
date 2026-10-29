---
architecture-index: indvars against IndVarSimplify
architecture-order: 150
---
# indvars against LLVM's IndVarSimplify

TL;DR: `indvars` is 64.4 M of `d_faces` -O2 (46 runs, 34 of them in the pipeline step), LLVM's `IndVarSimplify` 23.6 M (41 loop invocations). Two rows are 55% of it and both are an analysis derived again after an edit, where LLVM's `ScalarEvolution` is kept and told what changed: the facts of the body are derived afresh after each of 12 widenings (21.8 M), and each exit's trip count is found again at every run (13.9 M).

Sources: llrm CI-mode build of main 7ac662405, `llrm-cc -m32 -O2` of `d_faces.c`, callgrind inclusive, `LLRM_DEBUG=time` for runs; LLVM 24 (d1106deb71cc) `opt -passes='default<O2>'` on the same file, callgrind inclusive of `IndVarSimplifyPass::run`, which includes the `ScalarEvolution` it builds.

## 1. What LLVM does

Per loop (a loop pass, 41 invocations here: 27 in `d_draw_faces`, 11 in `sc_mipfloor_c`): `simplifyAndExtend` (`SimplifyIndvar` and `WidenIV`: a counter read through `sext`/`zext` becomes a counter of that width), `rewriteLoopExitValues` (a value used after the loop becomes a closed form from `SCEV`), `optimizeLoopExits` (an exit whose count another decides is folded), `linearFunctionTestReplace` (the exit test moves to the counter that stays), `sinkUnusedInvariants`. Every step asks `ScalarEvolution`, which keeps what it found per value and per loop (`BackedgeTakenCounts`) and drops only what an edit's `forgetValue` names.

## 2. What llrm does, and what is named

`IndVars::run` (46 runs; `widened_with`, `exitfold::folded_with`, `exitsink::sunk`, `loopexit::evaluated`, `exit_values`, `congruent`, `rewound`) over the whole function, not a loop.

| row | M | LLVM | here | probe / kill |
|---|--:|---|---|---|
| `Standing::of` in `widened_with`: `consts::known` of the whole body derived afresh after each widening | 21.8 (12 widenings, 12 derivations of a 1,350-instruction body, 1.8 M each) | `WidenIV` edits the IV's uses; `SE.forgetValue` on what changed; the rest stands | `standing.changed()` drops all facts; the restart derives them for the next loop | done (#2100): the facts that stand are kept for every value the widening did not reach, the rest settled again with the manager's rule (`known_from` with a dirty set); `Standing::of` 21.8 to 5.0 M, `indvars` 65.5 to 48.7 M, d_faces -O2 -0.80% |
| `exitfold::folded_with` | 20.8 (`induction::exits` 13.9 over 111 calls, 125 K each; `guards` 4.9) | `getExitCount` cached per loop | the exit counts found again at every run, whether or not the body changed | probe, measured: 111 calls (92 `exitfold`, 19 `indvars`) over 21 distinct (body version, loop): repeat ratio 5.3, so about 90 of the 111 are asked of a body and loop already answered (11 M, 0.45%); passes. Next: the exit counts kept by (mark, loop) in the manager |
| `Bounded` first ask | 5.6 | `SE` | the manager's analyses, built at the first ask of the step | the same analyses are asked by the next steps: not a row of this pass |
| `exitsink::sunk` | 4.3 (46 calls, 93 K each) | `sinkUnusedInvariants` | scans every exit of the function each run | probe: runs that sink nothing; kill if under 80% sink nothing: then a structural precheck |
| `loopexit::evaluated`, `exit_values`, `congruent`, `rewound` | under 1 M together | `rewriteLoopExitValues`, `replaceCongruentIVs` | | none |

A structural precheck before the facts in `widened_with` (some header phi has a `sext`/`zext` user to the index width, which `_find` needs of any counter it widens) was tried as a throwaway: 2,480.8 to 2,477.8 M on `d_faces` -O2 (-0.12%), same output. The 12 derivations are not the loops without a candidate: they follow widenings.

## 3. Runs and cost per instruction

| | runs | input | M | per input instruction |
|---|--:|--:|--:|--:|
| llrm `indvars` (step, 34 of 46 runs) | 34 | 6,586 function instructions | 64.4 | 9.8 K |
| LLVM `IndVarSimplify` | 41 loop invocations | 3,526 loop instructions after the pass | 23.6 | 6.7 K |

Not like for like: LLVM's loop dump is the loop alone and the pass sees a loop; ours sees the function. Per loop invocation LLVM is 575 K; ours has 46 runs of the whole function, 1.4 M a run.

## 4. Order

1. Row 1: done, #2100.
2. Row 2 by keeping the exit counts per (loop, body version) in the manager's `Counted`: the analysis exists; `exitfold` and `indvars` ask `induction::exits` directly.
3. Then the LLVM-shaped proof of concept on a dumped input (edit-contract-replay's set): row 1 and 2 as LLVM has them, edits missed classified.
