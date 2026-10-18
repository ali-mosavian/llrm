# Analysis cost, d_faces -O2

Self cost of `llrm_analysis::*` frames, order tree, callgrind `--separate-callers=30`
(total 2,044.9 M; plain perf 1,994 M). Overlaps the slot rows: it re-slices them.
Every module is llrm-only today; none is a translation.

| module | M | owner | replaced by | largest consumer (M) |
|---|--:|---|---|---|
| memory | 129 | basicaa-port | BasicAA S1 (MemoryLocation, GEP decompose) | interprocedural::stamped 30, ModuleAnalyses::computed 28 |
| alias | 94 | basicaa-port | BasicAA + CaptureTracking + GlobalsAA S1-S6 | stamped 32, computed 31 |
| ranges + ranger | 84 | vt-constantfolding | LVI + CVP (ConstantRange merged, unused) | decide 13 |
| memoryssa + regions | 82 | mssa-green-2127 | MemorySSA port, `mssa/` | decide 23, fold 17, floatfold 18 |
| induction | 42 | scev-port | `scev/*`, S7 deletes | decide 19 |
| num | 38 | none (bignum helper) | goes with its consumers | decide 21 |
| constant_cycles | 33 | scalar-cleanup-port | SCCPSolver | decide 15 |
| cfg | 33 | domtree-port | none: wrapper over translated DomTree/LoopInfo | computed 4 |
| consts | 28 | fold-poc | InstSimplify/KnownBits/SCCP | decide 8, fold 6 |
| difference | 24 | domtree-port | ConstraintElimination (O(n^3) closure today) | decide 24 |
| frameescape | 19 | basicaa-port | CaptureTracking, S6 deletes | computed 4 |
| guards | 12 | domtree-port | ConstraintElimination DFS | decide 10 |
| liveness | 11 | greedy-inline-spiller | pending | spill 3 |
| branchprob | 11 | cgscc-poc | BPI + BFI | profit::_odds_in 6 |

`decide` (owner simplifycfg-fold-2148) consumes ~133 M of these; its sources are
replaced by ConstantFoldTerminator, SCCP, CVP/LVI and ConstraintElimination.

## Clang denominator

Clang -O2 on d_faces (411.6 M): analyses, inclusive, from the callgrind graph:
AAResults alias 24.5 + getModRefInfo 15.4, SCEV 12.8, MemorySSA 2.5, TTI 1.3,
DomTree/LoopInfo 1.0 = ~57 M (14%). llrm: 599 M (29%).
Per IR instruction entering the pipeline (llrm 10,484 / clang 6,839): llrm 57 M/k, clang 8.3 M/k, 6.9x.
Clang counts analysis queries inclusive; llrm self cost only, so the gap is a floor.

Reproduce: `projection/ctx.py`, `consumers.py`, `clanga.py` (scratch).
