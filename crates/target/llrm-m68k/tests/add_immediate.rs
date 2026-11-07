//! LSR priced no immediate add as legal, so hex's `*--p` loop kept a second
//! counter for the exit value: `lea (-1,%a0)` per trip beside the pointer
//! (hex 86 -> 84 B, 670 -> 656 clk at 68000).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const HEX: &str = include_str!("../bench/library/hex.ll");

#[test]
fn hex_keeps_one_counter_and_no_pointer_step() {
    for cpu in ["68000", "68020"] {
        let out = assembly(HEX, cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(out.contains("subq.l\t#1, %d"), "{cpu}: the counter is not stepped by subq:\n{out}");
        assert!(!out.contains("lea\t(-1,%a0), %a0"), "{cpu}: a second pointer step:\n{out}");
    }
}

#[test]
fn a_32_bit_immediate_is_legal_for_add_and_compare_and_a_wider_one_is_not() {
    use llrm_m68k::machine::M68k;
    use llrm_mir::tti::Tti;
    let tti = M68k::default();
    for imm in [0, -1, 40, i64::from(i32::MIN), i64::from(i32::MAX)] {
        assert!(tti.is_legal_add_immediate(imm) && tti.is_legal_icmp_immediate(imm), "{imm}");
    }
    let wide = i64::from(i32::MAX) + 1;
    assert!(!tti.is_legal_add_immediate(wide) && !tti.is_legal_icmp_immediate(wide));
}
