# Loop and scalar buckets: top callees on d_faces -O2

Owner w-5. Callgrind inclusive Ir of `llrm-cc -m32 -O2` on `qbq-llrm/src/render/d_faces.c` at the tip of `feat/parity`, 1,082 M in all. Inclusive numbers overlap (a callee under another is counted in both) and are upper bounds for what a change can save: F1c got 40% of its plan. Ledger: loop passes 140.7 M against the pin's 86.8 M; scalar 116.7 against 81.8.

By pass, inclusive: Combine 85.8, Licm 50.3, Gvn 48.3, IndVars 39.7 (+6.0 `IndVarsOnCreated`), Cvp 29.4, SimplifyCfg 27.7, EarlyCse 26.7, LoopStrengthReduce 25.5, JumpThreading 19.3, DeadStoreElimination 15.7, LoopFullUnroll 11.9, MemCpyOpt 11.5, Sroa 10.9.

| # | callee | M Ir | under | owner, date |
|---|---|--:|---|---|
| 1 | `MemorySsa::run` (`PlainAccesses` 26.3, `Annotated` 27.1, `PointerValues` 13.3 inside) | 45.7 | Licm, Gvn, MemCpyOpt, DSE | w-3: the Accesses rewrite in flight |
| 2 | `ScalarEvolution::create_scev_iter` (`get_add_expr` 6.9, `get_operands_to_create` 2.1, the rest the phi and select paths) | 27.5 | IndVars, Licm, LSR | w-5, profile row 11-03 (L2: `getAddExpr` 2.3 K against 970 Ir; interning alone showed no gain) |
| 3 | `get_backedge_taken_info` (`compute_exit_limit_from_cond_cached` 25.7, `get_backedge_taken_count` 22.0) | 27.2 | IndVars, LoopDeletion, FullUnroll | w-5, 11-17: re-profile at the tip first |
| 4 | `get_range_ref` (+ `cached_range`) | 21.9 | all SCEV clients | w-5, PR 11-10 (L2b) |
| 5 | `is_implied_cond` guard chain | 19.9 | `get_exit_limit` and `apply_loop_guards` | w-5, PR 11-10 (L2c) |
| 6 | `simplify_users_of_iv` (`eliminate_iv_user` 21.3) | 24.4 | IndVars | w-5, 11-24 |
| 7 | `LoopStrengthReduce` `reduced_held` | 25.5 | LSR | w-5, 12-01 |
| 8 | `MemoryDependenceResults::get_non_local_pointer_dependency` | 27.4 | Gvn | w-3 (with #1) |
| 9 | `LazyValueInfoImpl::solve` | 27.3 | Cvp, JumpThreading | w-4 |
| 10 | `compute_known_bits_into` | 31.4 | Combine, SimplifyCfg, Cvp | funcattrs-port (known bits) |

A row is a profile first (self cost by line, as the F1 rows were) and a change only where the profile names one. Rows 2 to 7 are mine and queue behind L2b/L2c.

## IndVars against the pin, same IR (w-5)

Input: `d_faces -O2`'s IR just before `indvars` in the function that has loops, which `llrm-cc --dump DIR` writes as the file before `walk/N/*indvars*.ll` (the file after the pass is the next one). Ours: `valgrind --tool=callgrind --toggle-collect='*indvars*IndVars*run*' llrm-pass -O2 indvars FILE`. The pin: `--toggle-collect='*IndVarSimplify::run*' opt -passes=indvars -S FILE`. Inclusive Ir, in millions.

| function (pin / ours) | pin | ours | ratio |
|---|--:|--:|--:|
| the pass in all | 13.84 | 25.37 | 1.83 |
| `simplifyUsersOfIV` / `simplify_users_of_iv` | 10.90 | 15.66 | 1.44 |
| `isImpliedCond` | 6.74 | 10.55 | 1.57 |
| `evaluatePredicateAt` | 6.54 | 10.24 | 1.57 |
| `isBasicBlockEntryGuardedByCond` | 5.29 | 9.26 | 1.75 |
| `createSCEVIter` | 4.59 | 7.94 | 1.73 |
| `getRangeRef` | 5.44 | 7.56 | 1.39 |
| `getAddExpr` | 4.99 | 6.99 | 1.40 |
| `getSCEV` | 4.46 | 6.29 | 1.41 |
| `getBackedgeTakenCount` | 3.00 | 5.67 | 1.89 |
| `StrengthenNoWrapFlags` | 4.16 | 5.21 | 1.25 |
| `isKnownViaNonRecursiveReasoning` | 3.06 | 4.45 | 1.45 |
| exit optimization (`optimizeLoopExits`) | <2.9 (rest of the pass) | 5.40 | >1.9 |

The ratio is 1.25 to 1.9 on every SCEV function, not one defect: a constant factor in how expressions are built and looked up, with `getBackedgeTakenCount` (1.89), `isBasicBlockEntryGuardedByCond` (1.75) and `createSCEVIter` (1.73) the widest, and the exit optimization (ours 5.4 M, the pin's under 2.9 of all its other work) the one block ours does that the pin does not itemize. One function, one pass, so the ratios are a guide to where to profile by line, not a promise of savings.

## IndVars against the pin, on a dist build (w-5, correction)

The IndVars table above ran ours from a `release` build (no LTO) against the pin's optimized `opt`: 25.4 M against 13.8 M, 1.83x. On the `dist` build the board uses (fat LTO, one codegen unit) the same pass on the same IR is 18.5 M, 1.33x. The `release` build keeps `llrm_mir::context::mask`, `Fx::write_*`, `Opcode::eq` and the key hash as calls, which a profile then ranks; under dist they are inlined and the flat profile has no function above 1.7% (`get_range_ref` 1.67, `memcpy` 1.37, `is_known_via_non_recursive_reasoning` 1.36, `simplify_icmp_operands` 1.35, `match_binary` 1.25, `Kind::clone` 1.09, `get_constant` 0.97). Profile SCEV costs on dist; the first rows of this document that were read off `release` profiles overstate by up to that 1.4x, and a change measured there (the borrowed `UniqueSCEVs` lookup: -0.35 M of 25.4) is to be re-measured on a dist pair before it counts.

## Exit limit, by line (w-5)

`compute_exit_limit_from_icmp_pred` is 14.3 M inclusive (toggle-collect on it agrees) and has no self cost worth naming (8 K): it is its callees. Inside it, `how_far_to_zero` is 4.4 M, `how_many_less_thans` 3.4 M, and `LoopGuards::collect` about 4.9 M of those and the rest (`collect_from_block` recurses through `collect_from_phi`: 576 calls to the closure, 218 `guard_rewrite`, 103 `guard_calls`), so the inclusive figure of `collect` counts its recursion again. No line or call is a defect: guards, range and `get_add_expr` are the same 1.4 to 1.9x constant factor the IndVars table shows. The row is closed as "no single change"; what is left is that factor, the pin's `get_add_expr` at 970 Ir against ours 2,300, which is `strengthen_no_wrap_flags` (L2, w-3's) and interning (tried, no gain).
