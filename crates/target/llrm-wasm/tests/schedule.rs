//! Scheduling order at `-O1` and up: the pin picks the BURR order
//! (WebAssemblyISelLowering L57 `Sched::RegPressure`); source order is `-O0`
//! only.

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

/// Source order at -O2 gave `bench_recmany` 9 locals and 2 more instructions
/// than the pin's order (8 locals): recmany 259 B against 251.
#[test]
fn recmany_o2_schedules_in_register_pressure_order() {
    let ir = include_str!("ir/recmany_schedule.ll");
    let text = assembly(ir, Width::W32, Options { level: Level::O2, ..Options::default() }).unwrap();
    let start = text.find("bench_recmany:").expect("the function");
    let locals = text[start..].lines().find(|l| l.trim_start().starts_with(".local")).expect("its locals");
    assert_eq!(locals.matches("i32").count(), 8, "{locals}");
}
