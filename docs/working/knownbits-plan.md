# Known bits: ours against the pin, per caller

Owner: funcattrs-port (2026-10-24). Pin d1106deb71cc. Ledger command ([ledger-command](ledger-command.md)), d_faces qbq-llrm, dist
builds (`pair_build.sh`), callgrind. Base 71a0c667a: 922.17 M Ir; the pin's clang 508.71 M.

## The premise, corrected
The 9.45 M / 8,082 calls quoted for the pin are one wrapper, `computeKnownBits(V, Q, Depth)`. Counting every entry
(`computeKnownBits` in all its forms, `MaskedValueIsZero`, `isKnownNonNegative`, `WithCache::calculateKnownBits`, InstCombine's
`SimplifyDemanded*` and `ComputeNumSignBits` falling through), the pin makes as many queries as we do:

| | entry calls | entry Ir | `computeKnownBits` calls (all depths) | `FromOperator` calls | self Ir of the family |
|---|--:|--:|--:|--:|--:|
| ours | 16,035 | 25.97 M | 50,804 | 27,599 | 15.66 M |
| pin | 16,871 | 19.39 M | 54,210 | 29,561 | 10.93 M |

So the gap, 6.6 M, is cost per query (1,620 against 1,150 Ir), not query count. `KnownBits` is two `u128` and a width, `Copy`, nothing
allocated (a constant expression builds an `Instruction`: 0.06 M). The limits match the pin: depth 6, the context scan only with a
context instruction, dominating conditions only with `DC` and `DT`, assumptions only with `AC`.

## Per caller (entry calls, inclusive dist Ir)
| Ours | calls | M | Pin site | calls | M | Why ours is more |
|---|--:|--:|---|--:|--:|---|
| `simplify::Query::known` from `simplify_icmp` | 1,349 | 4.72 | `simplifyICmpWithZero` (InstructionSimplify.cpp:3088-3112) | 745 | 1.54 | same code; `simplify_icmp` runs 5,203 times against 4,314, `thread_cmp_over_select` 658 against 155 |
| `simplify::Query::known` from `simplify_shift` | 1,585 | 1.44 | `simplifyShift` | 1,220 | 0.62 | same code; 1.8x per call |
| `simplify::Query::known`, the rest | 406 | 1.18 | `simplifyRightShift`, `simplifyDivRem`, `simplifySubInst` | 284 | 0.41 | |
| `combine_compares::visit_icmp` | 1,968 | 4.59 | `foldICmpUsingKnownBits` -> `SimplifyDemandedBits` | 2,044 | 2.84 | same count; DEVIATE 2 asks `computeKnownBits` where the pin walks `SimplifyDemandedUseBits` |
| `signbits::fall_through` + `compute_num_sign_bits_impl` | 4,658 | 4.51 | `ComputeNumSignBitsImpl` | 4,270 | 3.98 | at par |
| `get_no_common_bits_set_result` | 1,086 | 1.36 | `getNoCommonBitsSetResult` (`WithCache`) | 1,352 | 1.31 | at par |
| `ranges::value_tracking_at` | 1,082 | 1.35 | `analyzeVariableOffsets`, `computeConstantRangeIncludingKnownBits` | 166 + 468 | 0.23 + 0.72 | DEVIATE AN2: every variable index of every access, not per alias query |
| `nonzero::is_known_non_equal_demanded` | 489 | 1.22 | `isKnownNonEqual` | 614 | 1.18 | at par |
| `scev::get_range_ref` | 734 | 1.04 | `getRangeRef` | 562 | 0.58 | |
| `combine_casts::visit_sext` | 367 | 0.68 | `visitSExt` (`isKnownNonNegative`) | 459 | 0.61 | at par |
| `combine_shifts::set_shift_flags` | 498 | 0.62 | `setShiftFlags` | 538 | 0.61 | at par |
| `scev::get_constant_multiple_impl` | 419 | 0.62 | `getConstantMultipleImpl` | 307 | 0.32 | |
| `combine_gep_index::nusw_nneg_is_nuw` | 276 | 0.48 | `visitGetElementPtrInst` | 570 | 1.00 | below the pin |
| `combine_shifts::visit_ashr` | 196 | 0.38 | `visitAShr` (`MaskedValueIsZero`) | 195 | 0.41 | at par |

