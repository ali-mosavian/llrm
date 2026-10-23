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

Excess over the pin in M Ir, one tip, callee view for the bucket totals (`compile-time-owners.md`) and inclusive per-pass view for the rows. The two views overlap (MemorySSA is inside EarlyCSE and MemCpyOpt), so the rows are what is attributed and `unexplained` is the bucket's excess less them.

Scalar bucket, ours 137.6 / pin 81.8, excess 55.8:

| row | excess | status |
|---|--:|---|
| MemorySSA built per pass (EarlyCse 18.7 + MemCpyOpt 6.9 vs pin 2.9) | 22.7 | handed to w-3 (S3), 11-10 |
| EarlyCse fork + `Context` clone | 3.4 | handed to w-3, 11-20 |
| MemCpyOpt `Pointers::run` | 2.7 | handed to w-5, 11-27 |
| SCCP + IPSCCP | 3.4 | flat, no gap to mirror |
| GVN, SROA | -9.1 | below the pin |
| Reassociate 3.0, DSE 1.1, InstSimplify 1.4 | 5.5 | unexplained: per-pass split not yet made |
| rest of the bucket | 27.2 | unexplained: to attribute by callee (`ledger5.py`), 11-03 |

IPO bucket, ours 47.0 / pin 13.3, excess 33.7:

| row | excess | status |
|---|--:|---|
| Stamp: `stamped_all` 10.0 + `Stamp::run` 3.9 against the pin's 1.4 | 12.5 | handed to w-6, 11-17 |
| Bounded | 7.0 | funcattrs-port, 11-20 |
| GlobalOpt 4.8 vs 0.47, WriteOnly 2.3 | 6.6 | unexplained: pin GlobalOpt mirrored? to read, 11-06 |
| functionattrs post-order in the walk | 1.1 | flat (skipping unchanged components gave -1.6 M) |
| rest of the bucket | 6.5 | unexplained: to attribute by callee, 11-03 |

`Homes` 8.4 and `Spares` 8.0 are llrm-only module passes counted in bucket 4 (w-6).

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
