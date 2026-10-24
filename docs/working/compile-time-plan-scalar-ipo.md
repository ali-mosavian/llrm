# Scalar and module/IPO compile-time plan

Owner: funcattrs-port. Posted 2026-10-23.

Measured on the ledger command (`docs/working/ledger-command.md`: `d_faces.c`, qbq-llrm, `-m32 -march=i486 -O2`).
Ours: dist of tip `e45020064ba216c121fbfad457e89181f07da85c`, 1063.4 M Ir.
Pin: `clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)` via `tools/pin_clang.py`, 507.8 M Ir (ledger: 508.7).
Inclusive per pass, M Ir.

| pass | ours | pin | excess |
|---|--:|--:|--:|
| GVN | 40.6 | 49.2 | -8.6 (off the list) |
| EarlyCSE | 32.5 | 7.8 | 24.7 |
| SCCP + IPSCCP (pin: SCCPPass 11x 4.2 + IPSCCP 1x 4.1) | 11.7 | 8.3 | 3.4 |
| MemCpyOpt | 10.7 | 0.47 | 10.2 |
| SROA | 9.3 | 9.8 | -0.5 (parity) |
| functionattrs post-order | 8.2 | 1.4 | 6.8 |
| GlobalOpt | 4.8 | 0.47 | 4.3 |
| Reassociate | 4.1 | 1.1 | 3.0 |
| DSE | 3.9 | 2.8 | 1.1 |
| InstSimplify | 2.5 | 1.1 | 1.4 |
| MemorySSA (EarlyCse 18.7 + MemCpyOpt 6.9 + others) | ~25 | 2.9 | ~22 |

The MemorySSA rows overlap EarlyCSE and MemCpyOpt: EarlyCse 32.5 = MemorySSA 18.7 + fork/clone 3.4 + walk 8.7 + rest 1.7; MemCpyOpt 10.7 = MemorySSA 6.9 + `Pointers::run` 2.7 + body 0.7.

## Bucket ledger

Excess over the pin in M Ir, one tip. The buckets are `ledger4.py`'s: the inclusive cost of the passes it lists, ours and the pin's, so the rows below add up to the bucket (scalar 137.7 of 137.6). The pin side's MemorySSA (2.9) sits inside its EarlyCSE.

