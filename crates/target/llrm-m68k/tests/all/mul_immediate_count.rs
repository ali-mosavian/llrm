//! The 68000 16-bit multiply is priced by its source, not at the 70-clock
//! bound: gcc's `huffman_decode` -O2 ran `mulu.w #72` 52 times and
//! `m68k_clk.py` printed `146830*`, a bound with no count to compare.

use llrm_m68k::isa::Cpu;
use llrm_m68k::timing::cost_on;

fn price(
    text: &str,
    bytes: u32,
) -> (u32, bool) {
    let cost = cost_on(Cpu::M68000, text, bytes, false).unwrap();
    (cost.clocks, cost.partial)
}

#[test]
fn an_unsigned_multiply_by_an_immediate_counts_its_one_bits() {
    // 38 + 2n, n = 2 for 72 (0b1001000), plus 4 for the immediate word
    assert_eq!(price("mulu.w #72, %d1", 4), (46, false));
    assert_eq!(price("mulu.w #0, %d1", 4), (42, false));
    assert_eq!(price("mulu.w #65535, %d1", 4), (74, false));
}

#[test]
fn a_signed_multiply_by_an_immediate_counts_its_bit_transitions() {
    // 72 = 0b0001001000 with a 0 appended: four 01/10 transitions
    assert_eq!(price("muls.w #72, %d1", 4), (50, false));
    assert_eq!(price("muls.w #-1, %d1", 4), (44, false));
}

#[test]
fn a_multiply_by_a_register_stays_a_bound() {
    assert!(price("mulu.w %d2, %d1", 2).1);
}

#[test]
fn a_multiply_by_memory_is_exact_once_the_trace_fixed_its_source() {
    use llrm_m68k::timing::cost_on_counted;
    // `muls.w (%a0)+, %d3`: 4 clocks of addressing over 38 + 2n, n = 3 gives 48
    let counted = cost_on_counted(Cpu::M68000, "muls.w (%a0)+, %d3", 2, false, Some(3)).unwrap();
    assert_eq!((counted.clocks, counted.partial), (48, false));
    assert!(cost_on_counted(Cpu::M68000, "muls.w (%a0)+, %d3", 2, false, None).unwrap().partial);
}

#[test]
fn a_multiply_or_divide_bound_is_a_maximum_and_a_register_shift_is_not() {
    // memmove_overlap 68020 O2 held two `muls.l`: the bound read `4668*` and
    // hid a win over gcc's 8548
    let multiply = cost_on(Cpu::M68020, "muls.l %d1, %d0", 4, false).unwrap();
    assert!(multiply.partial && multiply.maximum);
    let shift = cost_on(Cpu::M68000, "asr.l %d1, %d0", 2, false).unwrap();
    assert!(shift.partial && !shift.maximum, "a shift count at zero is a lower bound");
}
