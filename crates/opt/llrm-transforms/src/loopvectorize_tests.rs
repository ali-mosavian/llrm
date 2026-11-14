//! `LoopVectorize` on the arm64 cost model (`always`): the loops the pin
//! widens, with the pin's factors (`opt -passes=loop-vectorize` on aarch64: i16
//! copy of 1920 is VF 8, IC 2), and the ones it leaves alone.

use super::LoopVectorize;
use crate::testing::parsed;

fn run(source: &str) -> String {
    let mut module = parsed(source);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopVectorize { always: true, only_when_forced: false });
    manager
        .run_module(
            &mut module,
            std::rc::Rc::new(llrm_arm64::machine::Arm64Machine { platform: llrm_arm64::machine::Platform::Linux }),
        )
        .unwrap();
    crate::testing::printed(&module)
}

fn count(
    text: &str,
    what: &str,
) -> usize {
    text.matches(what).count()
}

const COPY: &str = include_str!("../tests/lv/copy.ll");

/// The scalar copy stayed scalar: 1920 iterations of one i16 load and store.
#[test]
fn a_copy_loop_is_widened_by_the_pins_factors() {
    let text = run(COPY);
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "store <8 x i16>"), 2, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 16"), "{text}");
    assert_eq!(count(&text, "load i16"), 0, "{text}");
    // The loop the pin marks as vectorized: one loop, no remainder.
    assert_eq!(count(&text, "llvm.loop.isvectorized"), 1, "{text}");
}

const ADD: &str = include_str!("../tests/lv/binop_add.ll");
const CHAIN: &str = include_str!("../tests/lv/binop_chain.ll");
const SPLAT: &str = include_str!("../tests/lv/binop_splat.ll");
const SHIFT: &str = include_str!("../tests/lv/binop_shift.ll");
const MUL64: &str = include_str!("../tests/lv/binop_mul64.ll");
const UDIV: &str = include_str!("../tests/lv/binop_udiv.ll");
const FP_AXPY: &str = include_str!("../tests/lv/fp_axpy.ll");
const FP_STRIDED: &str = include_str!("../tests/lv/fp_strided.ll");
const FP_CONVERT: &str = include_str!("../tests/lv/fp_convert.ll");
const FP_CONVERT_NARROW: &str = include_str!("../tests/lv/fp_convert_narrow.ll");
const FP_FMULADD: &str = include_str!("../tests/lv/fp_fmuladd.ll");
const FP_SUB_DOUBLE: &str = include_str!("../tests/lv/fp_sub_double.ll");
const FP_NEG_DOUBLE: &str = include_str!("../tests/lv/fp_neg_double.ll");

/// `c[i] = a[i] + b[i]` stayed scalar ("An instruction the vectorizer does not
/// widen yet"); the pin takes VF 8, IC 2 (`opt -passes=loop-vectorize`: four
/// `load <8 x i16>`, step 16) and adds the wide values.
#[test]
fn an_add_of_two_loaded_arrays_is_widened() {
    let text = run(ADD);
    assert_eq!(count(&text, "load <8 x i16>"), 4, "{text}");
    assert_eq!(count(&text, "add <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "store <8 x i16>"), 2, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 16"), "{text}");
    assert_eq!(count(&text, "load i16"), 0, "{text}");
}

