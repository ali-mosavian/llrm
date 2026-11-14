//! The pin prices a `trunc` to a legal integer at nothing
//! (`TargetTransformInfoImplBase::getCastInstrCost`); charging an `i64` to
//! `i32` truncation one unit made the outer `i < 4` loop of
//! `int64_divmod_edges` 78 against the pin's 70, so 4 copies stayed over the
//! full-unroll threshold of 300 at -O3: +33 executed instructions.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

const DIVMOD_EDGES: &str = include_str!("int64_divmod_edges.ll");

#[test]
fn the_four_trip_outer_loop_of_int64_divmod_edges_is_fully_unrolled_at_o3() {
    let text = assembly(DIVMOD_EDGES, Width::W32, Options { level: Level::O3, ..Options::default() }).unwrap();
    // 8 calls of `report` stay in the two loops that are not unrolled; the four
    // iterations of the third bring the pin's 20.
    assert_eq!(text.matches("call\treport").count(), 20, "{text}");
}
