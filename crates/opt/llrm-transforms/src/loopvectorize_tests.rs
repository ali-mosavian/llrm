//! `LoopVectorize` on the arm64 cost model (`always`): the loops the pin
//! widens, with the pin's factors (`opt -passes=loop-vectorize` on aarch64: i16
//! copy of 1920 is VF 8, IC 2), and the ones it leaves alone.

use super::LoopVectorize;
use crate::testing::parsed;

fn run(source: &str) -> String {
    let mut module = parsed(source);
    let mut manager = llrm_mir::passes::PassManager::default();
    manager.add(LoopVectorize { always: true });
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
}

/// A trip count VF x IC does not divide would need a remainder loop (VP3): the
/// loop is left alone.
#[test]
fn a_trip_count_with_a_remainder_is_left_scalar() {
    let text = run(&COPY.replace("1920", "1921"));
    assert_eq!(count(&text, "<8 x i16>"), 0, "{text}");
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
