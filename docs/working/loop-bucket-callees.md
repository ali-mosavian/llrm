# Loop and scalar buckets: top callees on d_faces -O2

Owner w-5. Callgrind inclusive Ir of `llrm-c -m32 -O2` on `qbq-llrm/src/render/d_faces.c` at the tip of `feat/parity`, 1,082 M in all. Inclusive numbers overlap (a callee under another is counted in both) and are upper bounds for what a change can save: F1c got 40% of its plan. Ledger: loop passes 140.7 M against the pin's 86.8 M; scalar 116.7 against 81.8.

By pass, inclusive: Combine 85.8, Licm 50.3, Gvn 48.3, IndVars 39.7 (+6.0 `IndVarsOnCreated`), Cvp 29.4, SimplifyCfg 27.7, EarlyCse 26.7, LoopStrengthReduce 25.5, JumpThreading 19.3, DeadStoreElimination 15.7, LoopFullUnroll 11.9, MemCpyOpt 11.5, Sroa 10.9.

| # | callee | M Ir | under | owner, date |
|---|---|--:|---|---|
| 1 | `MemorySsa::run` (`PlainAccesses` 26.3, `Annotated` 27.1, `PointerValues` 13.3 inside) | 45.7 | Licm, Gvn, MemCpyOpt, DSE | w-3: the Accesses rewrite in flight |
| 2 | `ScalarEvolution::create_scev_iter` | 27.5 | IndVars, Licm, LSR | w-3: L2 (`getAddExpr` 2.3 K against 970 Ir) |
| 3 | `get_backedge_taken_info` (`compute_exit_limit_from_cond_cached` 25.7, `get_backedge_taken_count` 22.0) | 27.2 | IndVars, LoopDeletion, FullUnroll | w-5, 11-17: re-profile at the tip first |
| 4 | `get_range_ref` (+ `cached_range`) | 21.9 | all SCEV clients | w-5, PR 11-10 (L2b) |
| 5 | `is_implied_cond` guard chain | 19.9 | `get_exit_limit` and `apply_loop_guards` | w-5, PR 11-10 (L2c) |
| 6 | `simplify_users_of_iv` (`eliminate_iv_user` 21.3) | 24.4 | IndVars | w-5, 11-24 |
| 7 | `LoopStrengthReduce` `reduced_held` | 25.5 | LSR | w-5, 12-01 |
| 8 | `MemoryDependenceResults::get_non_local_pointer_dependency` | 27.4 | Gvn | w-3 (handed over), after #1 |
| 9 | `LazyValueInfoImpl::solve` | 27.3 | Cvp, JumpThreading | w-3 (handed over) |
| 10 | `compute_known_bits_into` | 31.4 | Combine, SimplifyCfg, Cvp | w-3 (handed over) |

A row is a profile first (self cost by line, as the F1 rows were) and a change only where the profile names one. Rows 3 to 7 are mine and queue behind L2b/L2c.
