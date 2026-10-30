# Optimizer passes on dist, ours against the pin (w-5, 2026-10-25)

Whole-compile inclusive Ir per pass, `llrm-c -m32 -O2` against the pinned `clang -O2 -fno-inline-functions` on `qbq-llrm/src/render/d_faces.c`, 11 functions, **dist builds** (fat LTO, line tables: `CARGO_PROFILE_DIST_DEBUG=line-tables-only cargo build --profile dist`), callgrind `--trace-children=yes`, whole-compile totals 895.8 M ours (front end child excluded) and 396.9 M pin. Runs are the call counts of the pass's `run` (ours per function; the pin's loop passes per loop). Not the same IR: each pipeline works on its own, so a per-run cost mixes the pass with what the pipeline before it left (and with the analyses it asks for: the pin caches the dominator tree, loop info and SCEV across passes, ours builds them again per pass: MemorySSA is 45.7 M under LICM, GVN, MemCpyOpt and DSE). The pin's loop passes run under `FunctionToLoopPassAdaptor` (37.9 M in all), which builds the analyses they share outside the passes' own `run`: 3.2 M (MemorySSA 2.5, dominator tree 0.5, loop info 0.2, SCEV 0.15), so the loop rows leave out that much of the pin and include the analyses of ours. Same-IR comparisons exist for single passes (IndVars: 18.5 M against 13.8 M, 1.33x, #4435).

| pass | ours M | runs | per run K | pin M | runs | per run K | ratio |
|---|--:|--:|--:|--:|--:|--:|--:|
| InstCombine | 58.30 | 88 | 662.5 | 39.49 | 88 | 448.7 | 1.48 |
| GVN | 40.29 | 11 | 3662.8 | 32.04 | 11 | 2912.5 | 1.26 |
| IndVars | 27.28 | 12 | 2273.2 | 11.56 | 19 | 608.2 | 2.36 |
| LICM | 41.17 | 44 | 935.6 | 7.28 | 70 | 104.0 | 5.65 |
| CVP | 26.59 | 22 | 1208.8 | 6.66 | 22 | 302.6 | 3.99 |
| SimplifyCFG | 22.81 | 88 | 259.2 | 8.38 | 88 | 95.2 | 2.72 |
| EarlyCSE | 22.56 | 22 | 1025.5 | 5.70 | 22 | 259.2 | 3.96 |
| LSR | 18.45 | 7 | 2635.5 | 10.44 | 16 | 652.6 | 1.77 |
| JumpThreading | 15.92 | 22 | 723.8 | 4.50 | 22 | 204.4 | 3.54 |
| DSE | 13.34 | 11 | 1212.6 | 2.31 | 11 | 210.0 | 5.77 |
| MemCpyOpt | 9.63 | 11 | 875.2 | 1.28 | 11 | 116.2 | 7.53 |
| SROA | 9.20 | 33 | 278.7 | 9.76 | 44 | 221.8 | 0.94 |
| SCCP | 6.41 | 11 | 582.5 | 2.42 | 11 | 219.6 | 2.65 |
| LoopDeletion | 5.95 | 11 | 540.6 | 3.04 | 35 | 87.0 | 1.95 |
| LoopRotate | 2.19 | 11 | 199.1 | 5.87 | 35 | 167.8 | 0.37 |
| LCSSA | 5.62 | 51 | 110.1 | 0.40 | 19 | 21.1 | 14.00 |
| LoopSimplify | 4.09 | 51 | 80.2 | 0.61 | 19 | 31.9 | 6.76 |
| LoopFullUnroll | 9.90 | 11 | 900.0 | 1.98 | 17 | 116.5 | 5.00 |
| LoopIdiom | 4.65 | 11 | 423.1 | 3.07 | 19 | 161.4 | 1.52 |
| LoopLoadElim | 7.01 | 11 | 636.9 | 2.10 | 11 | 191.1 | 3.33 |
| Reassociate | 4.09 | 11 | 372.2 | 0.82 | 11 | 74.7 | 4.98 |
| InstSimplify | 2.41 | 11 | 218.7 | 0.89 | 11 | 81.2 | 2.69 |
| ADCE | 1.50 | 11 | 136.8 | 0.63 | 11 | 57.0 | 2.40 |
| BDCE | 1.25 | 11 | 113.6 | 1.12 | 11 | 101.9 | 1.12 |
| sum | 360.6 | | | 162.3 | | | 2.22 |

## Same IR, pass by pass (the table to read)

Every pass instance the `--dump` of d_faces -O2 recorded (80 of them: a dump file is written where a pass changed the function), run on the file before it by both: ours `llrm-pass -O2 --order PASS@SLOT FILE` on the dist build, the pin `opt -passes=<pass> -S FILE`, callgrind `--toggle-collect` on the pass's `run` (`GVNPass::*`, `JumpThreadingPass::*` and `ReassociatePass::*`, whose `run` is inlined into the adaptor). Same input, so the ratio is the pass.

| pass | instances | ours M | pin M | ratio | excess M |
|---|--:|--:|--:|--:|--:|
| licm | 7 | 19.61 | 4.41 | 4.45 | 15.20 |
| cvp | 6 | 25.62 | 14.47 | 1.77 | 11.15 |
| jumpthreading | 6 | 15.04 | 6.39 | 2.35 | 8.65 |
| dse | 1 | 9.25 | 2.40 | 3.85 | 6.85 |
| indvars | 5 | 22.08 | 16.54 | 1.34 | 5.54 |
| gvn | 4 | 39.49 | 34.24 | 1.15 | 5.25 |
| reassociate | 4 | 3.90 | 0.55 | 7.14 | 3.35 |
| lcssa | 10 | 3.05 | 0.88 | 3.48 | 2.18 |
| loopdeletion | 1 | 5.39 | 3.64 | 1.48 | 1.74 |
| sccp | 4 | 5.54 | 4.03 | 1.37 | 1.51 |
| loopsimplify | 6 | 1.46 | 0.47 | 3.13 | 1.00 |
| unroll | 1 | 2.62 | 1.82 | 1.44 | 0.80 |
| adce | 2 | 1.34 | 0.79 | 1.69 | 0.55 |
| loopidiom | 2 | 4.27 | 3.87 | 1.11 | 0.41 |
| tailrec | 2 | 0.30 | 0.21 | 1.43 | 0.09 |
| bdce | 2 | 1.07 | 1.37 | 0.78 | -0.30 |
| combine | 17 | 39.24 | 67.51 | 0.58 | -28.26 |
| sum | 80 | 199.3 | 163.6 | 1.22 | 35.7 |

Read with care: a changed-only sample (passes that found nothing are not in it), `combine` is ours 0.58x the pin's `instcombine` because the slot is one stage of our combine, not the whole of the pin's pass (the whole-compile table has InstCombine at 1.48x), and per-instance inputs are the pipeline's, not each pass's worst case. The whole-compile table above stays for run counts.

Over the same IR the excess is in LICM (4.5x), JumpThreading (2.4x), Reassociate (7x), DSE (3.9x), LCSSA (3.5x), LoopSimplify (3.1x) and CVP (1.8x); IndVars (1.34x), SCCP (1.37x), LoopDeletion (1.48x), GVN (1.15x) and LoopIdiom (1.11x) are near the pin's. The whole-compile ratios of IndVars (2.4x), EarlyCSE, MemCpyOpt and LoopFullUnroll were run counts and IR shape.

## Where the excess is (whole-compile table, different IR) (ours less pin, M)

LICM 33.9, CVP 19.9, InstCombine 18.8, EarlyCSE 16.9, IndVars 15.7, SimplifyCFG 14.4, JumpThreading 11.4, DSE 11.0, MemCpyOpt 8.4, GVN 8.3, LSR 8.0, LoopFullUnroll 7.9, LCSSA 5.2, LoopLoadElim 4.9, SCCP 4.0, LoopSimplify 3.5, Reassociate 3.3, LoopDeletion 2.9. The ratios that stand out are LCSSA 14x, MemCpyOpt 7.5x, LoopSimplify 6.8x, DSE 5.8x, LICM 5.7x, LoopFullUnroll 5.0x, Reassociate 5.0x; the large absolute ones with a small ratio are InstCombine (1.48x) and GVN (1.26x). SROA (0.94x) and LoopRotate (0.37x, fewer runs) are at or under the pin's.

## Owners (proposed, w-3 first)

| passes | what they share | owner |
|---|---|---|
| LICM, LSR, GVN, MemCpyOpt, DSE, EarlyCSE | MemorySSA, memdep, alias queries | w-3 |
| IndVars, LoopDeletion, LoopFullUnroll, LoopIdiom, LoopLoadElim, LCSSA, LoopSimplify | SCEV, loop analyses rebuilt per pass | w-5 |
| CVP, JumpThreading, SCCP, InstCombine, Reassociate, InstSimplify | LazyValueInfo, known bits | w-4, funcattrs-port |
| SimplifyCFG | | unowned |