/// `c[i] = a[i] * b[i] + 2.5` over `float` stayed scalar ("An element type the
/// target has no vector of"); the pin takes VF 4, IC 2 (`opt
/// -passes=loop-vectorize -mcpu=generic`: four `load <4 x float>`, step 8) and
/// splats the constant.
#[test]
fn a_float_multiply_add_of_loaded_arrays_is_widened() {
    let text = run(FP_AXPY);
    assert_eq!(count(&text, "load <4 x float>"), 4, "{text}");
    assert_eq!(count(&text, "fmul <4 x float>"), 2, "{text}");
    assert_eq!(count(&text, "fadd <4 x float>"), 2, "{text}");
    assert_eq!(count(&text, "store <4 x float>"), 2, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 8"), "{text}");
    assert_eq!(count(&text, "load float"), 0, "{text}");
}

/// `d[i] = fmuladd(a[i], k, b[i])`: "Found a non-intrinsic callsite" kept the
/// loop scalar; the pin widens the call (VF 4, IC 2: two `llvm.fmuladd.v4f32`,
/// `k` splatted, step 8).
#[test]
fn a_float_fmuladd_call_is_widened_to_a_vector_intrinsic() {
    let text = run(FP_FMULADD);
    assert_eq!(count(&text, "call <4 x float> @llvm.fmuladd.v4f32("), 2, "{text}");
    assert_eq!(count(&text, "call float @llvm.fmuladd.f32"), 0, "{text}");
    assert_eq!(count(&text, "load <4 x float>"), 4, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 8"), "{text}");
}

/// `b[i] = (float)a[i] * 2.5f`: "An instruction the vectorizer does not widen
/// yet" kept the `sitofp` loop scalar; the pin widens it (VF 4, IC 2: two
/// `sitofp <4 x i32>`, step 8).
#[test]
fn an_int_to_float_conversion_is_widened_with_its_operand() {
    let text = run(FP_CONVERT);
    assert_eq!(count(&text, "sitofp <4 x i32>"), 2, "{text}");
    assert_eq!(count(&text, "fmul <4 x float>"), 2, "{text}");
    assert_eq!(count(&text, "load <4 x i32>"), 2, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 8"), "{text}");
}

/// `b[i] = (float)a[i] * 2.5f` over `i64`: "A cast that changes the lane width"
/// kept the loop scalar; the pin takes VF 4, IC 1 (`opt
/// -passes=loop-vectorize -mcpu=generic`: one `load <4 x i64>`, one `sitofp
/// <4 x i64> to <4 x float>`, step 4).
#[test]
fn a_narrowing_int_to_float_conversion_is_widened() {
    let text = run(FP_CONVERT_NARROW);
    assert_eq!(count(&text, "load <4 x i64>"), 1, "{text}");
    assert_eq!(count(&text, "sitofp <4 x i64> "), 1, "{text}");
    assert_eq!(count(&text, "fmul <4 x float>"), 1, "{text}");
    assert_eq!(count(&text, "store <4 x float>"), 1, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 4"), "{text}");
}

/// The IDCT pass-1 shape (two stride-8 `i32` loads, `sitofp`, `fmuladd`,
/// consecutive store): "An access that is not consecutive" kept it scalar; the
/// pin makes four scalar loads per access and puts them together with
/// `insertelement` (`CM_Scalarize`, VF 4).
#[test]
fn strided_loads_are_scalarized_and_put_together() {
    let text = run(FP_STRIDED);
    assert_eq!(count(&text, "load i32"), 8, "{text}");
    assert_eq!(count(&text, "insertelement <4 x i32>"), 8, "{text}");
    assert_eq!(count(&text, "sitofp <4 x i32>"), 2, "{text}");
    assert_eq!(count(&text, "call <4 x float> @llvm.fmuladd.v4f32("), 2, "{text}");
    assert_eq!(count(&text, "store <4 x float>"), 1, "{text}");
}

/// `double` lanes: pin VF 2, IC 2 (step 4).
#[test]
fn a_double_subtract_of_loaded_arrays_is_widened() {
    let text = run(FP_SUB_DOUBLE);
    assert_eq!(count(&text, "load <2 x double>"), 4, "{text}");
    assert_eq!(count(&text, "fsub <2 x double>"), 2, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 4"), "{text}");
}

/// `c[i] = -(a[i] - b[i])`: the `fneg` was "a binary result used other than by
/// a store or a binary operation"; the pin widens it with the `fsub` (VF 2,
/// IC 2).
#[test]
fn a_float_negation_is_widened_with_its_operand() {
    let text = run(FP_NEG_DOUBLE);
    assert_eq!(count(&text, "fneg <2 x double>"), 2, "{text}");
    assert_eq!(count(&text, "fsub <2 x double>"), 2, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 4"), "{text}");
}

/// A chain of operations feeds one into the next, a constant operand is a
/// splat: pin VF 8, IC 2.
#[test]
fn a_chain_of_binary_operations_is_widened_with_its_constants_splat() {
    let text = run(CHAIN);
    assert_eq!(count(&text, "add <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "mul <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "ashr <8 x i16>"), 2, "{text}");
    assert!(text.contains("<i16 3, i16 3, i16 3, i16 3, i16 3, i16 3, i16 3, i16 3>"), "{text}");
}

/// One loaded array against a constant: pin VF 16, IC 2.
#[test]
fn an_add_of_a_constant_is_widened() {
    let text = run(SPLAT);
    assert_eq!(count(&text, "load <16 x i8>"), 2, "{text}");
    assert_eq!(count(&text, "add <16 x i8>"), 2, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 32"), "{text}");
}

/// A shift by a constant: pin VF 8, IC 2.
#[test]
fn a_shift_by_a_constant_is_widened() {
    let text = run(SHIFT);
    assert_eq!(count(&text, "shl <8 x i16>"), 2, "{text}");
}

/// The pin leaves `i64` multiplies scalar (a `mul <2 x i64>` has no NEON
/// instruction, so the vector costs more) and does not divide in vectors.
#[test]
fn a_mul_of_i64_and_a_division_stay_scalar() {
    for source in [MUL64, UDIV] {
        let text = run(source);
        assert_eq!(count(&text, "load <"), 0, "{text}");
    }
}

/// Where the pin allows no epilogue it does not interleave
/// (`selectInterleaveCount` L3705 returns 1): a trip count under 16 widened i64
/// copy twice (IC 2, four lanes a step) where the pin takes VF 2, IC 1 (`opt
/// -passes=loop-vectorize`, `icmp eq i64 %index.next, 8` after `add nuw i64
/// %index, 2`).
#[test]
fn a_tiny_trip_count_is_not_interleaved() {
    let text = run(&COPY.replace("i16", "i64").replace("align 2", "align 8").replace("1920", "8"));
    assert_eq!(count(&text, "load <2 x i64>"), 1, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 2"), "{text}");
}

/// `-O1` runs the pass with `VectorizeOnlyWhenForced`: the pin (and clang)
/// widen nothing there, and the scroll fill was widened at O1.
#[test]
fn a_loop_is_left_alone_when_only_forced_loops_are_vectorized() {
    let mut module = crate::testing::parsed(COPY);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopVectorize { always: true, only_when_forced: true });
    manager
        .run_module(
            &mut module,
            std::rc::Rc::new(llrm_arm64::machine::Arm64Machine { platform: llrm_arm64::machine::Platform::Linux }),
        )
        .unwrap();
    assert_eq!(count(&crate::testing::printed(&module), "load <"), 0);
}

/// A trip count VF x IC does not divide (1921) was left scalar ("A scalar
/// epilogue would remain"); the pin widens the first 1920 and leaves the last
/// iteration to a copy of the loop, entered at 1920 (`opt
/// -passes=loop-vectorize` on this loop: `icmp eq i64 %index.next, 1920`, then
/// `%i = phi [ 1920, ..]`).
#[test]
fn a_trip_count_with_a_remainder_is_widened_with_a_scalar_remainder() {
    let text = run(&COPY.replace("1920", "1921"));
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "load i16"), 1, "{text}");
    assert!(text.contains("icmp eq i64 %next, 1920"), "{text}");
    assert!(text.contains("%i = phi i64 [ 0, %entry ]"), "{text}");
    assert!(text.contains("%i.scalar = phi i64 [ 1920, %loop ]"), "{text}");
    assert!(text.contains("icmp eq i64 %next.scalar, 1921"), "{text}");
}

/// The remainder of a loop that starts at 8 resumes at the vector loop's end,
/// `8 + 1920`.
#[test]
fn a_remainder_resumes_where_the_vector_loop_ends() {
    let text = run(&include_str!("../tests/lv/start8.ll").replace("1928", "1929"));
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
    assert!(text.contains("icmp eq i64 %next, 1928"), "{text}");
    assert!(text.contains("phi i64 [ 1928, %loop ]"), "{text}");
}

/// Under 16 iterations (TinyTripCountVectorThreshold) and at `-Os` the pin
/// allows no epilogue: the loop stays scalar.
#[test]
fn no_remainder_is_made_where_the_pin_allows_none() {
    let text = run(&COPY.replace("1920", "15"));
    assert_eq!(count(&text, "<8 x i16>"), 0, "{text}");
    let text = run(&COPY.replace("1920", "1921").replace(
        "define void @f(ptr noalias %a, ptr noalias %b, i64 %n)",
        "define void @f(ptr noalias %a, ptr noalias %b, i64 %n) optsize",
    ));
    assert_eq!(count(&text, "<8 x i16>"), 0, "{text}");
}

/// DEVIATE LV-OS: an optsize loop whose trip count VF divides was widened and
/// added bytes at -Os (histo +4, scroll +4, jpeg_fixed +24); it stays scalar.
#[test]
fn an_optsize_loop_stays_scalar_even_when_vf_divides_the_trip_count() {
    let text = run(&COPY.replace(
        "define void @f(ptr noalias %a, ptr noalias %b, i64 %n)",
        "define void @f(ptr noalias %a, ptr noalias %b, i64 %n) optsize",
    ));
    assert_eq!(count(&text, "<8 x i16>"), 0, "{text}");
    let text = run(&COPY.replace(
        "define void @f(ptr noalias %a, ptr noalias %b, i64 %n)",
        "define void @f(ptr noalias %a, ptr noalias %b, i64 %n) minsize",
    ));
    assert_eq!(count(&text, "<8 x i16>"), 0, "{text}");
    assert_eq!(count(&run(COPY), "<8 x i16>") > 0, true);
}

/// A trip count `n` the loop is given: the pin checks `n < 16` (minimum
/// iterations), widens `n - n % 16` iterations (`n.vec`), tests `n == n.vec` in
/// the middle block and enters the remainder at `n.vec`, or at 0 for a short
/// loop.
#[test]
fn a_symbolic_trip_count_is_checked_at_run_time() {
    let text = run(include_str!("../tests/lv/symbolic_trip.ll"));
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "load i16"), 1, "{text}");
    assert!(text.contains("%min.iters.check = icmp ult i64 %n, 16"), "{text}");
    assert!(text.contains("%n.mod.vf = and i64 %n, 15"), "{text}");
    assert!(text.contains("%n.vec = sub i64 %n, %n.mod.vf"), "{text}");
    assert!(text.contains("icmp eq i64 %next, %n.vec"), "{text}");
    assert!(text.contains("%cmp.n = icmp eq i64 %n, %n.vec"), "{text}");
    assert!(text.contains("%bc.resume.val = phi i64 [ %n.vec, %middle.block ], [ 0, %entry ]"), "{text}");
    assert!(text.contains("br i1 %min.iters.check, label %scalar.ph, label %vector.ph"), "{text}");
}

/// A loop starting at 8 against a symbolic bound: the iterations are
/// `bound - 8` and the vector loop ends at `8 + n.vec` (`ind.end`).
#[test]
fn a_symbolic_bound_from_a_nonzero_start_counts_from_the_start() {
    let text = run(&include_str!("../tests/lv/symbolic_trip.ll").replace("[ 0, %entry ]", "[ 8, %entry ]"));
    assert!(text.contains("%n.trip = sub i64 %n, 8"), "{text}");
    assert!(text.contains("%ind.end = add i64 8, %n.vec"), "{text}");
    assert!(text.contains("icmp eq i64 %next, %ind.end"), "{text}");
    assert!(text.contains("%bc.resume.val = phi i64 [ %ind.end, %middle.block ], [ 8, %entry ]"), "{text}");
}

/// Run again over its own output the pass would widen the remainder again (its
/// bound is symbolic): the pin marks the loops `llvm.loop.isvectorized` and a
/// later run leaves them alone.
#[test]
fn the_loops_it_made_are_not_widened_again() {
    let once = run(include_str!("../tests/lv/symbolic_trip.ll"));
    assert_eq!(count(&once, "llvm.loop.isvectorized"), 2, "{once}");
    assert_eq!(count(&once, "llvm.loop.unroll.runtime.disable"), 2, "{once}");
    let twice = run(&once);
    assert_eq!(count(&twice, "load <8 x i16>"), 2, "{twice}");
}

/// The store 80 elements ahead of the load bounds the safe distance: the pin
/// keeps VF 8 and IC 1 (remark "vectorization width: 8, interleaved count: 1");
/// IC 2 was the whole copy loop's.
#[test]
fn a_forward_dependence_bounds_the_widening() {
    let text = run(include_str!("../tests/lv/scroll_offset.ll"));
    assert_eq!(count(&text, "load <8 x i16>"), 1, "{text}");
    assert_eq!(count(&text, "store <8 x i16>"), 1, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 8"), "{text}");
}

/// A copy loop whose induction starts at 8 (exit at 1928) was refused as "No
/// canonical induction variable"; the pin widens it like the zero-start loop
/// (VF 8, IC 2): its vector loop counts from its own canonical IV.
#[test]
fn an_induction_that_starts_past_zero_is_widened() {
    let text = run(include_str!("../tests/lv/start8.ll"));
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "store <8 x i16>"), 2, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 16"), "{text}");
}

/// A fill of an invariant value was refused as "A stored value that is not a
/// loaded one"; the pin broadcasts it in the preheader and stores the splat
/// (VF 8, IC 2: remark on `tests/lv/fill.ll`).
#[test]
fn a_fill_with_an_invariant_value_stores_a_splat() {
    let text = run(include_str!("../tests/lv/fill.ll"));
    assert_eq!(count(&text, "store <8 x i16> %broadcast.splat"), 2, "{text}");
    assert_eq!(count(&text, "insertelement <8 x i16> poison, i16 %x, i64 0"), 1, "{text}");
    assert_eq!(count(&text, "store i16"), 0, "{text}");
}

/// The same fill of a constant needs no instruction: the stores take the
/// constant splat (the pin prints `splat (i16 7)`).
#[test]
fn a_fill_with_a_constant_stores_a_constant_splat() {
    let text = run(include_str!("../tests/lv/fill_const.ll"));
    assert_eq!(count(&text, "store <8 x i16> <i16 7,"), 2, "{text}");
    assert_eq!(count(&text, "insertelement"), 0, "{text}");
}

/// A loop returning its last index (`ret %i` through an exit phi) would return
/// the last vector iteration's index (1904, not 1919) once widened; the pin
/// fixes it up in the middle block. Left scalar until that is translated
/// (LVP5).
#[test]
fn an_induction_used_after_the_loop_is_left_scalar() {
    let text = run(include_str!("../tests/lv/exit_value.ll"));
    assert_eq!(count(&text, "<8 x i16>"), 0, "{text}");
}

/// An exit phi of an invariant value gets an arm from the middle block and
/// from the remainder; both leave to the one exit (the arm from the loop that
/// is now the vector loop is gone).
#[test]
fn an_exit_phi_takes_an_arm_from_each_way_out() {
    let source = include_str!("../tests/lv/symbolic_trip.ll")
        .replace("define void", "define i32")
        .replace("ret void", "%r = phi i32 [ 7, %loop ]\n  ret i32 %r");
    let text = run(&source);
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
    assert!(text.contains("%r = phi i32 [ 7, %middle.block ], [ 7, %loop.scalar ]"), "{text}");
}

/// The splat of an invariant stored value is made in `vector.ph`, where the
/// pin makes it, not in the preheader the run-time check branches out of (the
/// short-trip path does not pay for it).
#[test]
fn a_splat_is_made_in_the_vector_preheader() {
    let text = run(include_str!("../tests/lv/symbolic_fill.ll"));
    let at = |label: &str| text.find(label).unwrap_or_else(|| panic!("no {label} in {text}"));
    let splat = at("insertelement <2 x i64> poison, i64 %v");
    assert!(at("vector.ph:") < splat && splat < at("\nloop:"), "{text}");
    assert_eq!(count(&text, "store <2 x i64> %broadcast.splat"), 2, "{text}");
}

const SUM: &str = include_str!("../tests/lv/sum_sext.ll");
const SUM_REMAINDER: &str = include_str!("../tests/lv/sum_sext_rem.ll");

/// A sum of sign-extended loads stayed one scalar loop ("Reductions are not
/// widened yet"); the pin keeps the loop scalar and interleaves it four times
/// (`-force-vector-width=1`: "interleaved loop (interleaved count: 4)"): four
/// copies of the body on their own accumulators, `index + k`, the step 4 and
/// three `bin.rdx` in the middle block.
#[test]
fn a_sum_is_interleaved_four_times_on_separate_accumulators() {
    let text = run(SUM);
    assert_eq!(count(&text, "load i16"), 4, "{text}");
    // The index and the sum, three more sums, and the exit's.
    assert_eq!(count(&text, "= phi i64"), 6, "{text}");
    assert_eq!(count(&text, "= add i64 %sum"), 3, "{text}");
    assert!(text.contains("%bin.rdx2 = add i64 %sum.3, %bin.rdx1"), "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 4"), "{text}");
    assert_eq!(count(&text, "llvm.loop.isvectorized"), 1, "{text}");
}

/// A trip count the copies do not divide leaves a remainder that resumes the
/// sum from `bc.merge.rdx` and hands its own last value to the exit.
#[test]
fn an_interleaved_sum_with_a_remainder_resumes_from_the_folded_value() {
    let text = run(SUM_REMAINDER);
    assert_eq!(count(&text, "= add i64 %sum"), 3, "{text}");
    assert_eq!(count(&text, "load i16"), 5, "{text}");
    // The remainder starts from the folded sum; a constant trip count has no
    // scalar.ph.
    assert!(text.contains("%s.scalar = phi i64 [ %bin.rdx2, %middle.block ]"), "{text}");
    assert!(!text.contains("bc.merge.rdx"), "{text}");
    assert!(text.contains("%r = phi i64 [ %sum.scalar, %loop.scalar ]"), "{text}");
}

const SUM_N: &str = include_str!("../tests/lv/sum_sext_n.ll");

/// A run-time trip count checks `min.iters.check`, and the remainder resumes
/// the sum from `bc.merge.rdx` (the folded value, or the start where the
/// interleaved loop was skipped), as the pin's (IC 4).
#[test]
fn an_interleaved_sum_of_a_run_time_count_merges_the_sum_at_scalar_ph() {
    let text = run(SUM_N);
    assert!(text.contains("min.iters.check"), "{text}");
    assert!(text.contains("%bc.merge.rdx = phi i64 [ %bin.rdx2, %middle.block ], [ 0, %entry ]"), "{text}");
    assert!(text.contains("%s.scalar = phi i64 [ %bc.merge.rdx, %scalar.ph ]"), "{text}");
    assert!(text.contains("%r = phi i64 [ %bin.rdx2, %middle.block ], [ %sum.scalar, %loop.scalar ]"), "{text}");
}

const SUM_STEP_USED: &str = include_str!("../tests/lv/sum_step_used.ll");

/// The body read the induction's step (`counts[i] * (i + 1)`, histo): the
/// interleaver rewrote that `add` to step by 4, so every product was by
/// `i + 4` and the sum came out wrong (histo -O2 gave 535552). The step stays
/// `i + 1` for the body and the induction gets its own `index.next`.
#[test]
fn an_induction_step_the_body_reads_keeps_its_value() {
    let text = run(SUM_STEP_USED);
    assert!(text.contains("%next = add nuw nsw i64 %i, 1"), "{text}");
    assert!(text.contains("%index.next = add i64 %i, 4"), "{text}");
    assert!(text.contains("%done = icmp eq i64 %index.next, 256"), "{text}");
    assert!(text.contains("%i = phi i64 [ 0, %entry ], [ %index.next, %loop ]"), "{text}");
}

const SUM_PLAIN: &str = include_str!("../tests/lv/sum_plain.ll");

/// A sum the pin vectorizes (VF 8: the loads and the add have vector forms)
/// is not interleaved alone: its widening is the pin's answer, and four scalar
/// copies add a remainder and a fold the loop does not need (huge -O2 ran
/// 12,213 more instructions).
#[test]
fn a_sum_a_vector_factor_takes_is_not_interleaved_alone() {
    let text = run(SUM_PLAIN);
    assert_eq!(count(&text, "load i16"), 1, "{text}");
    assert!(!text.contains("bin.rdx"), "{text}");
}

const REM_STORE: &str = include_str!("../tests/lv/rem_store.ll");

/// A remainder by a constant stored per element is vectorized by the pin
/// (huge -O2: VF 8); the target prices no vector division here, which read as
/// "the pin stays scalar" and interleaved the loop alone (+31,612
/// instructions).
#[test]
fn a_remainder_the_pin_may_vectorize_is_not_interleaved_alone() {
    let text = run(REM_STORE);
    assert_eq!(count(&text, "urem i32"), 1, "{text}");
}

const DOT_SEXT: &str = include_str!("../tests/lv/dot_sext.ll");

/// A dot product of extended shorts is vectorized by the pin (dot -O2: VF 8,
/// widening multiplies); the `i64` multiply has no vector price here, which
/// read as scalarized and interleaved the loop alone (+160 instructions).
#[test]
fn a_dot_product_of_extended_values_is_not_interleaved_alone() {
    let text = run(DOT_SEXT);
    assert!(!text.contains("bin.rdx"), "{text}");
}

const FP_CANON_SUM: &str = include_str!("../tests/lv/fp_canon_sum.ll");
const FP_SCALE: &str = include_str!("../tests/lv/fp_scale.ll");

/// `sfm16_float_add`'s loop stayed one scalar loop ("An element type the target
/// has no cost for"): the pin prices VF 4 at 48 against the scalar 11
/// (`-debug-only=loop-vectorize`), keeps it scalar and interleaves it four
/// times.
#[test]
fn a_loop_the_pin_finds_vectorization_not_beneficial_for_is_interleaved() {
    let text = run(FP_CANON_SUM);
    assert_eq!(count(&text, "fcmp uno"), 4, "{text}");
    // Four accumulators and the exit's.
    assert_eq!(count(&text, "= phi i32"), 5, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 4"), "{text}");
    assert_eq!(count(&text, "llvm.loop.isvectorized"), 1, "{text}");
}

/// A scale of consecutive floats the pin widens (VF 4, IC 2) is widened, not
/// interleaved alone: interleaving where a vector factor takes the loop was the
/// +12,213 and +31,612 instruction failure; while floats were not widened the
/// loop stayed scalar for that reason.
#[test]
fn a_float_loop_the_pin_vectorizes_is_widened_not_interleaved_alone() {
    let text = run(FP_SCALE);
    assert_eq!(count(&text, "fmul <4 x float>"), 2, "{text}");
    assert_eq!(count(&text, "fmul float"), 0, "{text}");
    assert_eq!(count(&text, "llvm.loop.isvectorized"), 1, "{text}");
}

const MEMCHECK_COPY: &str = include_str!("../tests/lv/memcheck_copy.ll");
const MEMCHECK_COPY_CONST: &str = include_str!("../tests/lv/memcheck_copy_const.ll");

/// `b[i] = a[i]` over plain pointers stayed scalar (`Runtime memory checks are
/// needed`). The pin's block order (`opt -passes=loop-vectorize`): the
/// iteration check, `vector.memcheck` (one `diff.check`), `vector.ph`; an
/// overlap runs the scalar loop whole, from index 0.
#[test]
fn plain_pointers_are_widened_behind_a_memory_check() {
    let text = run(MEMCHECK_COPY);
    assert_eq!(count(&text, "vector.memcheck:"), 1, "{text}");
    assert_eq!(count(&text, "diff.check = icmp ult"), 1, "{text}");
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
    let check = text.split("vector.memcheck:").nth(1).expect("the block").split("vector.ph:").next().unwrap();
    assert!(check.contains("br i1 %diff.check, label %scalar.ph, label %vector.ph"), "{text}");
    assert!(text.contains("[ 0, %vector.memcheck ]"), "{text}");
}

/// A constant trip count the lanes divide needs no remainder, but a memory
/// conflict runs the scalar loop: the check keeps its `scalar.ph`.
#[test]
fn a_memory_check_keeps_the_remainder_of_a_divisible_constant_trip() {
    let text = run(MEMCHECK_COPY_CONST);
    assert_eq!(count(&text, "vector.memcheck:"), 1, "{text}");
    assert_eq!(count(&text, "scalar.ph:"), 1, "{text}");
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
}

const INDUCTION_TRUNC: &str = include_str!("../tests/lv/induction_trunc.ll");
const INDUCTION_VALUE: &str = include_str!("../tests/lv/induction_value.ll");
const INDUCTION_START: &str = include_str!("../tests/lv/induction_start.ll");

/// `a[i] = ((i * 7) & 255) | 0x700` over `i16` (scroll's init loop) stayed
/// scalar ("A value the loop computes that is not widened": the `trunc` of the
/// induction); the pin takes VF 8, IC 2 on a `<8 x i16>` `vec.ind` that
/// replaces the `trunc`.
#[test]
fn a_truncated_induction_is_widened_to_a_vec_ind() {
    let text = run(INDUCTION_TRUNC);
    assert!(
        text.contains("<i16 0, i16 1, i16 2, i16 3, i16 4, i16 5, i16 6, i16 7>"),
        "{text}"
    );
    assert_eq!(count(&text, "phi <8 x i16>"), 1, "{text}");
    assert_eq!(count(&text, "step.add = add <8 x i16>"), 1, "{text}");
    assert_eq!(count(&text, "vec.ind.next = add <8 x i16>"), 1, "{text}");
    assert_eq!(count(&text, "mul <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "store <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "trunc"), 0, "{text}");
}

/// `a[i] = i + 5` over `i64`: the induction read as a value is a `<2 x i64>`
/// `vec.ind` (pin VF 2, IC 2).
#[test]
fn an_induction_read_as_a_value_is_widened() {
    let text = run(INDUCTION_VALUE);
    assert!(text.contains("<i64 0, i64 1>"), "{text}");
    assert_eq!(count(&text, "phi <2 x i64>"), 1, "{text}");
    assert_eq!(count(&text, "add <2 x i64>"), 4, "{text}");
    assert_eq!(count(&text, "store <2 x i64>"), 2, "{text}");
}

/// A run-time start: the lanes begin at the truncated start plus the lane
/// numbers, made in the vector preheader (`induction`).
#[test]
fn an_induction_that_starts_at_a_run_time_value_is_widened() {
    let text = run(INDUCTION_START);
    assert!(text.contains("induction = add <8 x i16>"), "{text}");
    assert_eq!(count(&text, "phi <8 x i16>"), 1, "{text}");
    assert_eq!(count(&text, "store <8 x i16>"), 2, "{text}");
}

const REVERSE_COPY: &str = include_str!("../tests/lv/reverse_copy.ll");
const REVERSE_SYMBOLIC: &str = include_str!("../tests/lv/reverse_symbolic.ll");
const REVERSE_MIXED: &str = include_str!("../tests/lv/reverse_mixed.ll");
const FORWARD_PHI_EXIT: &str = include_str!("../tests/lv/forward_phi_exit.ll");

/// scroll's `for (i = 1919; i >= 0; --i) s[i + 80] = s[i]` stayed 1920 scalar
/// iterations ("No integer induction that steps by one"); the pin takes VF 8,
/// IC 2: loads at `-7` and `-15` elements from the address of the first
/// iteration, stores likewise, no `shufflevector` (the reverses cancel), and
/// the loop leaves when the stepped induction is `-1`.
#[test]
fn a_reverse_copy_is_widened_on_the_lowest_addresses_without_a_shuffle() {
    let text = run(REVERSE_COPY);
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "store <8 x i16>"), 2, "{text}");
    assert_eq!(count(&text, "i64 -7"), 2, "{text}");
    assert_eq!(count(&text, "i64 -15"), 2, "{text}");
    assert_eq!(count(&text, "shufflevector"), 0, "{text}");
    assert!(text.contains("add nsw i64 %i, -16"), "{text}");
    assert!(text.contains("icmp eq i64 %next, -1"), "{text}");
}

/// The same with a run-time start: the iterations are `start - bound`, the end
/// `start - n.vec` (the pin: `sub %n, %n.vec`).
#[test]
fn a_reverse_copy_from_a_run_time_start_counts_its_iterations_down() {
    let text = run(REVERSE_SYMBOLIC);
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
    assert!(text.contains("n.trip = sub i64 %n, -1"), "{text}");
    assert!(text.contains("ind.end = sub i64 %n, %n.vec"), "{text}");
    assert!(text.contains("bc.resume.val"), "{text}");
}

/// A loop that counts down and writes forward keeps a reverse `shufflevector`
/// in the pin (VF 8, IC 2); the arm64 selector declines it, so the loop stays
/// scalar until LV-R1.
#[test]
fn a_loop_that_counts_down_and_writes_forward_stays_scalar() {
    let text = run(REVERSE_MIXED);
    assert_eq!(count(&text, "<8 x i16>"), 0, "{text}");
}

/// A loop counting up that tests the induction before its step (`i == 1919`)
/// was left alone ("The exit test is not the stepped induction against a
/// loop-invariant bound"); the pin takes VF 8, IC 2, and the test moves onto
/// the step.
#[test]
fn an_exit_test_on_the_induction_before_its_step_is_moved_onto_the_step() {
    let text = run(FORWARD_PHI_EXIT);
    assert_eq!(count(&text, "load <8 x i16>"), 2, "{text}");
    assert!(text.contains("icmp eq i64 %next, 1920"), "{text}");
}

/// The widened loops compute what the scalar ones do: a fill, then scroll's
/// overlapping reverse copy (`s[i + 80] = s[i]`, `i` from 1919 down), then a
/// checksum of elements across the seams, run by the interpreter before and
/// after. A wrong lane order, an end pointer off by a lane or an exit never
/// taken changes the answer (or exhausts the fuel).
#[test]
fn a_reverse_copy_keeps_the_answers_of_the_scalar_loop() {
    let source = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
@s = global [2000 x i16] zeroinitializer, align 2
define i64 @f() {
entry:
  br label %fill
fill:
  %i = phi i64 [ 0, %entry ], [ %in, %fill ]
  %t = trunc i64 %i to i16
  %m = mul i16 %t, 7
  %v = add i16 %m, 3
  %p = getelementptr inbounds i16, ptr @s, i64 %i
  store i16 %v, ptr %p, align 2
  %in = add nuw nsw i64 %i, 1
  %fd = icmp eq i64 %in, 2000
  br i1 %fd, label %back, label %fill
back:
  br label %loop
loop:
  %j = phi i64 [ 1919, %back ], [ %next, %loop ]
  %a = getelementptr inbounds i16, ptr @s, i64 %j
  %x = load i16, ptr %a, align 2
  %b = getelementptr inbounds i8, ptr %a, i64 160
  store i16 %x, ptr %b, align 2
  %next = add nsw i64 %j, -1
  %done = icmp eq i64 %j, 0
  br i1 %done, label %exit, label %loop
exit:
  %p0 = getelementptr inbounds i16, ptr @s, i64 0
  %p1 = getelementptr inbounds i16, ptr @s, i64 79
  %p2 = getelementptr inbounds i16, ptr @s, i64 80
  %p3 = getelementptr inbounds i16, ptr @s, i64 81
  %p4 = getelementptr inbounds i16, ptr @s, i64 1000
  %p5 = getelementptr inbounds i16, ptr @s, i64 1999
  %l0 = load i16, ptr %p0, align 2
  %l1 = load i16, ptr %p1, align 2
  %l2 = load i16, ptr %p2, align 2
  %l3 = load i16, ptr %p3, align 2
  %l4 = load i16, ptr %p4, align 2
  %l5 = load i16, ptr %p5, align 2
  %e0 = zext i16 %l0 to i64
  %e1 = zext i16 %l1 to i64
  %e2 = zext i16 %l2 to i64
  %e3 = zext i16 %l3 to i64
  %e4 = zext i16 %l4 to i64
  %e5 = zext i16 %l5 to i64
  %s1 = shl i64 %e1, 16
  %s2 = shl i64 %e2, 32
  %s3 = shl i64 %e3, 8
  %s4 = shl i64 %e4, 24
  %s5 = shl i64 %e5, 40
  %o1 = xor i64 %e0, %s1
  %o2 = xor i64 %o1, %s2
  %o3 = xor i64 %o2, %s3
  %o4 = xor i64 %o3, %s4
  %o5 = xor i64 %o4, %s5
  ret i64 %o5
}
";
    let before = crate::testing::parsed(source);
    let after = crate::testing::parsed(&run(source));
    let answer = |module: &llrm_mir::module::Module| {
        llrm_mir::interpret::run(module, "f", Vec::new(), 2_000_000).unwrap_or_else(|trap| panic!("{trap:?}"))
    };
    let text = run(source);
    assert!(text.contains("<8 x i16>"), "the reverse loop was not widened:\n{text}");
    assert_eq!(answer(&after), answer(&before), "{text}");
}

const REDUCTION_SUM: &str = include_str!("../tests/lv/reduction_sum.ll");
const SCROLL_CHECKSUM: &str = include_str!("../tests/lv/scroll_checksum.ll");

/// `Reductions are not widened yet` left every sum of a loop scalar. The sum of
/// 1003 words is a vector phi per part from `<start, 0, 0, 0>`, folded in the
/// middle block and reduced by `llvm.vector.reduce.add`, with the remainder
/// resuming from the total.
#[test]
fn a_sum_is_a_vector_phi_reduced_in_the_middle_block() {
    let text = run(REDUCTION_SUM);
    assert!(count(&text, "vec.phi") >= 1, "{text}");
    assert_eq!(count(&text, "call i32 @llvm.vector.reduce.add.v4i32"), 1, "{text}");
    // A constant trip count enters the vector loop for sure: no merge phi.
    assert!(text.contains("phi i32 [ %rdx, %middle.block ]"), "{text}");
    assert!(text.contains("insertelement <4 x i32> zeroinitializer, i32 %start, i64 0"), "{text}");
}

/// The pin leaves scroll's checksum (a sum of extended products) scalar and
/// interleaves it ("the cost-model indicates that vectorization is not
/// beneficial", interleaved count 4): a loop the pin declines is declined.
#[test]
fn the_checksum_the_pin_declines_stays_scalar() {
    let text = run(SCROLL_CHECKSUM);
    assert_eq!(count(&text, "x i64>"), 0, "{text}");
    assert_eq!(count(&text, "vector.reduce"), 0, "{text}");
}

/// A sum run by the interpreter before and after widening: a wrong lane in the
/// start, a part lost in the fold or a remainder that restarts from the start
/// changes the answer.
#[test]
fn a_widened_sum_keeps_the_answer_of_the_scalar_loop() {
    let source = "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"
@s = global [2000 x i32] zeroinitializer, align 4
define i32 @f() {
entry:
  br label %fill
fill:
  %i = phi i64 [ 0, %entry ], [ %in, %fill ]
  %t = trunc i64 %i to i32
  %m = mul i32 %t, 7
  %v = add i32 %m, 3
  %p = getelementptr inbounds i32, ptr @s, i64 %i
  store i32 %v, ptr %p, align 4
  %in = add nuw nsw i64 %i, 1
  %fd = icmp eq i64 %in, 2000
  br i1 %fd, label %sum, label %fill
sum:
  br label %loop
loop:
  %j = phi i64 [ 0, %sum ], [ %jn, %loop ]
  %acc = phi i32 [ 100, %sum ], [ %next, %loop ]
  %q = getelementptr inbounds i32, ptr @s, i64 %j
  %x = load i32, ptr %q, align 4
  %next = add nsw i32 %acc, %x
  %jn = add nuw nsw i64 %j, 1
  %done = icmp eq i64 %jn, 1003
  br i1 %done, label %exit, label %loop
exit:
  %r = phi i32 [ %next, %loop ]
  ret i32 %r
}
";
    let answer = |text: &str| {
        let module = parsed(text);
        llrm_mir::interpret::run(&module, "f", Vec::new(), 2_000_000)
    };
    let before = answer(source);
    let after_text = run(source);
    assert!(after_text.contains("llvm.vector.reduce.add"), "the sum was not widened:\n{after_text}");
    assert_eq!(format!("{:?}", answer(&after_text)), format!("{before:?}"), "{after_text}");
}
