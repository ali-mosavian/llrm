//! `getArithmeticInstrCost` is a unit cost for every kind but throughput (`BasicTTIImpl.h`), so the unroller counts
//! `compare_to_int64`'s 64-bit arithmetic as the pin does. Priced in clocks it kept the 3x3 loop nest rolled at -O3:
//! 758 B, 31218 T; folded it is two `report` calls.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

const IR: &str = include_str!("data/compare_to_int64.ll");

#[test]
fn a_nest_of_i64_arithmetic_with_constant_trips_folds_at_o3() {
    let request = Request::new(Level::O3);
    let prepared = prepare(IR, &request).expect("prepares");
    let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == "main").expect("main");
    let text = text_in(&prepared, name, number, &request).expect("compiles");
    assert_eq!(text.matches("call _report").count(), 2, "{text}");
    assert!(text.lines().count() < 40, "the loops stayed:\n{text}");
}
