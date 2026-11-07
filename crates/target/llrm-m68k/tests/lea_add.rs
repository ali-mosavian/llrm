//! `(x & 15) | 48`, which the mid-end writes for the sum `(x & 15) + 48`, was
//! `move.l %d0,%a0; lea (48,%a0),%a0; move.l %a0,%d0` (8 bytes): the selector's
//! `lea` for an add of an immediate, round-tripped through an address register.
//! It is `add.l #48,%d0` (6 bytes), `addq.l` for 1 to 8, where the sum is data;
//! the sum is data whenever no register read of it is an address (an address
//! use folds into the load or store: `(48,%a0)`).

use llrm_m68k::compile::{Options, text_with};
use llrm_m68k::dagisel::hooks::Cpu;

const DATALAYOUT: &str = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16";

fn text(
    cpu: Cpu,
    size: bool,
    body: &str,
) -> String {
    let module = llrm_mir::parse::module(&format!("target datalayout = \"{DATALAYOUT}\"\n{body}")).expect("parses");
    text_with(&module, "g", cpu, Options { size, mid_end: false, ..Options::default() }).expect("compiles")
}

fn sum(
    mask: i64,
    constant: i64,
) -> String {
    format!(
        "define i32 @g(i32 %x) {{
  %a = and i32 %x, {mask}
  %b = or disjoint i32 %a, {constant}
  ret i32 %b
}}"
    )
}

#[test]
fn a_data_sum_is_not_a_round_trip_through_an_address_register() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        for size in [false, true] {
            let out = text(cpu, size, &sum(15, 48));
            assert!(!out.contains("lea") && !out.contains("%a0"), "{cpu:?} size={size}:\n{out}");
            assert!(out.contains("#48"), "{cpu:?} size={size}:\n{out}");
        }
    }
}

#[test]
fn a_small_data_sum_is_addq() {
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text(cpu, true, &sum(248, 3));
        assert!(out.contains("addq.l\t#3, %d0") && !out.contains("lea"), "{cpu:?}:\n{out}");
    }
}

const HEX: &str = include_str!("fixtures/hex_cc.ll");

/// hex's digit: the `or disjoint` arm of a select beside an `add` arm, stored
/// as a byte and read again after the loop. The sum was read twice, so the
/// three adjacent instructions the round-trip rewrite wants were not there:
/// `move.l %d2, %a1; lea (48,%a1), %a1` joined `add.w #87` through an address
/// register, 68 B at the 68020 against gcc's 62.
#[test]
fn a_sum_that_joins_a_data_add_and_is_read_twice_stays_data() {
    let module = llrm_mir::parse::module(HEX).expect("parses");
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text_with(&module, "k", cpu, Options { mid_end: false, ..Options::default() }).expect("compiles");
        assert!(!out.contains("lea\t(48,"), "{cpu:?}:\n{out}");
    }
}

/// `moveq #10, %d3; cmp.l %d3, %d1` and `moveq #87, %d3; add.l %d3, %d1` took
/// the free register before the narrowing ran: 12 B and 20 clk more than
/// `cmpi.b #10` and `add.w #87` on hex, the 68000 loop's compare and add.
#[test]
fn a_narrowable_compare_and_add_do_not_take_a_free_register() {
    let module = llrm_mir::parse::module(HEX).expect("parses");
    let out = text_with(&module, "k", Cpu::M68000, Options { mid_end: false, ..Options::default() }).expect("compiles");
    assert!(out.contains("cmpi.b\t#10"), "{out}");
    assert!(out.contains("add.w\t#87"), "{out}");
}
