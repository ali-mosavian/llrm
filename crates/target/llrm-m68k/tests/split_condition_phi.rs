//! A phi read after a branch on `a || b`, split into two branches at `-Os`.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

const IR: &str = include_str!("data/split_condition_phi.ll");

/// The phi took its input from the first branch's block only; the second
/// branch left for the phi's block with no copy of it, so the value was read
/// before any definition on that path and the coalescer panicked ("Cannot
/// overlap different values", `-Os` 68000).
#[test]
fn a_phi_gets_its_input_from_every_branch_a_split_condition_makes() {
    let text = assembly(IR, Cpu::M68000, Level::Os).expect("compiles");
    assert!(text.contains("rts"), "{text}");
}
