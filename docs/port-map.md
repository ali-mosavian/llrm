# Port map

One row per Python module. A module is `ported` only when `tools/port_diff.py`
shows identical stage dumps and its Python tests are ported. `draft` is Rust
that cites Python but has not passed the dump diff. Phase numbers follow the
plan: 2 C unoptimized, 3 optimizer, 4 QB, 5 BC rewrite.

`src/old/` is the frozen LLVM-style pipeline. Ported code may import from it
only for a module still `todo`, and never after cutover.

| Phase | Python | Lines | Rust | Status | Python tests |
|---|---|---:|---|---|---|
| - | `qbopt/legacy/calls.py` | 839 | `src/legacy/calls.rs` | n/a (legacy) | test_calls, test_runtime, test_select, test_transform |
| - | `qbopt/legacy/lift.py` | 1061 | `src/legacy/lift.rs` | draft | test_calls, test_lift, test_module |
| - | `qbopt/legacy/regalloc.py` | 588 | `src/legacy/regalloc.rs` | n/a (legacy) | test_flow, test_layout |
| - | `qbopt/legacy/simplify.py` | 252 | `src/legacy/simplify.rs` | n/a (legacy) | test_simplify |
| 2 | `qbopt/analysis/alias.py` | 773 | `src/analysis/alias.rs` | todo | test_inline, test_mir_alias |
| 2 | `qbopt/analysis/intervals.py` | 232 | `src/analysis/intervals.rs` | todo | test_allocation, test_coalesce, test_splitkit |
| 2 | `qbopt/analysis/liveness.py` | 159 | `src/analysis/liveness.rs` | draft | test_regalloc, test_word_arithmetic_carry |
| 2 | `qbopt/backend/addressforms.py` | 585 | `src/backend/addressforms.rs` | todo | - |
| 2 | `qbopt/backend/addressvalues.py` | 54 | `src/backend/addressvalues.rs` | todo | - |
| 2 | `qbopt/backend/allocate.py` | 1650 | `src/backend/allocate.rs` | todo | test_allocation, test_constrain, test_cpu_profile, test_dead_call_deliveries, test_interval_redefinitions, test_lir, test_lower_arguments, test_spiller, test_wholeseg |
| 2 | `qbopt/backend/arithmetic.py` | 69 | `src/backend/arithmetic.rs` | todo | test_arithmetic_target |
| 2 | `qbopt/backend/asm.py` | 1079 | `src/backend/asm.rs` | todo | test_loop_exit_layout, test_raising_calls, test_select, test_symbolic_relocation |
| 2 | `qbopt/backend/coalesce.py` | 373 | `src/backend/coalesce.rs` | todo | test_coalesce, test_flow |
| 2 | `qbopt/backend/comparefold.py` | 109 | `src/backend/comparefold.rs` | todo | test_memory_folding |
| 2 | `qbopt/backend/constrain.py` | 448 | `src/backend/constrain.rs` | todo | test_constrain |
| 2 | `qbopt/backend/copyprop.py` | 248 | `src/backend/copyprop.rs` | todo | - |
| 2 | `qbopt/backend/copysink.py` | 182 | `src/backend/copysink.rs` | todo | test_copysink |
| 2 | `qbopt/backend/cpu.py` | 221 | `src/backend/cpu.rs` | todo | test_cpu_profile, test_floatalloc, test_inline, test_quality, test_schedule |
| 2 | `qbopt/backend/division.py` | 101 | `src/backend/division.rs` | todo | test_reciprocal, test_timing_bounds |
| 2 | `qbopt/backend/farcall.py` | 56 | `src/backend/farcall.rs` | todo | - |
| 2 | `qbopt/backend/farload.py` | 141 | `src/backend/farload.rs` | todo | - |
| 2 | `qbopt/backend/floatalloc.py` | 1086 | `src/backend/floatalloc.rs` | todo | test_cpu_profile, test_float_values, test_floatalloc, test_raising_copies |
| 2 | `qbopt/backend/floatregions.py` | 195 | `src/backend/floatregions.rs` | todo | - |
| 2 | `qbopt/backend/fpu.py` | 113 | `src/backend/fpu.rs` | todo | test_fpu |
| 2 | `qbopt/backend/frame.py` | 107 | `src/backend/frame.rs` | todo | test_constrain, test_floatalloc, test_flow, test_frame, test_jumps, test_lower_arguments, test_native_frame, test_native_stack_arguments, test_peephole, test_prologue, test_rematerialized_definitions, test_spiller |
| 2 | `qbopt/backend/jumps.py` | 551 | `src/backend/jumps.rs` | todo | - |
| 2 | `qbopt/backend/layout.py` | 619 | `src/backend/layout.rs` | todo | test_emission_order, test_layout, test_wholeseg |
| 2 | `qbopt/backend/liveness.py` | 142 | `src/backend/liveness.rs` | todo | test_liveness |
| 2 | `qbopt/backend/lower.py` | 2117 | `src/backend/lower.rs` | todo | test_addressforms, test_basic_semantics, test_c_segment_addresses, test_call_clobbers, test_constrain, test_consts, test_dead_ownership, test_far_load_identity, test_file_contracts, test_float_values, test_floatalloc, test_floatfold, test_flow, test_frame_addresses, test_hir, test_lir, test_lir_emission_order, test_lower_conditions, test_lower_switches, test_masm, test_native_returns, test_opaque_relocations, test_pointer_offset, test_raising_addresses, test_raising_calls, test_spiller, test_stack_segment, test_symbolic_relocation, test_transform, test_unroll |
| 2 | `qbopt/backend/lower_floats.py` | 35 | `src/backend/lower_floats.rs` | draft | test_float_values, test_sccp, test_unroll |
| 2 | `qbopt/backend/lower_int64.py` | 560 | `src/backend/lower_int64.rs` | todo | test_cfront, test_modern_frontend |
| 2 | `qbopt/backend/lower_switches.py` | 129 | `src/backend/lower_switches.rs` | draft | - |
| 2 | `qbopt/backend/machinecse.py` | 245 | `src/backend/machinecse.rs` | todo | test_machinecse |
| 2 | `qbopt/backend/machinedce.py` | 135 | `src/backend/machinedce.rs` | todo | test_machinedce |
| 2 | `qbopt/backend/masm.py` | 423 | `src/backend/masm.rs` | todo | test_cfront_frontend, test_hir, test_hir_execute, test_memory_folding, test_modern_frontend, test_peephole |
| 2 | `qbopt/backend/nativeframe.py` | 325 | `src/backend/nativeframe.rs` | todo | test_native_frame, test_native_stack_arguments |
| 2 | `qbopt/backend/omfwrite.py` | 675 | `src/backend/omfwrite.rs` | todo | test_cbench_e2e, test_cfront_frontend, test_cfront_int64_e2e, test_frontend_parity, test_spiller, test_wholeseg |
| 2 | `qbopt/backend/parcopy.py` | 258 | `src/backend/parcopy.rs` | todo | test_lir_verify, test_parcopy, test_wholeseg |
| 2 | `qbopt/backend/peephole.py` | 2683 | `src/backend/peephole.rs` | todo | test_addressforms, test_dead_address_arithmetic, test_lir_emission_order, test_liveness, test_machine_copyprop, test_prologue |
| 2 | `qbopt/backend/phielim.py` | 508 | `src/backend/phielim.rs` | todo | test_floatalloc, test_flow, test_lir, test_spiller |
| 2 | `qbopt/backend/pointers.py` | 55 | `src/backend/pointers.rs` | draft | test_pointer_offset |
| 2 | `qbopt/backend/prologue.py` | 198 | `src/backend/prologue.rs` | todo | test_flow, test_parcopy |
| 2 | `qbopt/backend/reencode.py` | 225 | `src/backend/reencode.rs` | n/a (unused) | test_reencode |
| 2 | `qbopt/backend/regthrash.py` | 255 | `src/backend/regthrash.rs` | todo | - |
| 2 | `qbopt/backend/rmw.py` | 332 | `src/backend/rmw.rs` | todo | test_rmw |
| 2 | `qbopt/backend/schedule.py` | 319 | `src/backend/schedule.rs` | todo | - |
| 2 | `qbopt/backend/select.py` | 1784 | `src/backend/select.rs` | todo | test_address_roles, test_allocation, test_allocation_hints, test_arithmetic_immediates, test_coalesce, test_constrain, test_e2e, test_float_register_arithmetic, test_flow, test_fpu, test_layout, test_lower_conditions, test_lower_switches, test_parcopy, test_peephole, test_postallocation, test_regthrash, test_select |
| 2 | `qbopt/backend/spiller.py` | 1807 | `src/backend/spiller.rs` | todo | test_constrain, test_flow, test_rematerialized_definitions, test_spiller, test_wholeseg |
| 2 | `qbopt/backend/spillforward.py` | 176 | `src/backend/spillforward.rs` | todo | test_peephole |
| 2 | `qbopt/backend/splitkit.py` | 485 | `src/backend/splitkit.rs` | todo | test_splitkit |
| 2 | `qbopt/backend/storecombine.py` | 52 | `src/backend/storecombine.rs` | todo | - |
| 2 | `qbopt/backend/target.py` | 341 | `src/backend/target.rs` | todo | test_coalesce, test_constrain, test_flow, test_lir, test_regalloc, test_select, test_selectors, test_strength_offsets |
| 2 | `qbopt/backend/timing.py` | 42 | `src/backend/timing.rs` | todo | test_timing_bounds |
| 2 | `qbopt/backend/twoaddr.py` | 244 | `src/backend/twoaddr.rs` | todo | test_lir |
| 2 | `qbopt/backend/verify.py` | 200 | `src/backend/verify.rs` | todo | test_addressforms, test_flow, test_memory_folding, test_peephole, test_phi_widths, test_regthrash |
| 2 | `qbopt/cfront/compile.py` | 779 | `src/cfront/compile.rs` | draft | test_cfront, test_cfront_frontend, test_cpu_profile, test_float_values, test_frontend_parity, test_indvars, test_inline, test_jumps, test_omfwrite, test_quality, test_spiller, test_stack_segment, test_unroll |
| 2 | `qbopt/cfront/hir.py` | 268 | `src/cfront/hir.rs` | ported | test_cfront_frontend |
| 2 | `qbopt/cfront/libfunc.py` | 36 | `src/cfront/libfunc.rs` | todo | - |
| 2 | `qbopt/cfront/raise_hir.py` | 1630 | `src/cfront/raise_hir.rs` | draft | test_algebraic, test_cfront_frontend |
| 2 | `qbopt/cfront/stream.py` | 65 | `src/cfront/stream.rs` | ported | test_cfront_frontend |
| 2 | `qbopt/cycles/cycles.py` | 478 | `src/cycles/cycles.rs` | todo | test_cpu_profile |
| 2 | `qbopt/cycles/timings.py` | 239 | `src/cycles/timings.rs` | draft | - |
| 2 | `qbopt/flow.py` | 198 | `src/flow.rs` | todo | test_allocation, test_coalesce, test_cpu_profile, test_flow, test_jumps, test_lir_verify, test_native_frame, test_parcopy, test_raising_copies, test_spiller, test_wholeseg |
| 2 | `qbopt/model/floating.py` | 48 | `src/model/floating.rs` | draft | test_cfront, test_cse_commutative, test_float_cse_paths, test_float_values, test_floatbounds, test_floatfacts, test_floatfold, test_loopclone, test_native_float_licm, test_sccp, test_unroll |
| 2 | `qbopt/model/ir.py` | 1450 | `src/model/ir/` | draft | test_address_roles, test_addressforms, test_algebraic, test_allocation, test_allocation_hints, test_array_bounds, test_avail, test_availability_call_effects, test_availability_operands, test_c_segment_addresses, test_call_clobbers, test_cfg_empty, test_cfg_merge, test_coalesce, test_constant_call_memory, test_constant_carry, test_constant_cells, test_constant_division, test_constrain, test_consts, test_copy_values, test_copysink, test_cse_commutative, test_cse_dominance, test_dead_address_arithmetic, test_dead_call_deliveries, test_dead_ownership, test_e2e, test_emission_order, test_extract, test_far_load_identity, test_farload, test_float_constants, test_float_cse_paths, test_float_loop_exit, test_float_register_arithmetic, test_float_values, test_floatalloc, test_floatfold, test_floating, test_flow, test_frame, test_frame_addresses, test_frame_escape, test_frontend_parity, test_gvn_join, test_high_product, test_induction_identity, test_indvars, test_inline, test_invariant_shift, test_invariant_values, test_ir, test_layout, test_lcssa, test_lir, test_lir_emission_order, test_literal_initializers, test_liveness, test_load_pre, test_loop_exit_layout, test_lower_arguments, test_lower_conditions, test_lower_sign, test_lower_switches, test_machine_copyprop, test_machinecse, test_masm, test_memory_cse, test_memory_folding, test_memory_joins, test_memory_opportunities, test_mir, test_mir_alias, test_multiply_select, test_native_frame, test_native_returns, test_native_stack_arguments, test_omfwrite, test_pairs, test_parcopy, test_peephole, test_phi_widths, test_pointer_memory, test_postallocation, test_private_frame, test_prologue, test_promote, test_quality, test_raising_arrays, test_raising_frame, test_ranges, test_regalloc, test_regthrash, test_rematerialized_definitions, test_rewind, test_rmw, test_runtime_cells, test_scale_selection, test_scaled_addressing, test_sccp, test_schedule, test_select, test_select_float_stack, test_simplify, test_spiller, test_splitkit, test_ssa_phis, test_stack_segment, test_stages, test_store_combine, test_test_immediate, test_transform, test_unroll, test_unroll_budget, test_unswitch, test_wholeseg, test_wide |
| 2 | `qbopt/model/lir.py` | 360 | `src/model/lir.rs` | draft | test_allocation, test_countdown, test_cpu_profile, test_emission_order, test_floatalloc, test_flow, test_hir, test_jumps, test_lir_verify, test_machinedce, test_omfwrite, test_pointer_memory, test_quality, test_select, test_spiller, test_store_combine, test_wholeseg |
| 2 | `qbopt/model/memory.py` | 142 | `src/model/memory.rs` | draft | test_loopclone, test_mir_alias, test_private_frame, test_sccp |
| 2 | `qbopt/model/mir.py` | 3383 | `src/model/mir.rs` | draft | test_addressforms, test_allocation, test_avail, test_cfg_empty, test_cfg_merge, test_constant_cells, test_constrain, test_countdown, test_cse_commutative, test_cse_dominance, test_emission_order, test_extract, test_farload, test_file_contracts, test_float_constants, test_float_loop_exit, test_floatalloc, test_floating, test_flow, test_frontend_parity, test_gvn_join, test_high_product, test_hoist_selector, test_induction_inequality, test_indvars, test_invariant_argument, test_invariant_shift, test_laststore, test_layout, test_lcssa_merges, test_licm_operand_dependencies, test_lir, test_lir_emission_order, test_literal_call_contracts, test_literal_initializers, test_liveness, test_load_pre, test_loopexit, test_loopmotion, test_loopsimplify, test_lower_sign, test_masm, test_memory_folding, test_memory_joins, test_memory_opportunities, test_mir, test_native_float_licm, test_native_frame, test_native_status_flags, test_noreturn, test_observers, test_opaque_memory_effects, test_opaque_relocations, test_pairs, test_peephole, test_phi_widths, test_pointer_memory, test_postallocation, test_raising_addresses, test_raising_call_memory, test_raising_dispatch, test_raising_longs, test_raising_unary, test_ranges, test_regions, test_rounding_contracts, test_rule5, test_scalar_division, test_scale_selection, test_scoreboard, test_select, test_spiller, test_switch_loopclone, test_symbol_licm, test_transform, test_unary_promotion, test_unroll, test_unsigned_edge_ranges, test_word_arithmetic_carry |
| 2 | `qbopt/model/passes.py` | 226 | `src/model/passes.rs` | todo | test_cpu_profile, test_flow, test_lir_verify, test_mir_alias, test_promote, test_rewind, test_rule5, test_transform, test_unroll, test_unroll_budget, test_unswitch |
| 3 | `qbopt/analysis/avail.py` | 555 | `src/analysis/avail.rs` | todo | test_avail, test_availability_call_effects, test_availability_operands, test_far_memory_identity, test_float_values, test_memoryssa_forward, test_observers, test_qgldiff_forwarding |
| 3 | `qbopt/analysis/constant_cycles.py` | 123 | `src/analysis/constant_cycles.rs` | draft | - |
| 3 | `qbopt/analysis/consts.py` | 769 | `src/analysis/consts.rs` | draft | test_array_access, test_array_bounds, test_constant_arguments, test_constant_call_memory, test_constant_carry, test_constant_conditions, test_constant_cycles, test_constant_division, test_constant_index, test_constant_stores, test_float_recurrences, test_floatfacts, test_high_product, test_induction_identity, test_induction_inequality, test_indvars, test_loopexit, test_pointer_constants, test_promote, test_raising_arrays, test_raising_copies, test_raising_dispatch, test_raising_longs, test_rewind, test_transform |
| 3 | `qbopt/analysis/effects.py` | 30 | `src/analysis/effects.rs` | todo | test_far_load_identity, test_opaque_memory_effects |
| 3 | `qbopt/analysis/flags.py` | 111 | `src/analysis/flags.rs` | draft (Flag, ALL, DIVERGENT) | test_calls, test_ir, test_lift |
| 3 | `qbopt/analysis/floatbounds.py` | 153 | `src/analysis/floatbounds.rs` | todo | test_floatbounds |
| 3 | `qbopt/analysis/floatfacts.py` | 356 | `src/analysis/floatfacts.rs` | todo | test_float_loop_exit, test_float_recurrences, test_float_values, test_floatbounds, test_floatfacts, test_floatfold, test_literal_initializers |
| 3 | `qbopt/analysis/frameescape.py` | 170 | `src/analysis/frameescape.rs` | draft | test_frame_escape |
| 3 | `qbopt/analysis/induction.py` | 1322 | `src/analysis/induction.rs` | draft | test_induction_identity, test_induction_inequality, test_indvars, test_loopexit, test_mir, test_modern_frontend, test_quotient_recurrence, test_rewind, test_unroll |
| 3 | `qbopt/analysis/interprocedural.py` | 487 | `src/analysis/interprocedural.rs` | todo | test_sccp |
| 3 | `qbopt/analysis/loops.py` | 316 | `src/analysis/loops.rs` | draft | test_algebraic, test_countdown, test_float_values, test_hir, test_induction_identity, test_induction_inequality, test_indvars, test_invariant_argument, test_jumps, test_laststore, test_loopclone, test_loopexit, test_loopmotion, test_loops, test_promote, test_quotient_recurrence, test_rewind, test_scoreboard, test_unswitch |
| 3 | `qbopt/analysis/memoryssa.py` | 204 | `src/analysis/memoryssa.rs` | todo | test_memoryssa |
| 3 | `qbopt/analysis/noreturn.py` | 87 | `src/analysis/noreturn.rs` | todo | test_noreturn, test_sccp |
| 3 | `qbopt/analysis/observers.py` | 352 | `src/analysis/observers.rs` | todo | test_private_frame |
| 3 | `qbopt/analysis/pointerfacts.py` | 65 | `src/analysis/pointerfacts.rs` | todo | test_pointer_constants |
| 3 | `qbopt/analysis/ranges.py` | 369 | `src/analysis/ranges.rs` | draft | test_array_facts, test_cfg_merge, test_edge_ranges, test_floatbounds, test_mir_alias, test_promote, test_regions |
| 3 | `qbopt/analysis/regions.py` | 391 | `src/analysis/regions.rs` | draft | test_external_cells |
| 3 | `qbopt/analysis/ssa.py` | 344 | `src/analysis/ssa.rs` | draft | test_algebraic, test_float_values, test_induction_identity, test_mir, test_pointer_memory, test_rule5, test_ssa_phis, test_ssa_unreachable, test_transform |
| 3 | `qbopt/optimize/algebraic.py` | 1027 | `src/optimize/algebraic.rs` | todo | test_algebraic, test_invariant_values |
| 3 | `qbopt/optimize/cfg.py` | 77 | `src/optimize/cfg.rs` | draft | - |
| 3 | `qbopt/optimize/edges.py` | 40 | `src/optimize/edges.rs` | draft | - |
| 3 | `qbopt/optimize/exitsink.py` | 107 | `src/optimize/exitsink.rs` | todo | - |
| 3 | `qbopt/optimize/fill.py` | 301 | `src/optimize/fill.rs` | todo | - |
| 3 | `qbopt/optimize/floatfold.py` | 234 | `src/optimize/floatfold.rs` | todo | test_floatfold, test_unroll |
| 3 | `qbopt/optimize/floatloop.py` | 140 | `src/optimize/floatloop.rs` | todo | test_float_loop_exit |
| 3 | `qbopt/optimize/gvn.py` | 206 | `src/optimize/gvn.rs` | todo | test_mir_alias |
| 3 | `qbopt/optimize/indvars.py` | 1155 | `src/optimize/indvars.rs` | draft | test_induction_identity, test_indvars, test_loopexit |
| 3 | `qbopt/optimize/inline.py` | 462 | `src/optimize/inline.rs` | todo | - |
| 3 | `qbopt/optimize/ivshare.py` | 117 | `src/optimize/ivshare.rs` | todo | test_ivshare |
| 3 | `qbopt/optimize/lcssa.py` | 134 | `src/optimize/lcssa.rs` | todo | test_layout, test_lcssa, test_lcssa_merges |
| 3 | `qbopt/optimize/lcssamerges.py` | 119 | `src/optimize/lcssamerges.rs` | todo | - |
| 3 | `qbopt/optimize/loadjoins.py` | 154 | `src/optimize/loadjoins.rs` | todo | test_load_pre |
| 3 | `qbopt/optimize/loopclone.py` | 214 | `src/optimize/loopclone.rs` | todo | test_layout, test_loopclone, test_switch_loopclone |
| 3 | `qbopt/optimize/loopexit.py` | 399 | `src/optimize/loopexit.rs` | todo | test_induction_identity, test_loopexit |
| 3 | `qbopt/optimize/loopmotion.py` | 251 | `src/optimize/loopmotion.rs` | todo | test_float_loop_exit, test_laststore |
| 3 | `qbopt/optimize/loopsimplify.py` | 122 | `src/optimize/loopsimplify.rs` | todo | - |
| 3 | `qbopt/optimize/peel.py` | 123 | `src/optimize/peel.rs` | todo | test_transform, test_unroll |
| 3 | `qbopt/optimize/pointeraccess.py` | 133 | `src/optimize/pointeraccess.rs` | todo | - |
| 3 | `qbopt/optimize/profit.py` | 264 | `src/optimize/profit.rs` | todo | test_unroll_budget |
| 3 | `qbopt/optimize/promote.py` | 1105 | `src/optimize/promote.rs` | todo | test_flow, test_mir_alias, test_promote, test_transform, test_unary_promotion |
| 3 | `qbopt/optimize/rotate.py` | 406 | `src/optimize/rotate.rs` | draft | test_countdown, test_induction_identity, test_rewind |
| 3 | `qbopt/optimize/strength.py` | 1359 | `src/optimize/strength.rs` | draft | test_cpu_profile, test_flow, test_ivshare, test_ranges |
| 3 | `qbopt/optimize/transform.py` | 3297 | `src/optimize/transform.rs` | draft | test_cfront, test_constant_arguments, test_constant_stores, test_consts, test_copy_values, test_cpu_driver, test_dead_ownership, test_far_memory_identity, test_float_cse_paths, test_float_loop_exit, test_float_recurrences, test_float_values, test_flow, test_hoist_selector, test_induction_inequality, test_invariant_argument, test_ivshare, test_laststore, test_licm_operand_dependencies, test_loopexit, test_loopmotion, test_loopsimplify, test_lower_switches, test_memory_cse, test_mir, test_native_float_licm, test_parcopy, test_phi_widths, test_pointer_memory, test_promote, test_qgldiff_forwarding, test_quotient_recurrence, test_raising_calls, test_raising_copies, test_raising_dispatch, test_ranges, test_rule5, test_scalar_division, test_sccp, test_transform, test_unroll |
| 3 | `qbopt/optimize/unroll.py` | 449 | `src/optimize/unroll.rs` | todo | test_transform, test_unroll |
| 3 | `qbopt/optimize/unswitch.py` | 218 | `src/optimize/unswitch.rs` | todo | test_indvars, test_unswitch |
| 3 | `qbopt/optimize/wholephis.py` | 100 | `src/optimize/wholephis.rs` | todo | test_algebraic |
| 3 | `qbopt/optimize/wholestores.py` | 37 | `src/optimize/wholestores.rs` | todo | - |
| 4 | `qbopt/frontend/modern/compile.py` | 200 | `src/frontend/modern/compile.rs` | todo | test_hir_execute |
| 4 | `qbopt/frontend/modern/driver.py` | 53 | `src/frontend/modern/driver.rs` | todo | - |
| 4 | `qbopt/frontend/qb/__init__.py` | 23 | `src/frontend/qb/__init__.rs` | todo | test_hir, test_modern_frontend, test_qb_frontend_command, test_qbstages |
| 4 | `qbopt/frontend/qb/__main__.py` | 64 | `src/frontend/qb/__main__.rs` | todo | - |
| 4 | `qbopt/frontend/qb/abi.py` | 1339 | `src/frontend/qb/abi.rs` | todo | test_hir |
| 4 | `qbopt/frontend/qb/compile.py` | 1645 | `src/frontend/qb/compile.rs` | todo | - |
| 4 | `qbopt/frontend/qb/driver.py` | 204 | `src/frontend/qb/driver.rs` | todo | test_hir, test_qb_frontend_command |
| 4 | `qbopt/frontend/qb/inline_x87.py` | 64 | `src/frontend/qb/inline_x87.rs` | todo | - |
| 4 | `qbopt/frontend/qb/stage_text.py` | 178 | `src/frontend/qb/stage_text.rs` | n/a (tools only) | - |
| 4 | `qbopt/hir/__init__.py` | 93 | `src/hir/__init__.rs` | todo | test_hir, test_hir_execute, test_modern_e2e, test_modern_frontend, test_qbstages |
| 4 | `qbopt/hir/__main__.py` | 29 | `src/hir/__main__.rs` | todo | - |
| 4 | `qbopt/hir/callmemory.py` | 71 | `src/hir/callmemory.rs` | todo | - |
| 4 | `qbopt/hir/codec.py` | 129 | `src/hir/codec.rs` | todo | - |
| 4 | `qbopt/hir/dump.py` | 141 | `src/hir/dump.rs` | todo | - |
| 4 | `qbopt/hir/execute.py` | 438 | `src/hir/execute.rs` | n/a (tools only) | test_modern_e2e |
| 4 | `qbopt/hir/lower.py` | 1438 | `src/hir/lower.rs` | todo | - |
| 4 | `qbopt/hir/model.py` | 370 | `src/hir/model.rs` | todo | test_hir |
| 4 | `qbopt/hir/verify.py` | 447 | `src/hir/verify.rs` | todo | - |
| 5 | `qbopt/abi/callsite.py` | 52 | `src/abi/callsite.rs` | todo | - |
| 5 | `qbopt/abi/events.py` | 52 | `src/abi/events.rs` | todo | test_runtime |
| 5 | `qbopt/abi/handlers.py` | 47 | `src/abi/handlers.rs` | todo | test_extent |
| 5 | `qbopt/abi/inputscan.py` | 864 | `src/abi/inputscan.rs` | todo | - |
| 5 | `qbopt/abi/linkunit.py` | 179 | `src/abi/linkunit.rs` | todo | test_array_access, test_basic_semantics, test_contract_profile, test_cpu_driver |
| 5 | `qbopt/abi/nativecalls.py` | 122 | `src/abi/nativecalls.rs` | todo | test_native_frame |
| 5 | `qbopt/abi/ports.py` | 15 | `src/abi/ports.rs` | todo | - |
| 5 | `qbopt/abi/profile.py` | 113 | `src/abi/profile.rs` | todo | - |
| 5 | `qbopt/abi/runtime.py` | 1436 | `src/abi/runtime.rs` | todo | test_allocation, test_callsite_abi, test_cfg_merge, test_coalesce, test_constant_call_memory, test_environ_contract, test_float_values, test_floatbounds, test_flow, test_huge_array_access, test_invariant_shift, test_ivshare, test_lir, test_literal_initializers, test_loopmotion, test_mir, test_noreturn, test_numeric_argument_escape, test_parcopy, test_raising_calls, test_redim_contract, test_regions, test_rounding_contracts, test_rule5, test_runtime, test_runtime_cells, test_transform |
| 5 | `qbopt/frontend/addressfacts.py` | 36 | `src/frontend/addressfacts.rs` | todo | test_array_facts |
| 5 | `qbopt/frontend/arrayfacts.py` | 403 | `src/frontend/arrayfacts.rs` | todo | test_array_facts, test_cfg_merge |
| 5 | `qbopt/frontend/blocks.py` | 671 | `src/frontend/blocks.rs` | todo | test_algebraic, test_allocation, test_array_facts, test_avail, test_basic_semantics, test_blocks, test_c_discovery, test_c_segment_addresses, test_cfg_merge, test_coalesce, test_dispatch_edges, test_e2e, test_extent, test_float_constants, test_float_identity, test_float_values, test_flow, test_fpu, test_induction_identity, test_ir, test_layout, test_licm_operand_dependencies, test_lir, test_loopexit, test_loops, test_mir, test_native_status_flags, test_observers, test_pairs, test_parcopy, test_peephole, test_raising_addresses, test_regions, test_runtime, test_scoreboard, test_select, test_simplify, test_spiller, test_stack, test_stages, test_transform, test_twoaddr, test_unswitch, test_wholeseg |
| 5 | `qbopt/frontend/declen.py` | 265 | `src/frontend/declen.rs` | draft | test_addressforms, test_blocks, test_c_segment_addresses, test_calls, test_callsite_abi, test_declen, test_e2e, test_float_register_arithmetic, test_fpu, test_ir, test_layout, test_lift, test_machine_copyprop, test_peephole, test_raising_copies, test_raising_frame, test_reencode, test_runtime, test_select, test_stack, test_stack_segment, test_test_immediate |
| 5 | `qbopt/frontend/extent.py` | 244 | `src/frontend/extent.rs` | todo | test_extent, test_extent_cv, test_ir, test_native_frame, test_native_stack_arguments |
| 5 | `qbopt/frontend/fppatches.py` | 70 | `src/frontend/fppatches.rs` | todo | test_c_discovery, test_far_load_identity, test_float_constants, test_float_register_arithmetic, test_licm_operand_dependencies, test_opaque_memory_effects, test_unary_promotion |
| 5 | `qbopt/frontend/fpstack.py` | 208 | `src/frontend/fpstack.rs` | todo | test_fpstack |
| 5 | `qbopt/frontend/pairs.py` | 410 | `src/frontend/pairs.rs` | todo | - |
| 5 | `qbopt/frontend/raising_address_state.py` | 137 | `src/frontend/raising_address_state.rs` | todo | - |
| 5 | `qbopt/frontend/raising_addresses.py` | 104 | `src/frontend/raising_addresses.rs` | todo | - |
| 5 | `qbopt/frontend/raising_array_access.py` | 397 | `src/frontend/raising_array_access.rs` | todo | test_array_access, test_huge_array_access |
| 5 | `qbopt/frontend/raising_array_bounds.py` | 285 | `src/frontend/raising_array_bounds.rs` | todo | test_array_bounds, test_array_facts |
| 5 | `qbopt/frontend/raising_arrays.py` | 161 | `src/frontend/raising_arrays.rs` | todo | - |
| 5 | `qbopt/frontend/raising_bytes.py` | 54 | `src/frontend/raising_bytes.rs` | todo | test_raising_bytes |
| 5 | `qbopt/frontend/raising_call_memory.py` | 558 | `src/frontend/raising_call_memory.rs` | todo | test_raising_call_memory |
| 5 | `qbopt/frontend/raising_calls.py` | 255 | `src/frontend/raising_calls.rs` | todo | - |
| 5 | `qbopt/frontend/raising_carried.py` | 73 | `src/frontend/raising_carried.rs` | todo | - |
| 5 | `qbopt/frontend/raising_conditions.py` | 45 | `src/frontend/raising_conditions.rs` | todo | - |
| 5 | `qbopt/frontend/raising_control.py` | 21 | `src/frontend/raising_control.rs` | todo | - |
| 5 | `qbopt/frontend/raising_copies.py` | 185 | `src/frontend/raising_copies.rs` | todo | test_raising_copies |
| 5 | `qbopt/frontend/raising_defseg.py` | 234 | `src/frontend/raising_defseg.rs` | todo | - |
| 5 | `qbopt/frontend/raising_dispatch.py` | 123 | `src/frontend/raising_dispatch.rs` | todo | - |
| 5 | `qbopt/frontend/raising_division.py` | 77 | `src/frontend/raising_division.rs` | todo | - |
| 5 | `qbopt/frontend/raising_fields.py` | 69 | `src/frontend/raising_fields.rs` | todo | - |
| 5 | `qbopt/frontend/raising_float_calls.py` | 244 | `src/frontend/raising_float_calls.rs` | todo | - |
| 5 | `qbopt/frontend/raising_float_results.py` | 56 | `src/frontend/raising_float_results.rs` | todo | - |
| 5 | `qbopt/frontend/raising_float_values.py` | 175 | `src/frontend/raising_float_values.rs` | todo | test_fpstack |
| 5 | `qbopt/frontend/raising_floats.py` | 113 | `src/frontend/raising_floats.rs` | draft | - |
| 5 | `qbopt/frontend/raising_frame.py` | 172 | `src/frontend/raising_frame.rs` | todo | test_raising_frame |
| 5 | `qbopt/frontend/raising_literals.py` | 199 | `src/frontend/raising_literals.rs` | todo | test_literal_initializers, test_raising_copies |
| 5 | `qbopt/frontend/raising_longs.py` | 515 | `src/frontend/raising_longs.rs` | todo | test_raising_longs, test_raising_unary |
| 5 | `qbopt/frontend/raising_numeric_policy.py` | 47 | `src/frontend/raising_numeric_policy.rs` | todo | test_float_cse_paths |
| 5 | `qbopt/frontend/raising_returns.py` | 31 | `src/frontend/raising_returns.rs` | todo | - |
| 5 | `qbopt/frontend/raising_words.py` | 122 | `src/frontend/raising_words.rs` | todo | test_raising_words, test_word_arithmetic_carry |
| 5 | `qbopt/frontend/stack.py` | 131 | `src/frontend/stack.rs` | todo | test_stack |
| 5 | `qbopt/frontend/wide.py` | 312 | `src/frontend/wide.rs` | n/a (tools only) | test_wide |
| 5 | `qbopt/objectfile/addends.py` | 48 | `src/objectfile/addends.rs` | todo | test_omf_addends |
| 5 | `qbopt/objectfile/cvinfo.py` | 777 | `src/objectfile/cvinfo.rs` | todo | test_cvinfo |
| 5 | `qbopt/objectfile/module.py` | 573 | `src/objectfile/module.rs` | draft (addresses) | test_address_roles, test_addressforms, test_algebraic, test_allocation, test_arithmetic_immediates, test_array_access, test_array_bounds, test_array_facts, test_avail, test_availability_call_effects, test_availability_operands, test_blocks, test_cfg_merge, test_coalesce, test_constant_arguments, test_constant_call_memory, test_constant_cells, test_constant_index, test_constant_stores, test_constrain, test_consts, test_countdown, test_dead_ownership, test_dispatch_edges, test_e2e, test_edge_ranges, test_extent, test_extent_cv, test_extract, test_far_memory_identity, test_farload, test_float_constants, test_float_cse_paths, test_float_values, test_floatalloc, test_floatbounds, test_floatfacts, test_floatfold, test_flow, test_folded_relocations, test_gvn_join, test_hir, test_huge_array_access, test_in_place, test_induction_identity, test_indvars, test_inline, test_ir, test_layout, test_lift, test_lir, test_literal_initializers, test_load_pre, test_loopmotion, test_lower_arguments, test_lower_conditions, test_machinecse, test_masm, test_memory_folding, test_memory_joins, test_memory_opportunities, test_memoryssa, test_memoryssa_forward, test_mir, test_mir_alias, test_multiply_select, test_numeric_argument_escape, test_observers, test_omf_addends, test_omfwrite, test_pairs, test_parcopy, test_peephole, test_pointer_constants, test_pointer_memory, test_pointer_offset, test_postallocation, test_private_frame, test_promote, test_raising_addresses, test_raising_arrays, test_raising_call_memory, test_raising_calls, test_raising_frame, test_raising_longs, test_ranges, test_regions, test_rewrite, test_runtime, test_runtime_cells, test_scaled_addressing, test_sccp, test_schedule, test_scoreboard, test_select, test_simplify, test_spiller, test_splitkit, test_stack_segment, test_stages, test_store_combine, test_symbolic_relocation, test_test_immediate, test_transform, test_twoaddr, test_unroll, test_wholeseg, test_wide |
| 5 | `qbopt/objectfile/omf.py` | 904 | `src/objectfile/omf.rs` | draft (ValueError subclasses panic) | test_allocation, test_array_access, test_cfg_merge, test_coalesce, test_constant_arguments, test_constant_call_memory, test_constant_cells, test_contract_profile, test_contracts, test_cvinfo, test_edge_ranges, test_entry_contract, test_extent, test_extent_cv, test_float_values, test_floatfacts, test_flow, test_folded_relocations, test_gvn_join, test_hir, test_huge_array_access, test_layout, test_lir, test_load_pre, test_memory_joins, test_mir, test_module, test_omf, test_omf_addends, test_omfwrite, test_pairs, test_parcopy, test_pointer_dependency, test_promote, test_raising_addresses, test_raising_calls, test_raising_copies, test_raising_longs, test_regions, test_rewrite, test_runtime, test_rust_c_e2e, test_scoreboard, test_select, test_simplify, test_symbolic_relocation, test_transform, test_twoaddr |
| 5 | `qbopt/rewrite.py` | 341 | `src/rewrite.rs` | todo | test_array_access, test_basic_semantics, test_contract_profile, test_cpu_driver, test_lir, test_regions, test_rewrite, test_simplify, test_transform |
| 5 | `qbopt/wholeseg.py` | 432 | `src/wholeseg.rs` | todo | test_algebraic, test_allocation, test_arithmetic_target, test_array_access, test_array_bounds, test_array_facts, test_basic_semantics, test_blocks, test_c_discovery, test_c_emission, test_cfg_empty, test_cfg_merge, test_coalesce, test_constant_arguments, test_constant_call_memory, test_constant_conditions, test_constant_stores, test_e2e, test_edge_ranges, test_emission_order, test_entry_contract, test_extent, test_external_cells, test_float_identity, test_float_loop_exit, test_floatbounds, test_floatfold, test_flow, test_folded_relocations, test_frontend_parity, test_gvn_join, test_huge_array_access, test_in_place, test_induction_identity, test_indvars, test_invariant_argument, test_ivshare, test_layout, test_literal_initializers, test_load_pre, test_loopexit, test_loopmotion, test_lower_arguments, test_memory_joins, test_native_checkpoints, test_native_float_cse, test_native_float_licm, test_numeric_argument_escape, test_observers, test_omfwrite, test_omfwrite_corpus, test_opaque_emission, test_parcopy, test_peephole, test_promote, test_raising_dispatch, test_raising_frame, test_raising_longs, test_reciprocal, test_rewrite, test_rotate, test_rule5, test_runtime, test_scoreboard, test_selectors, test_spiller, test_store_combine, test_strength_offsets, test_symbol_licm, test_transform, test_trig_contract, test_twoaddr, test_unswitch, test_wholeseg, test_widen_ownership |
