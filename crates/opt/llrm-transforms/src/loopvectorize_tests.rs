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
