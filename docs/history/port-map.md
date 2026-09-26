# Port map

One row per Python module. A module is `ported` only when `tools/port_diff.py`
shows identical stage dumps and its Python tests are ported. `draft` is Rust
that cites Python but has not passed the dump diff. Phase numbers follow the
plan: 2 C unoptimized, 3 optimizer, 4 QB, 5 BC rewrite.

`BC dump` means `port_diff --bc` matched every stage, MIR, LIR and the
emitted code, of every `tests/fixtures/omf` object. `BC harness` means the Python
tests are ported and a dump of every pre-raise fact wholeseg computes matched
on every object under `tests/fixtures/`. Python tests that monkeypatch a stage out
of `mir.bodies` are not ported; each Rust test file's header names them.

`matrix` means `tools/matrix.py --rewriter rust` gave every program in all
12 configurations the verdict, rewritten bytes and output that `--rewriter
python` did.

`written_bc harness` means Python's own `omfwrite.written_bc` inputs (LIR
bodies, records, source map) were replayed through the Rust `written_bc` for
every object under `tests/fixtures/omf` and `tests/fixtures/regressions`, with identical
bytes or refusal.

`llrm-objdump` and `llrm-opt` were built on the deleted `src/old` pipeline.
They are still to be ported, from `tools/dump.py` and `tools/stages.py`.