Scalar, ours 137.6 / pin 81.8, excess 55.8 (the pin's 81.8 holds IPSCCP 4.1):

| pass | ours | pin | excess | status |
|---|--:|--:|--:|---|
| EarlyCse: MemorySSA 18.7 vs 2.9 | 18.7 | 2.9 | 15.8 | handed to w-3 (S3), 11-10 |
| EarlyCse: fork + `Context` clone | 3.4 | 0 | 3.4 | handed to w-3, 11-20 |
| EarlyCse: the walk and the rest | 10.4 | 4.9 | 5.5 | unexplained |
| MemCpyOpt: MemorySSA | 6.9 | 0 | 6.9 | handed to w-3 (S3), 11-10 |
| MemCpyOpt: `Pointers::run` | 2.7 | 0 | 2.7 | handed to w-5, 11-27 |
| MemCpyOpt: body | 1.1 | 0.47 | 0.6 | unexplained |
| DeadStoreElimination: MemorySSA (`Accesses::managed` 6.4 + `Mssa::build` 0.8 + dominators 0.3) | 7.5 | 0 | 7.5 | handed to w-3 (S3), 11-10 |
| DeadStoreElimination: `fork` + `Context` clone, as EarlyCse's | 1.7 | 0 | 1.7 | handed to w-3, 11-20 |
| DeadStoreElimination: the walk (`eliminate_dead_defs_of` 3.9, 72 calls) and the rest | 4.3 | 2.8 | 1.5 | unexplained |
| SCCP + IPSCCP | 11.7 | 8.3 | 3.4 | flat, no gap to mirror |
| `Together` (SROA then Mem2Reg, one slot): ledger4 counts SROA twice, as `Sroa` and as `Together` | 9.3 | 0 | 9.3 | instrument error: not a cost, the bucket is 128.3 and its excess 46.5 |
| `Dead` (62 runs of 100 K, at the slots of the pin's InstCombine runs, L677/768/801/837/1242; the pin's ADCE is 11 runs of 87 K) | 6.4 | 0 | 6.4 | stand-in for InstCombine's dead-code removal: w-6's InstCombine buy-back, 11-08 |
| ADCE, BDCE, TRE, Float2Int | 3.7 | 3.1 | 0.6 | unexplained, small |
| GVN | 40.6 | 49.2 | -8.6 | below the pin |
| SROA, Mem2Reg | 9.3 | 9.8 | -0.5 | below the pin |

Handed 37.9 (w-3, w-5) and 6.4 (w-6), flat 3.4, below the pin -9.1, instrument error 9.3, unexplained 7.6 (EarlyCse walk 5.5, DSE walk 1.5, MemCpyOpt body 0.6).

IPO, ours 47.0 / pin 13.3, excess 33.7:

| pass | ours | status |
|---|--:|---|
| `Interprocedural::run` (holds `stamped_all` 10.0) | 14.0 | stamp part handed to w-6, 11-17; the other 4.0 unexplained |
| `Stamp::run` | 3.9 | handed to w-6, 11-17 |
| `Homes` | 8.4 | llrm-only, no pin pass: w-6's retire list, date to set |
| `Spares` | 8.0 | llrm-only, no pin pass: w-6's retire list, date to set |
| `GlobalOpt` 4.8 + `WriteOnly` 2.3 (pin GlobalOpt 0.47) | 7.1 | unexplained, funcattrs-port 11-06 |
| `GlobalDce`, `CalleePop`, `ModuleIpo` and the rest | 5.7 | unexplained |

The pin's 13.3 is not split by pass here: its passes (GlobalOpt 0.47, PostOrderFunctionAttrs 1.4, GlobalDCE 0.15, ...) do not add to it, so part of the IPO excess is a bucket-definition difference. To read in `ledger4.py` by 11-06.

## Steps

One PR per step, ledger pair in each body. Dates are land dates.

| land | owner | step | pin source | M removed |
|---|---|---|---|--:|
| dropped 10-24 | funcattrs-port | functionattrs post-order: 5.7 of the 8.2 M is inside `stamped_all` (the Stamp row, w-6's); the walk's own is 2.5 vs pin 1.4. Skipping unchanged components measured -1.6 M (0.14%), object identical: not landed | FunctionAttrs.cpp `PostOrderFunctionAttrsPass` | 0 |
| dropped 10-24 | funcattrs-port | SCCP: the 3.4 M is flat (no self cost over 1.0 M: `Range::intersect_with`, `icmp`, `Solver::visit`, `get_value_state`), no translation gap to mirror | SCCPSolver.cpp | 0 |
| 11-20 | funcattrs-port | Bounded callers cleanup, delete `Bounded` | none (llrm-only) | ~7 |
| 11-20 | w-3 | EarlyCse fork + `Context` clone: `AliasQuery` takes the function per call (alias.rs), with S2's exit | EarlyCSE.cpp `EarlyCSE::run` | 3.4 |
| 11-27 | w-5 | MemCpyOpt without `Pointers::run` (the llrm-only points-to solve AA reads), with AA call pairs | MemCpyOptimizer.cpp `runImpl` | 2.7 |
| 11-10 | w-3 | MemorySSA once per function (S3) | MemorySSA.cpp | up to 22 |
| 11-17 | w-6 | Stamp 14.0 (`stamped_all` 10.0 + `Stamp` 3.9), llrm-only | none | up to 14 |

Row-23 back half follows, 11-23 to 12-10. `Homes` 8.4 and `Spares` 8.0 are llrm-only module passes on w-6's retire list.
