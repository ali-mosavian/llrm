//! `bench_sieve` (the pin's -O2 IR): the `ret` phi takes 0 from the entry and
//! from the block after the clear, and the sum from the loop. The pin's
//! `MachineCSE` makes the two zeros one (`moveq #0,%d2` in the entry, one exit);
//! the m68k driver had no CSE, so each early return kept its own
//! `moveq #0,%d0; bra exit` block (59 lines against 53).

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const SIEVE: &str = include_str!("fixtures/sieve_pin_o2.ll");

fn text(shared_cse: bool) -> String {
    let mut module = llrm_mir::parse::module(SIEVE).expect("parses");
    llrm_mir::parse::upgrade_lifetimes(&mut module);
    text_with(&module, "bench_sieve", Cpu::M68000, Options { shared_cse, ..Options::default() }).expect("compiles")
}

fn zero_exits(asm: &str) -> usize {
    asm.matches("moveq\t#0, %d0\n\tbra").count()
}

/// The premise: without the stage the early return of 0 is a block of its own.
#[test]
fn without_cse_each_early_return_of_zero_is_a_block_of_its_own() {
    let asm = text(false);
    assert!(zero_exits(&asm) >= 1, "the shape changed:\n{asm}");
}

#[test]
fn with_cse_the_zeros_of_the_return_phi_are_one_definition() {
    let (plain, shared) = (text(false), text(true));
    assert_eq!(zero_exits(&shared), 0, "a zero exit stayed:\n{shared}");
    assert!(shared.lines().count() < plain.lines().count(), "no smaller:\n{shared}");
}
