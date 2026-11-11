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

/// A chain of operations feeds one into the next, a constant operand is a splat:
/// pin VF 8, IC 2.
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

/// Where the pin allows no epilogue it does not interleave (`selectInterleaveCount`
/// L3705 returns 1): a trip count under 16 widened i64 copy twice (IC 2, four
/// lanes a step) where the pin takes VF 2, IC 1 (`opt -passes=loop-vectorize`,
/// `icmp eq i64 %index.next, 8` after `add nuw i64 %index, 2`).
#[test]
fn a_tiny_trip_count_is_not_interleaved() {
    let text = run(&COPY.replace("i16", "i64").replace("align 2", "align 8").replace("1920", "8"));
    assert_eq!(count(&text, "load <2 x i64>"), 1, "{text}");
    assert!(text.contains("add nuw nsw i64 %i, 2"), "{text}");
}

/// `-O1` runs the pass with `VectorizeOnlyWhenForced`: the pin (and clang) widen
/// nothing there, and the scroll fill was widened at O1.
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

/// Two pointers that may overlap need the pin's `vector.memcheck` (LoopVectorize
/// L1637), which is not built: `b[i] = a[i]` over plain pointers was widened
/// with no overlap test, so `b = a + 1` read stale lanes. It stays scalar.
#[test]
fn a_copy_between_pointers_that_may_overlap_stays_scalar() {
    let text = run(&COPY.replace("noalias ", ""));
    assert_eq!(count(&text, "load <8 x i16>"), 0, "{text}");
    assert_eq!(count(&text, "load i16"), 1, "{text}");
}

/// A trip count VF x IC does not divide (1921) was left scalar ("A scalar
/// epilogue would remain"); the pin widens the first 1920 and leaves the last
/// iteration to a copy of the loop, entered at 1920 (`opt -passes=loop-vectorize`
/// on this loop: `icmp eq i64 %index.next, 1920`, then `%i = phi [ 1920, ..]`).
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
    let text = run(&COPY.replace("1920", "1921").replace("define void @f(ptr noalias %a, ptr noalias %b, i64 %n)", "define void @f(ptr noalias %a, ptr noalias %b, i64 %n) optsize"));
    assert_eq!(count(&text, "<8 x i16>"), 0, "{text}");
}

/// DEVIATE LV-OS: an optsize loop whose trip count VF divides was widened and
/// added bytes at -Os (histo +4, scroll +4, jpeg_fixed +24); it stays scalar.
#[test]
fn an_optsize_loop_stays_scalar_even_when_vf_divides_the_trip_count() {
    let text = run(&COPY.replace("define void @f(ptr noalias %a, ptr noalias %b, i64 %n)", "define void @f(ptr noalias %a, ptr noalias %b, i64 %n) optsize"));
    assert_eq!(count(&text, "<8 x i16>"), 0, "{text}");
    let text = run(&COPY.replace("define void @f(ptr noalias %a, ptr noalias %b, i64 %n)", "define void @f(ptr noalias %a, ptr noalias %b, i64 %n) minsize"));
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
/// fixes it up in the middle block. Left scalar until that is translated (LVP5).
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
