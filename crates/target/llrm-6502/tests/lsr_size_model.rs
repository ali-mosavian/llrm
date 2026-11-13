//! The 6502's byte table for LSR's formulae (`Tti::lsr_size_model`).

use llrm_mir::tti::Tti;

#[test]
fn print() {
    let m = llrm_6502::tti::Mos6502::new().lsr_size_model().expect("the 6502 sizes LSR by bytes");
    println!("{m:?}\n{} {} {} {}", m.step_bytes(8), m.step_bytes(16), m.setup_bytes(8), m.setup_bytes(16));
}