| Phase | Python | Lines | Rust | Status | Python tests |
|---|---|---:|---|---|---|
| - | `qbopt/legacy/calls.py` | 839 | `crates/llrm-core/src/legacy/calls.rs` | ported (tests, BC dump) | test_calls, test_runtime, test_select, test_transform |
| - | `qbopt/legacy/lift.py` | 1061 | `crates/llrm-core/src/legacy/lift.rs` | ported (rewrite subset, BC dump); `lift`, `tail`, `regions`, `emit_region` n/a (tests only) | test_calls, test_lift, test_module |
| - | `qbopt/legacy/regalloc.py` | 588 | `crates/llrm-core/src/legacy/regalloc.rs` | n/a (legacy) | test_flow, test_layout |
| - | `qbopt/legacy/simplify.py` | 252 | `crates/llrm-core/src/legacy/simplify.rs` | n/a (legacy) | test_simplify |
| 2 | `qbopt/analysis/alias.py` | 773 | `crates/llrm-core/src/analysis/alias.rs` | ported (C path) | test_inline, test_mir_alias |
| 2 | `qbopt/analysis/intervals.py` | 232 | `crates/llrm-core/src/analysis/intervals.rs` | ported (C path) | test_allocation, test_coalesce, test_splitkit |
| 2 | `qbopt/analysis/liveness.py` | 159 | `crates/llrm-core/src/analysis/liveness.rs` | ported (C path) | test_regalloc, test_word_arithmetic_carry |
| 2 | `qbopt/backend/addressforms.py` | 585 | `crates/llrm-core/src/backend/addressforms.rs` | ported (C path) | - |
| 2 | `qbopt/backend/addressvalues.py` | 54 | `crates/llrm-core/src/backend/addressvalues.rs` | ported (C path) | - |
| 2 | `qbopt/backend/allocate.py` | 1650 | `crates/llrm-core/src/backend/allocate.rs` | ported (C path) | test_allocation, test_constrain, test_cpu_profile, test_dead_call_deliveries, test_interval_redefinitions, test_lir, test_lower_arguments, test_spiller, test_wholeseg |
| 2 | `qbopt/backend/arithmetic.py` | 69 | `crates/llrm-core/src/backend/arithmetic.rs` | ported (C path) | test_arithmetic_target |
| 2 | `qbopt/backend/asm.py` | 1079 | `crates/llrm-core/src/backend/asm.rs` | ported (tests, written_bc harness) | test_loop_exit_layout, test_raising_calls, test_select, test_symbolic_relocation |
| 2 | `qbopt/backend/coalesce.py` | 373 | `crates/llrm-core/src/backend/coalesce.rs` | ported (C path) | test_coalesce, test_flow |
| 2 | `qbopt/backend/comparefold.py` | 109 | `crates/llrm-core/src/backend/comparefold.rs` | ported (C path) | test_memory_folding |
| 2 | `qbopt/backend/constrain.py` | 448 | `crates/llrm-core/src/backend/constrain.rs` | ported (C path) | test_constrain |
| 2 | `qbopt/backend/copyprop.py` | 248 | `crates/llrm-core/src/backend/copyprop.rs` | ported (C path) | - |
| 2 | `qbopt/backend/copysink.py` | 182 | `crates/llrm-core/src/backend/copysink.rs` | ported (C path) | test_copysink |
| 2 | `qbopt/backend/cpu.py` | 221 | `crates/llrm-core/src/backend/cpu.rs` | ported (C path) | test_cpu_profile, test_floatalloc, test_inline, test_quality, test_schedule |
| 2 | `qbopt/backend/division.py` | 101 | `crates/llrm-core/src/backend/division.rs` | ported (C path) | test_reciprocal, test_timing_bounds |
| 2 | `qbopt/backend/farcall.py` | 56 | `crates/llrm-core/src/backend/farcall.rs` | ported (C path) | - |
| 2 | `qbopt/backend/farload.py` | 141 | `crates/llrm-core/src/backend/farload.rs` | ported (C path) | - |
| 2 | `qbopt/backend/floatalloc.py` | 1086 | `crates/llrm-core/src/backend/floatalloc.rs` | ported (C path) | test_cpu_profile, test_float_values, test_floatalloc, test_raising_copies |
| 2 | `qbopt/backend/floatregions.py` | 195 | `crates/llrm-core/src/backend/floatregions.rs` | ported (C path) | - |
| 2 | `qbopt/backend/fpu.py` | 113 | `crates/llrm-core/src/backend/fpu.rs` | ported (C path) | test_fpu |
| 2 | `qbopt/backend/frame.py` | 107 | `crates/llrm-core/src/backend/frame.rs` | ported (C path) | test_constrain, test_floatalloc, test_flow, test_frame, test_jumps, test_lower_arguments, test_native_frame, test_native_stack_arguments, test_peephole, test_prologue, test_rematerialized_definitions, test_spiller |
| 2 | `qbopt/backend/jumps.py` | 551 | `crates/llrm-core/src/backend/jumps.rs` | ported (C path) | - |
| 2 | `qbopt/backend/layout.py` | 619 | `crates/llrm-core/src/backend/layout.rs` | ported (tests, written_bc harness) | test_emission_order, test_layout, test_wholeseg |
| 2 | `qbopt/backend/liveness.py` | 142 | `crates/llrm-core/src/backend/liveness.rs` | ported (C path) | test_liveness |
| 2 | `qbopt/backend/lower.py` | 2117 | `crates/llrm-core/src/backend/lower.rs` | ported (C path, BC dump) | test_addressforms, test_basic_semantics, test_c_segment_addresses, test_call_clobbers, test_constrain, test_consts, test_dead_ownership, test_far_load_identity, test_file_contracts, test_float_values, test_floatalloc, test_floatfold, test_flow, test_frame_addresses, test_hir, test_lir, test_lir_emission_order, test_lower_conditions, test_lower_switches, test_masm, test_native_returns, test_opaque_relocations, test_pointer_offset, test_raising_addresses, test_raising_calls, test_spiller, test_stack_segment, test_symbolic_relocation, test_transform, test_unroll |
| 2 | `qbopt/backend/lower_floats.py` | 35 | `crates/llrm-core/src/backend/lower_floats.rs` | ported (C path) | test_float_values, test_sccp, test_unroll |
| 2 | `qbopt/backend/lower_int64.py` | 560 | `crates/llrm-core/src/backend/lower_int64.rs` | ported (C path) | test_cfront, test_modern_frontend |
| 2 | `qbopt/backend/lower_switches.py` | 129 | `crates/llrm-core/src/backend/lower_switches.rs` | ported (C path) | - |
| 2 | `qbopt/backend/machinecse.py` | 245 | `crates/llrm-core/src/backend/machinecse.rs` | ported (C path) | test_machinecse |
| 2 | `qbopt/backend/machinedce.py` | 135 | `crates/llrm-core/src/backend/machinedce.rs` | ported (C path) | test_machinedce |
| 2 | `qbopt/backend/masm.py` | 423 | `crates/llrm-core/src/backend/masm.rs` | ported (C path) | test_cfront_frontend, test_hir, test_hir_execute, test_memory_folding, test_modern_frontend, test_peephole |
| 2 | `qbopt/backend/nativeframe.py` | 325 | `crates/llrm-core/src/backend/nativeframe.rs` | ported (C path) | test_native_frame, test_native_stack_arguments |
| 2 | `qbopt/backend/omfwrite.py` | 675 | `crates/llrm-core/src/backend/omfwrite.rs` | ported (C path, written_bc harness) | test_cbench_e2e, test_cfront_frontend, test_cfront_int64_e2e, test_frontend_parity, test_spiller, test_wholeseg |
| 2 | `qbopt/backend/parcopy.py` | 258 | `crates/llrm-core/src/backend/parcopy.rs` | ported (C path) | test_lir_verify, test_parcopy, test_wholeseg |
| 2 | `qbopt/backend/peephole.py` | 2683 | `crates/llrm-core/src/backend/peephole.rs` | ported (C path) | test_addressforms, test_dead_address_arithmetic, test_lir_emission_order, test_liveness, test_machine_copyprop, test_prologue |
| 2 | `qbopt/backend/phielim.py` | 508 | `crates/llrm-core/src/backend/phielim.rs` | ported (C path) | test_floatalloc, test_flow, test_lir, test_spiller |
| 2 | `qbopt/backend/pointers.py` | 55 | `crates/llrm-core/src/backend/pointers.rs` | ported (C path) | test_pointer_offset |
| 2 | `qbopt/backend/prologue.py` | 198 | `crates/llrm-core/src/backend/prologue.rs` | ported (C path) | test_flow, test_parcopy |
| 2 | `qbopt/backend/reencode.py` | 225 | `crates/llrm-core/src/backend/reencode.rs` | n/a (unused) | test_reencode |
| 2 | `qbopt/backend/regthrash.py` | 255 | `crates/llrm-core/src/backend/regthrash.rs` | ported (C path) | - |
| 2 | `qbopt/backend/rmw.py` | 332 | `crates/llrm-core/src/backend/rmw.rs` | ported (C path) | test_rmw |
| 2 | `qbopt/backend/schedule.py` | 319 | `crates/llrm-core/src/backend/schedule.rs` | ported (C path) | - |
| 2 | `qbopt/backend/select.py` | 1784 | `crates/llrm-core/src/backend/select.rs` | ported (C path) | test_address_roles, test_allocation, test_allocation_hints, test_arithmetic_immediates, test_coalesce, test_constrain, test_e2e, test_float_register_arithmetic, test_flow, test_fpu, test_layout, test_lower_conditions, test_lower_switches, test_parcopy, test_peephole, test_postallocation, test_regthrash, test_select |
| 2 | `qbopt/backend/spiller.py` | 1807 | `crates/llrm-core/src/backend/spiller.rs` | ported (C path) | test_constrain, test_flow, test_rematerialized_definitions, test_spiller, test_wholeseg |
| 2 | `qbopt/backend/spillforward.py` | 176 | `crates/llrm-core/src/backend/spillforward.rs` | ported (C path) | test_peephole |
| 2 | `qbopt/backend/splitkit.py` | 485 | `crates/llrm-core/src/backend/splitkit.rs` | ported (C path) | test_splitkit |
| 2 | `qbopt/backend/storecombine.py` | 52 | `crates/llrm-core/src/backend/storecombine.rs` | ported (C path) | - |
| 2 | `qbopt/backend/target.py` | 341 | `crates/llrm-core/src/backend/target.rs` | ported (C path) | test_coalesce, test_constrain, test_flow, test_lir, test_regalloc, test_select, test_selectors, test_strength_offsets |
| 2 | `qbopt/backend/timing.py` | 42 | `crates/llrm-core/src/backend/timing.rs` | ported (C path) | test_timing_bounds |
| 2 | `qbopt/backend/twoaddr.py` | 244 | `crates/llrm-core/src/backend/twoaddr.rs` | ported (C path) | test_lir |
| 2 | `qbopt/backend/verify.py` | 200 | `crates/llrm-core/src/backend/verify.rs` | ported (C path) | test_addressforms, test_flow, test_memory_folding, test_peephole, test_phi_widths, test_regthrash |
| 2 | `qbopt/cfront/compile.py` | 779 | `crates/llrm-c/src/compile.rs` | ported (C path) | test_cfront, test_cfront_frontend, test_cpu_profile, test_float_values, test_frontend_parity, test_indvars, test_inline, test_jumps, test_omfwrite, test_quality, test_spiller, test_stack_segment, test_unroll |
| 2 | `qbopt/cfront/hir.py` | 268 | `crates/llrm-c/src/hir.rs` | ported | test_cfront_frontend |
| 2 | `qbopt/cfront/libfunc.py` | 36 | `crates/llrm-c/src/libfunc.rs` | ported (C path) | - |
| 2 | `qbopt/cfront/raise_hir.py` | 1630 | `crates/llrm-c/src/raise_hir.rs` | ported (C path) | test_algebraic, test_cfront_frontend |
| 2 | `qbopt/cfront/stream.py` | 65 | `crates/llrm-c/src/stream.rs` | ported | test_cfront_frontend |
| 2 | `qbopt/cycles/cycles.py` | 478 | `crates/llrm-core/src/cycles/cycles.rs` | ported | test_cpu_profile |
| 2 | `qbopt/cycles/timings.py` | 239 | `crates/llrm-core/src/cycles/timings.rs` | ported (C path) | - |
| 2 | `qbopt/flow.py` | 198 | `crates/llrm-core/src/flow.rs` | ported (C path, BC dump) | test_allocation, test_coalesce, test_cpu_profile, test_flow, test_jumps, test_lir_verify, test_native_frame, test_parcopy, test_raising_copies, test_spiller, test_wholeseg |
| 2 | `qbopt/model/floating.py` | 48 | `crates/llrm-core/src/model/floating.rs` | ported (C path) | test_cfront, test_cse_commutative, test_float_cse_paths, test_float_values, test_floatbounds, test_floatfacts, test_floatfold, test_loopclone, test_native_float_licm, test_sccp, test_unroll |
| 2 | `qbopt/model/ir.py` | 1450 | `crates/llrm-core/src/model/ir/` | ported (C path, BC dump) | test_address_roles, test_addressforms, test_algebraic, test_allocation, test_allocation_hints, test_array_bounds, test_avail, test_availability_call_effects, test_availability_operands, test_c_segment_addresses, test_call_clobbers, test_cfg_empty, test_cfg_merge, test_coalesce, test_constant_call_memory, test_constant_carry, test_constant_cells, test_constant_division, test_constrain, test_consts, test_copy_values, test_copysink, test_cse_commutative, test_cse_dominance, test_dead_address_arithmetic, test_dead_call_deliveries, test_dead_ownership, test_e2e, test_emission_order, test_extract, test_far_load_identity, test_farload, test_float_constants, test_float_cse_paths, test_float_loop_exit, test_float_register_arithmetic, test_float_values, test_floatalloc, test_floatfold, test_floating, test_flow, test_frame, test_frame_addresses, test_frame_escape, test_frontend_parity, test_gvn_join, test_high_product, test_induction_identity, test_indvars, test_inline, test_invariant_shift, test_invariant_values, test_ir, test_layout, test_lcssa, test_lir, test_lir_emission_order, test_literal_initializers, test_liveness, test_load_pre, test_loop_exit_layout, test_lower_arguments, test_lower_conditions, test_lower_sign, test_lower_switches, test_machine_copyprop, test_machinecse, test_masm, test_memory_cse, test_memory_folding, test_memory_joins, test_memory_opportunities, test_mir, test_mir_alias, test_multiply_select, test_native_frame, test_native_returns, test_native_stack_arguments, test_omfwrite, test_pairs, test_parcopy, test_peephole, test_phi_widths, test_pointer_memory, test_postallocation, test_private_frame, test_prologue, test_promote, test_quality, test_raising_arrays, test_raising_frame, test_ranges, test_regalloc, test_regthrash, test_rematerialized_definitions, test_rewind, test_rmw, test_runtime_cells, test_scale_selection, test_scaled_addressing, test_sccp, test_schedule, test_select, test_select_float_stack, test_simplify, test_spiller, test_splitkit, test_ssa_phis, test_stack_segment, test_stages, test_store_combine, test_test_immediate, test_transform, test_unroll, test_unroll_budget, test_unswitch, test_wholeseg, test_wide |
| 2 | `qbopt/model/lir.py` | 360 | `crates/llrm-core/src/model/lir.rs` | ported (C path) | test_allocation, test_countdown, test_cpu_profile, test_emission_order, test_floatalloc, test_flow, test_hir, test_jumps, test_lir_verify, test_machinedce, test_omfwrite, test_pointer_memory, test_quality, test_select, test_spiller, test_store_combine, test_wholeseg |
| 2 | `qbopt/model/memory.py` | 142 | `crates/llrm-core/src/model/memory.rs` | ported (C path) | test_loopclone, test_mir_alias, test_private_frame, test_sccp |
| 2 | `qbopt/model/mir.py` | 3383 | `crates/llrm-core/src/model/mir.rs` | ported (C path, BC dump) | test_addressforms, test_allocation, test_avail, test_cfg_empty, test_cfg_merge, test_constant_cells, test_constrain, test_countdown, test_cse_commutative, test_cse_dominance, test_emission_order, test_extract, test_farload, test_file_contracts, test_float_constants, test_float_loop_exit, test_floatalloc, test_floating, test_flow, test_frontend_parity, test_gvn_join, test_high_product, test_hoist_selector, test_induction_inequality, test_indvars, test_invariant_argument, test_invariant_shift, test_laststore, test_layout, test_lcssa_merges, test_licm_operand_dependencies, test_lir, test_lir_emission_order, test_literal_call_contracts, test_literal_initializers, test_liveness, test_load_pre, test_loopexit, test_loopmotion, test_loopsimplify, test_lower_sign, test_masm, test_memory_folding, test_memory_joins, test_memory_opportunities, test_mir, test_native_float_licm, test_native_frame, test_native_status_flags, test_noreturn, test_observers, test_opaque_memory_effects, test_opaque_relocations, test_pairs, test_peephole, test_phi_widths, test_pointer_memory, test_postallocation, test_raising_addresses, test_raising_call_memory, test_raising_dispatch, test_raising_longs, test_raising_unary, test_ranges, test_regions, test_rounding_contracts, test_rule5, test_scalar_division, test_scale_selection, test_scoreboard, test_select, test_spiller, test_switch_loopclone, test_symbol_licm, test_transform, test_unary_promotion, test_unroll, test_unsigned_edge_ranges, test_word_arithmetic_carry |
| 2 | `qbopt/model/passes.py` | 226 | `crates/llrm-core/src/model/passes.rs` | ported (C path) | test_cpu_profile, test_flow, test_lir_verify, test_mir_alias, test_promote, test_rewind, test_rule5, test_transform, test_unroll, test_unroll_budget, test_unswitch |
| 2 | `qbopt/backend/narrow.py` | 108 | `crates/llrm-core/src/backend/narrow.rs` | ported (C path) | - |
| 3 | `qbopt/analysis/avail.py` | 555 | `crates/llrm-core/src/analysis/avail.rs` | ported (C path) | test_avail, test_availability_call_effects, test_availability_operands, test_far_memory_identity, test_float_values, test_memoryssa_forward, test_observers, test_qgldiff_forwarding |
| 3 | `qbopt/analysis/constant_cycles.py` | 123 | `crates/llrm-core/src/analysis/constant_cycles.rs` | ported (C path) | - |
| 3 | `qbopt/analysis/consts.py` | 769 | `crates/llrm-core/src/analysis/consts.rs` | ported (C path) | test_array_access, test_array_bounds, test_constant_arguments, test_constant_call_memory, test_constant_carry, test_constant_conditions, test_constant_cycles, test_constant_division, test_constant_index, test_constant_stores, test_float_recurrences, test_floatfacts, test_high_product, test_induction_identity, test_induction_inequality, test_indvars, test_loopexit, test_pointer_constants, test_promote, test_raising_arrays, test_raising_copies, test_raising_dispatch, test_raising_longs, test_rewind, test_transform |
| 3 | `qbopt/analysis/effects.py` | 30 | `crates/llrm-core/src/analysis/effects.rs` | ported (C path) | test_far_load_identity, test_opaque_memory_effects |
| 3 | `qbopt/analysis/flags.py` | 111 | `crates/llrm-core/src/analysis/flags.rs` | ported (C path, BC dump) | test_calls, test_ir, test_lift |
| 3 | `qbopt/analysis/floatbounds.py` | 153 | `crates/llrm-core/src/analysis/floatbounds.rs` | ported (C path) | test_floatbounds |
| 3 | `qbopt/analysis/floatfacts.py` | 356 | `crates/llrm-core/src/analysis/floatfacts.rs` | ported (C path) | test_float_loop_exit, test_float_recurrences, test_float_values, test_floatbounds, test_floatfacts, test_floatfold, test_literal_initializers |
| 3 | `qbopt/analysis/frameescape.py` | 170 | `crates/llrm-core/src/analysis/frameescape.rs` | ported (C path) | test_frame_escape |
| 3 | `qbopt/analysis/induction.py` | 1322 | `crates/llrm-core/src/analysis/induction.rs` | ported (C path) | test_induction_identity, test_induction_inequality, test_indvars, test_loopexit, test_mir, test_modern_frontend, test_quotient_recurrence, test_rewind, test_unroll |
| 3 | `qbopt/analysis/interprocedural.py` | 487 | `crates/llrm-core/src/analysis/interprocedural.rs` | ported (C path) | test_sccp |
| 3 | `qbopt/analysis/loops.py` | 316 | `crates/llrm-core/src/analysis/loops.rs` | ported (C path) | test_algebraic, test_countdown, test_float_values, test_hir, test_induction_identity, test_induction_inequality, test_indvars, test_invariant_argument, test_jumps, test_laststore, test_loopclone, test_loopexit, test_loopmotion, test_loops, test_promote, test_quotient_recurrence, test_rewind, test_scoreboard, test_unswitch |
| 3 | `qbopt/analysis/memoryssa.py` | 204 | `crates/llrm-core/src/analysis/memoryssa.rs` | ported (C path) | test_memoryssa |
| 3 | `qbopt/analysis/noreturn.py` | 87 | `crates/llrm-core/src/analysis/noreturn.rs` | ported (C path) | test_noreturn, test_sccp |
| 3 | `qbopt/analysis/observers.py` | 352 | `crates/llrm-core/src/analysis/observers.rs` | ported (C path) | test_private_frame |
| 3 | `qbopt/analysis/pointerfacts.py` | 65 | `crates/llrm-core/src/analysis/pointerfacts.rs` | ported (C path) | test_pointer_constants |
| 3 | `qbopt/analysis/ranges.py` | 369 | `crates/llrm-core/src/analysis/ranges.rs` | ported (C path) | test_array_facts, test_cfg_merge, test_edge_ranges, test_floatbounds, test_mir_alias, test_promote, test_regions |
| 3 | `qbopt/analysis/regions.py` | 391 | `crates/llrm-core/src/analysis/regions.rs` | ported (C path) | test_external_cells |
| 3 | `qbopt/analysis/ssa.py` | 344 | `crates/llrm-core/src/analysis/ssa.rs` | ported (C path) | test_algebraic, test_float_values, test_induction_identity, test_mir, test_pointer_memory, test_rule5, test_ssa_phis, test_ssa_unreachable, test_transform |
| 3 | `qbopt/optimize/algebraic.py` | 1027 | `crates/llrm-core/src/optimize/algebraic.rs` | ported (C path) | test_algebraic, test_invariant_values |
| 3 | `qbopt/optimize/cfg.py` | 77 | `crates/llrm-core/src/optimize/cfg.rs` | ported (C path) | - |
| 3 | `qbopt/optimize/edges.py` | 40 | `crates/llrm-core/src/optimize/edges.rs` | ported (C path) | - |
| 3 | `qbopt/optimize/exitsink.py` | 107 | `crates/llrm-core/src/optimize/exitsink.rs` | ported (C path) | - |
| 3 | `qbopt/optimize/fill.py` | 367 | `crates/llrm-core/src/optimize/fill.rs` | ported (C path) | test_modern_frontend |
| 3 | `qbopt/optimize/floatfold.py` | 234 | `crates/llrm-core/src/optimize/floatfold.rs` | ported (C path) | test_floatfold, test_unroll |
| 3 | `qbopt/optimize/floatloop.py` | 140 | `crates/llrm-core/src/optimize/floatloop.rs` | ported (C path) | test_float_loop_exit |
| 3 | `qbopt/optimize/gvn.py` | 206 | `crates/llrm-core/src/optimize/gvn.rs` | ported (C path) | test_mir_alias |
| 3 | `qbopt/optimize/indvars.py` | 1155 | `crates/llrm-core/src/optimize/indvars.rs` | ported (C path) | test_induction_identity, test_indvars, test_loopexit |
| 3 | `qbopt/optimize/inline.py` | 462 | `crates/llrm-core/src/optimize/inline.rs` | ported (C path) | - |
| 3 | `qbopt/optimize/ivshare.py` | 117 | `crates/llrm-core/src/optimize/ivshare.rs` | ported (C path) | test_ivshare |
| 3 | `qbopt/optimize/lcssa.py` | 134 | `crates/llrm-core/src/optimize/lcssa.rs` | ported (C path) | test_layout, test_lcssa, test_lcssa_merges |
| 3 | `qbopt/optimize/lcssamerges.py` | 119 | `crates/llrm-core/src/optimize/lcssamerges.rs` | ported (C path) | - |
| 3 | `qbopt/optimize/loadjoins.py` | 154 | `crates/llrm-core/src/optimize/loadjoins.rs` | ported (C path) | test_load_pre |
| 3 | `qbopt/optimize/loopclone.py` | 214 | `crates/llrm-core/src/optimize/loopclone.rs` | ported (C path) | test_layout, test_loopclone, test_switch_loopclone |
| 3 | `qbopt/optimize/loopexit.py` | 399 | `crates/llrm-core/src/optimize/loopexit.rs` | ported (C path) | test_induction_identity, test_loopexit |
| 3 | `qbopt/optimize/loopmotion.py` | 251 | `crates/llrm-core/src/optimize/loopmotion.rs` | ported (C path) | test_float_loop_exit, test_laststore |
| 3 | `qbopt/optimize/loopsimplify.py` | 122 | `crates/llrm-core/src/optimize/loopsimplify.rs` | ported (C path) | - |
| 3 | `qbopt/optimize/peel.py` | 122 | `crates/llrm-core/src/optimize/peel.rs` | ported (C path) | test_transform, test_unroll |
| 3 | `qbopt/optimize/pointeraccess.py` | 133 | `crates/llrm-core/src/optimize/pointeraccess.rs` | ported (C path) | - |
| 3 | `qbopt/optimize/profit.py` | 275 | `crates/llrm-core/src/optimize/profit.rs` | ported (C path) | test_unroll_budget |
| 3 | `qbopt/optimize/promote.py` | 1105 | `crates/llrm-core/src/optimize/promote.rs` | ported (C path) | test_flow, test_mir_alias, test_promote, test_transform, test_unary_promotion |
| 3 | `qbopt/optimize/rotate.py` | 406 | `crates/llrm-core/src/optimize/rotate.rs` | ported (C path) | test_countdown, test_induction_identity, test_rewind |
| 3 | `qbopt/optimize/strength.py` | 1410 | `crates/llrm-core/src/optimize/strength.rs` | ported (C path) | test_cpu_profile, test_flow, test_ivshare, test_ranges |
| 3 | `qbopt/optimize/transform.py` | 3353 | `crates/llrm-core/src/optimize/transform.rs` | ported (C path) | test_cfront, test_constant_arguments, test_constant_stores, test_consts, test_copy_values, test_cpu_driver, test_dead_ownership, test_far_memory_identity, test_float_cse_paths, test_float_loop_exit, test_float_recurrences, test_float_values, test_flow, test_hoist_selector, test_induction_inequality, test_invariant_argument, test_ivshare, test_laststore, test_licm_operand_dependencies, test_loopexit, test_loopmotion, test_loopsimplify, test_lower_switches, test_memory_cse, test_mir, test_native_float_licm, test_parcopy, test_phi_widths, test_pointer_memory, test_promote, test_qgldiff_forwarding, test_quotient_recurrence, test_raising_calls, test_raising_copies, test_raising_dispatch, test_ranges, test_rule5, test_scalar_division, test_sccp, test_transform, test_unroll |
| 3 | `qbopt/optimize/unroll.py` | 486 | `crates/llrm-core/src/optimize/unroll.rs` | ported (C path) | test_transform, test_unroll |
| 3 | `qbopt/optimize/unswitch.py` | 218 | `crates/llrm-core/src/optimize/unswitch.rs` | ported (C path) | test_indvars, test_unswitch |
| 3 | `qbopt/optimize/wholephis.py` | 100 | `crates/llrm-core/src/optimize/wholephis.rs` | ported (C path) | test_algebraic |
| 3 | `qbopt/optimize/wholestores.py` | 37 | `crates/llrm-core/src/optimize/wholestores.rs` | ported (C path) | - |
| 3 | `qbopt/analysis/peelsize.py` | 102 | `crates/llrm-core/src/analysis/peelsize.rs` | ported (C path) | - |
| 3 | `qbopt/optimize/canonical.py` | 80 | `crates/llrm-core/src/optimize/canonical.rs` | ported (C path) | - |
| 4 | `qbopt/frontend/modern/compile.py` | 187 | `crates/llrm-nib/src/compile.rs` (+ `tools/modernstages.py` as `modernstages.rs`) | ported | test_farload, test_hir_execute, test_modern_frontend, test_modernstages |
| 4 | `qbopt/frontend/modern/driver.py` | 53 | `crates/llrm-nib/src/driver.rs` | ported | test_modern_frontend |
| 4 | `qbopt/frontend/qb/__init__.py` | 23 | `crates/llrm-qb/src/lib.rs` | ported | test_hir, test_modern_frontend, test_qb_frontend_command, test_qbstages |
| 4 | `qbopt/frontend/qb/__main__.py` | 64 | `crates/llrm-qb/src/cli.rs` (`llrm-qb`) | ported | - |
| 4 | `qbopt/frontend/qb/abi.py` | 1339 | `crates/llrm-core/src/abi/qb.rs` | ported | test_hir |
| 4 | `qbopt/frontend/qb/compile.py` | 1645 | `crates/llrm-qb/src/compile.rs` | ported | - |
| 4 | `qbopt/frontend/qb/driver.py` | 204 | `crates/llrm-qb/src/driver.rs` | ported | test_hir, test_qb_frontend_command |
| 4 | `qbopt/frontend/qb/inline_x87.py` | 64 | `crates/llrm-qb/src/inline_x87.rs` | ported | - |
| 4 | `qbopt/frontend/qb/stage_text.py` | 178 | `crates/llrm-qb/src/stage_text.rs` (+ `tools/qbstages.py` as `qbstages.rs`) | ported | - |
| 4 | `qbopt/hir/__init__.py` | 93 | `crates/llrm-core/src/hir/mod.rs` | ported (test_hir HIR-only cases) | test_hir, test_hir_execute, test_modern_e2e, test_modern_frontend, test_qbstages |
| 4 | `qbopt/hir/__main__.py` | 29 | `crates/llrm-core/src/hir/__main__.rs` | n/a (no tool runs it; docs/architecture/hir/readme.md only) | - |
| 4 | `qbopt/hir/callmemory.py` | 71 | `crates/llrm-core/src/hir/callmemory.rs` | ported | - |
| 4 | `qbopt/hir/codec.py` | 129 | `crates/llrm-core/src/hir/codec.rs` | ported | - |
| 4 | `qbopt/hir/dump.py` | 141 | `crates/llrm-core/src/hir/dump.rs` | ported | - |
| 4 | `qbopt/hir/escape.py` | 28 | `crates/llrm-core/src/hir/escape.rs` | ported | - |
| 4 | `qbopt/hir/execute.py` | 438 | `crates/llrm-core/src/hir/execute.rs` | n/a (tools only) | test_modern_e2e |
| 4 | `qbopt/hir/lower.py` | 1438 | `crates/llrm-core/src/hir/lower.rs` | ported | - |
| 4 | `qbopt/hir/model.py` | 370 | `crates/llrm-core/src/hir/model.rs` | ported | test_hir |
| 4 | `qbopt/hir/verify.py` | 447 | `crates/llrm-core/src/hir/verify.rs` | ported | - |
| 5 | `qbopt/abi/callsite.py` | 52 | `crates/llrm-core/src/abi/callsite.rs` | ported (tests, BC harness) | - |
| 5 | `qbopt/abi/events.py` | 52 | `crates/llrm-core/src/abi/events.rs` | ported (tests, BC harness) | test_runtime |
| 5 | `qbopt/abi/handlers.py` | 47 | `crates/llrm-core/src/abi/handlers.rs` | ported (tests, BC harness) | test_extent |
| 5 | `qbopt/abi/inputscan.py` | 864 | `crates/llrm-core/src/abi/inputscan.rs` | ported (tests, BC harness) | - |
| 5 | `qbopt/abi/linkunit.py` | 179 | `crates/llrm-core/src/abi/linkunit.rs` | ported (tests, BC harness) | test_array_access, test_basic_semantics, test_contract_profile, test_cpu_driver |
| 5 | `qbopt/abi/nativecalls.py` | 122 | `crates/llrm-core/src/abi/nativecalls.rs` | ported (tests, BC harness) | test_native_frame |
| 5 | `qbopt/abi/ports.py` | 15 | `crates/llrm-core/src/abi/ports.rs` | ported (tests, BC harness) | - |
| 5 | `qbopt/abi/profile.py` | 113 | `crates/llrm-core/src/abi/profile.rs` | ported (tests, BC harness) | - |
| 5 | `qbopt/abi/runtime.py` | 1436 | `crates/llrm-core/src/abi/runtime.rs` | ported (tests, BC harness) | test_allocation, test_callsite_abi, test_cfg_merge, test_coalesce, test_constant_call_memory, test_environ_contract, test_float_values, test_floatbounds, test_flow, test_huge_array_access, test_invariant_shift, test_ivshare, test_lir, test_literal_initializers, test_loopmotion, test_mir, test_noreturn, test_numeric_argument_escape, test_parcopy, test_raising_calls, test_redim_contract, test_regions, test_rounding_contracts, test_rule5, test_runtime, test_runtime_cells, test_transform |
| 5 | `qbopt/frontend/addressfacts.py` | 36 | `crates/llrm-core/src/frontends/bc/addressfacts.rs` | ported (tests, BC dump) | test_array_facts |
| 5 | `qbopt/frontend/arrayfacts.py` | 403 | `crates/llrm-core/src/frontends/bc/arrayfacts.rs` | ported (tests, BC dump) | test_array_facts, test_cfg_merge |
| 5 | `qbopt/frontend/blocks.py` | 671 | `crates/llrm-core/src/frontends/bc/blocks.rs` | ported (tests, BC harness) | test_algebraic, test_allocation, test_array_facts, test_avail, test_basic_semantics, test_blocks, test_c_discovery, test_c_segment_addresses, test_cfg_merge, test_coalesce, test_dispatch_edges, test_e2e, test_extent, test_float_constants, test_float_identity, test_float_values, test_flow, test_fpu, test_induction_identity, test_ir, test_layout, test_licm_operand_dependencies, test_lir, test_loopexit, test_loops, test_mir, test_native_status_flags, test_observers, test_pairs, test_parcopy, test_peephole, test_raising_addresses, test_regions, test_runtime, test_scoreboard, test_select, test_simplify, test_spiller, test_stack, test_stages, test_transform, test_twoaddr, test_unswitch, test_wholeseg |
| 5 | `qbopt/frontend/declen.py` | 265 | `crates/llrm-core/src/frontends/bc/declen.rs` | ported (tests, BC harness) | test_addressforms, test_blocks, test_c_segment_addresses, test_calls, test_callsite_abi, test_declen, test_e2e, test_float_register_arithmetic, test_fpu, test_ir, test_layout, test_lift, test_machine_copyprop, test_peephole, test_raising_copies, test_raising_frame, test_reencode, test_runtime, test_select, test_stack, test_stack_segment, test_test_immediate |
| 5 | `qbopt/frontend/extent.py` | 244 | `crates/llrm-core/src/frontends/bc/extent.rs` | ported (tests, BC harness) | test_extent, test_extent_cv, test_ir, test_native_frame, test_native_stack_arguments |
| 5 | `qbopt/frontend/fppatches.py` | 70 | `crates/llrm-core/src/frontends/bc/fppatches.rs` | ported (tests, BC harness) | test_c_discovery, test_far_load_identity, test_float_constants, test_float_register_arithmetic, test_licm_operand_dependencies, test_opaque_memory_effects, test_unary_promotion |
| 5 | `qbopt/frontend/fpstack.py` | 208 | `crates/llrm-core/src/frontends/bc/fpstack.rs` | ported (tests, BC dump) | test_fpstack |
| 5 | `qbopt/frontend/pairs.py` | 410 | `crates/llrm-core/src/frontends/bc/pairs.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_address_state.py` | 137 | `crates/llrm-core/src/frontends/bc/raising_address_state.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_addresses.py` | 104 | `crates/llrm-core/src/frontends/bc/raising_addresses.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_array_access.py` | 397 | `crates/llrm-core/src/frontends/bc/raising_array_access.rs` | ported (tests, BC dump) | test_array_access, test_huge_array_access |
| 5 | `qbopt/frontend/raising_array_bounds.py` | 285 | `crates/llrm-core/src/frontends/bc/raising_array_bounds.rs` | ported (tests, BC dump) | test_array_bounds, test_array_facts |
| 5 | `qbopt/frontend/raising_arrays.py` | 161 | `crates/llrm-core/src/frontends/bc/raising_arrays.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_bytes.py` | 54 | `crates/llrm-core/src/frontends/bc/raising_bytes.rs` | ported (tests, BC dump) | test_raising_bytes |
| 5 | `qbopt/frontend/raising_call_memory.py` | 558 | `crates/llrm-core/src/frontends/bc/raising_call_memory.rs` | ported (tests, BC dump) | test_raising_call_memory |
| 5 | `qbopt/frontend/raising_calls.py` | 255 | `crates/llrm-core/src/frontends/bc/raising_calls.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_carried.py` | 73 | `crates/llrm-core/src/frontends/bc/raising_carried.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_conditions.py` | 45 | `crates/llrm-core/src/frontends/bc/raising_conditions.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_control.py` | 21 | `crates/llrm-core/src/frontends/bc/raising_control.rs` | ported (tests, BC harness) | - |
| 5 | `qbopt/frontend/raising_copies.py` | 185 | `crates/llrm-core/src/frontends/bc/raising_copies.rs` | ported (tests, BC dump) | test_raising_copies |
| 5 | `qbopt/frontend/raising_defseg.py` | 234 | `crates/llrm-core/src/frontends/bc/raising_defseg.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_dispatch.py` | 123 | `crates/llrm-core/src/frontends/bc/raising_dispatch.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_division.py` | 77 | `crates/llrm-core/src/frontends/bc/raising_division.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_fields.py` | 69 | `crates/llrm-core/src/frontends/bc/raising_fields.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_float_calls.py` | 244 | `crates/llrm-core/src/frontends/bc/raising_float_calls.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_float_results.py` | 56 | `crates/llrm-core/src/frontends/bc/raising_float_results.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_float_values.py` | 175 | `crates/llrm-core/src/frontends/bc/raising_float_values.rs` | ported (tests, BC dump) | test_fpstack |
| 5 | `qbopt/frontend/raising_floats.py` | 113 | `crates/llrm-core/src/frontends/bc/raising_floats.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_frame.py` | 172 | `crates/llrm-core/src/frontends/bc/raising_frame.rs` | ported (tests, BC dump) | test_raising_frame |
| 5 | `qbopt/frontend/raising_literals.py` | 199 | `crates/llrm-core/src/frontends/bc/raising_literals.rs` | ported (tests, BC dump) | test_literal_initializers, test_raising_copies |
| 5 | `qbopt/frontend/raising_longs.py` | 515 | `crates/llrm-core/src/frontends/bc/raising_longs.rs` | ported (tests, BC dump) | test_raising_longs, test_raising_unary |
| 5 | `qbopt/frontend/raising_numeric_policy.py` | 47 | `crates/llrm-core/src/frontends/bc/raising_numeric_policy.rs` | ported (tests, BC dump) | test_float_cse_paths |
| 5 | `qbopt/frontend/raising_returns.py` | 31 | `crates/llrm-core/src/frontends/bc/raising_returns.rs` | ported (tests, BC dump) | - |
| 5 | `qbopt/frontend/raising_words.py` | 122 | `crates/llrm-core/src/frontends/bc/raising_words.rs` | ported (tests, BC dump) | test_raising_words, test_word_arithmetic_carry |
| 5 | `qbopt/frontend/stack.py` | 131 | `crates/llrm-core/src/frontends/bc/stack.rs` | ported (tests, BC harness) | test_stack |
| 5 | `qbopt/frontend/wide.py` | 312 | `crates/llrm-core/src/frontends/bc/wide.rs` | n/a (tools only) | test_wide |
| 5 | `qbopt/objectfile/addends.py` | 48 | `crates/llrm-core/src/objectfile/addends.rs` | ported (tests, BC harness) | test_omf_addends |
| 5 | `qbopt/objectfile/cvinfo.py` | 777 | `crates/llrm-core/src/objectfile/cvinfo.rs` | ported (tests, BC harness) | test_cvinfo |
| 5 | `qbopt/objectfile/module.py` | 573 | `crates/llrm-core/src/objectfile/module.rs` | ported (tests, BC harness) | test_address_roles, test_addressforms, test_algebraic, test_allocation, test_arithmetic_immediates, test_array_access, test_array_bounds, test_array_facts, test_avail, test_availability_call_effects, test_availability_operands, test_blocks, test_cfg_merge, test_coalesce, test_constant_arguments, test_constant_call_memory, test_constant_cells, test_constant_index, test_constant_stores, test_constrain, test_consts, test_countdown, test_dead_ownership, test_dispatch_edges, test_e2e, test_edge_ranges, test_extent, test_extent_cv, test_extract, test_far_memory_identity, test_farload, test_float_constants, test_float_cse_paths, test_float_values, test_floatalloc, test_floatbounds, test_floatfacts, test_floatfold, test_flow, test_folded_relocations, test_gvn_join, test_hir, test_huge_array_access, test_in_place, test_induction_identity, test_indvars, test_inline, test_ir, test_layout, test_lift, test_lir, test_literal_initializers, test_load_pre, test_loopmotion, test_lower_arguments, test_lower_conditions, test_machinecse, test_masm, test_memory_folding, test_memory_joins, test_memory_opportunities, test_memoryssa, test_memoryssa_forward, test_mir, test_mir_alias, test_multiply_select, test_numeric_argument_escape, test_observers, test_omf_addends, test_omfwrite, test_pairs, test_parcopy, test_peephole, test_pointer_constants, test_pointer_memory, test_pointer_offset, test_postallocation, test_private_frame, test_promote, test_raising_addresses, test_raising_arrays, test_raising_call_memory, test_raising_calls, test_raising_frame, test_raising_longs, test_ranges, test_regions, test_rewrite, test_runtime, test_runtime_cells, test_scaled_addressing, test_sccp, test_schedule, test_scoreboard, test_select, test_simplify, test_spiller, test_splitkit, test_stack_segment, test_stages, test_store_combine, test_symbolic_relocation, test_test_immediate, test_transform, test_twoaddr, test_unroll, test_wholeseg, test_wide |
| 5 | `qbopt/objectfile/omf.py` | 904 | `crates/llrm-core/src/objectfile/omf.rs` | ported (tests, BC harness) | test_allocation, test_array_access, test_cfg_merge, test_coalesce, test_constant_arguments, test_constant_call_memory, test_constant_cells, test_contract_profile, test_contracts, test_cvinfo, test_edge_ranges, test_entry_contract, test_extent, test_extent_cv, test_float_values, test_floatfacts, test_flow, test_folded_relocations, test_gvn_join, test_hir, test_huge_array_access, test_layout, test_lir, test_load_pre, test_memory_joins, test_mir, test_module, test_omf, test_omf_addends, test_omfwrite, test_pairs, test_parcopy, test_pointer_dependency, test_promote, test_raising_addresses, test_raising_calls, test_raising_copies, test_raising_longs, test_regions, test_rewrite, test_runtime, test_rust_c_e2e, test_scoreboard, test_select, test_simplify, test_symbolic_relocation, test_transform, test_twoaddr |
| 5 | `qbopt/rewrite.py` | 341 | `crates/llrm-core/src/rewrite.rs` | ported (BC dump, matrix) | test_array_access, test_basic_semantics, test_contract_profile, test_cpu_driver, test_lir, test_regions, test_rewrite, test_simplify, test_transform |
| 5 | `qbopt/wholeseg.py` | 432 | `crates/llrm-core/src/wholeseg.rs` | ported (BC dump) | test_algebraic, test_allocation, test_arithmetic_target, test_array_access, test_array_bounds, test_array_facts, test_basic_semantics, test_blocks, test_c_discovery, test_c_emission, test_cfg_empty, test_cfg_merge, test_coalesce, test_constant_arguments, test_constant_call_memory, test_constant_conditions, test_constant_stores, test_e2e, test_edge_ranges, test_emission_order, test_entry_contract, test_extent, test_external_cells, test_float_identity, test_float_loop_exit, test_floatbounds, test_floatfold, test_flow, test_folded_relocations, test_frontend_parity, test_gvn_join, test_huge_array_access, test_in_place, test_induction_identity, test_indvars, test_invariant_argument, test_ivshare, test_layout, test_literal_initializers, test_load_pre, test_loopexit, test_loopmotion, test_lower_arguments, test_memory_joins, test_native_checkpoints, test_native_float_cse, test_native_float_licm, test_numeric_argument_escape, test_observers, test_omfwrite, test_omfwrite_corpus, test_opaque_emission, test_parcopy, test_peephole, test_promote, test_raising_dispatch, test_raising_frame, test_raising_longs, test_reciprocal, test_rewrite, test_rotate, test_rule5, test_runtime, test_scoreboard, test_selectors, test_spiller, test_store_combine, test_strength_offsets, test_symbol_licm, test_transform, test_trig_contract, test_twoaddr, test_unswitch, test_wholeseg, test_widen_ownership |