## Per query: where the 4.7 M of extra self and callee cost is
| Mechanism | Ours calls | Ours M | Pin calls | Pin M |
|---|--:|--:|--:|--:|
| select arms: `adjust_known_bits_for_select_arm` | 3,206 | 5.18 | 740 | 0.14 |
| of which `isGuaranteedNotToBeUndef` of the arm | 371 | 3.60 | 7 | 0.02 |
| operand accessors `int_constant`, `scalar_bits`, `is_pointer` | 123 K | 3.71 | (inline `isa`) | 0.14 |
| `DomConditionCache::registered` -> `findValuesAffectedByCondition` | 870 | 1.18 | 857 | 0.25 |
| `returned_argument_operand` (attribute by string) | 1,167 | 0.38 | 1,961 | 0.12 |
| `match_two_input_recurrence` | 3,305 | 0.34 | 4,824 | 0.09 |

## Rows (one commit or PR each; dist ledger Ir before and after; d_faces object and objsweep unchanged)
| # | Date | Caller or mechanism | Ours (calls, M) | Pin (calls, M) | Fix | Expected M |
|---|---|---|---|---|---|--:|
| 1 | done 2026-10-24 | `compute_known_bits_into` reads the operand twice | 50,804 `int_constant` 1.32, `is_pointer` 0.73 | `isa` checks | one match on the operand kind; the pointer test reuses its type (922.17 -> 921.20 M) | -0.97 |
| 2 | dropped 2026-10-24 | accessor calls | see the per-query table | inline | `#[inline]` on `type_of`, `scalar_bits`, `is_pointer`, `int_constant`, `instruction_of`: +0.03 M, under 0.1 M | 0 |
| 3 | 2026-10-31 | select arms | 3,206, 5.18 | 740, 0.14 | find which selects the queries reach (4.3x the pin's, and 12% informative against 1%): a select the pin has as `smin`/`smax` or folded earlier | -4.0 |
| 4 | 2026-11-03 | `simplify_icmp` | 1,349, 4.72 | 745, 1.54 | same cause as row 3 through `thread_cmp_over_select` (658 against 155); recount after row 3 | -2.0 |
| 5 | part done 2026-10-24; rest 2026-11-05 | `DomConditionCache::registered` | 870, 1.18 | 857, 0.25 | the visited set is `SmallPtrSet<8>`-shaped (done: 921.20 -> 920.99 M, `findValuesAffectedByCondition` 1.18 -> 0.96 M); open: the `logical_and`/`logical_or` matches, 0.35 M | -0.7 |
| 6 | 2026-11-07 | `visit_icmp` | 1,968, 4.59 | 2,044, 2.84 | DEVIATE 2's exit: `SimplifyDemandedBits` on the operands, as `foldICmpUsingKnownBits` | -1.7 |
| 7 | 2026-11-10 | `ranges::value_tracking_at` | 1,082, 1.35 | 634, 0.95 | DEVIATE AN2's exit (BasicAA per query, review 2026-11-13) | -0.4 |
| 8 | 2026-11-12 | `simplify_shift` | 1,585, 1.44 | 1,220, 0.62 | per-call cost after rows 1-3; then the operand mix | -0.6 |
| 9 | 2026-11-14 | SCEV `get_range_ref`, `get_constant_multiple_impl` | 1,153, 1.66 | 869, 0.90 | the pin caches both per SCEV (`UnsignedRanges`, `ConstantMultipleCache`): check ours hits the cache as often | -0.7 |
| 10 | 2026-11-17 | `ComputeNumSignBits` fall-through | 4,658, 4.51 | 4,270, 3.98 | per-call cost after rows 1-3 | -0.5 |
| 11 | 2026-11-18 | `returned_argument_operand` | 1,167, 0.38 | 1,961, 0.12 | `returned` as an attribute kind, not a string compare | -0.25 |
| 12 | 2026-11-20 | `match_two_input_recurrence` | 3,305, 0.34 | 4,824, 0.09 | the pin's shape: operand checks before the phi walk | -0.25 |

A row that gains under 0.1 M is dropped and marked so here.
