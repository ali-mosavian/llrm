//! The pin's early MachineLICM moves `xor %a, 77777` (a parameter by a constant)
//! out of its loop; the m68k driver had none, so the `eori.l` ran every trip.

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

fn text(shared_licm: bool) -> String {
    let mut module = llrm_mir::parse::module(include_str!("fixtures/licm_invariant.ll")).expect("parses");
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    let options = Options { direct_copies: true, shared_licm, ..Options::default() };
    text_with(&module, "f", Cpu::M68000, options).expect("compiles")
}

/// Whether the `eori.l` comes before the loop's label.
fn xor_before_loop(asm: &str) -> bool {
    let at = |needle: &str| asm.lines().position(|line| line.contains(needle)).unwrap_or_else(|| panic!("no {needle}:\n{asm}"));
    at("eori.l") < at(".LBB0_1:")
}

/// The premise: without the stage the xor is in the loop.
#[test]
fn without_licm_the_xor_runs_every_trip() {
    let asm = text(false);
    assert!(!xor_before_loop(&asm), "the shape changed:\n{asm}");
}

#[test]
fn with_licm_the_xor_of_a_parameter_is_made_before_the_loop() {
    let asm = text(true);
    assert!(xor_before_loop(&asm), "the xor stayed in the loop:\n{asm}");
}
