//! A function the variants of a module build alike is allocated once.
//! loop_fusion -O2 ran the allocator 84 times on 64 distinct inputs (the unroll
//! variants of the module, the source and ILP schedules, the copied and kept
//! return blocks): the compile took 4x the instructions of its -O1.

use llrm_z80::compile::{prepare, take_allocations, text_in};
use llrm_z80::driver::{Level, Request};

const IR: &str = include_str!("data/loop_fusion_o2.ll");

#[test]
fn an_allocation_the_variants_repeat_runs_once() {
    let request = Request::new(Level::O2);
    take_allocations();
    let prepared = prepare(IR, &request).expect("prepares");
    let names: Vec<String> = prepared.names().map(str::to_owned).collect();
    for (number, name) in names.iter().enumerate() {
        text_in(&prepared, name, number, &request).expect("compiles");
    }
    let runs = take_allocations();
    // 64 distinct inputs, and 8 more for the module the alternate LSR order
    // makes (LSR-KEY-TRIAL)
    assert!(runs <= 78, "{runs} allocations for {} functions (72 distinct inputs)", names.len());
}
