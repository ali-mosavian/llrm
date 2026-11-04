//! `(x & 15) | 48`, which the mid-end writes for the sum `(x & 15) + 48`, was
//! `move.l %d0,%a0; lea (48,%a0),%a0; move.l %a0,%d0` (8 bytes): the selector's
//! `lea` for an add of an immediate, round-tripped through an address register.
//! It is `add.l #48,%d0` (6 bytes), `addq.l` for 1 to 8, where the sum is data;
//! a sum that is also an address stays a `lea`.

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
            assert!(out.contains("add.l\t#48, %d0"), "{cpu:?} size={size}:\n{out}");
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

#[test]
fn a_sum_read_as_an_address_and_as_data_stays_a_lea() {
    let body = "define i32 @g(i32 %x) {
  %a = and i32 %x, 15
  %b = or disjoint i32 %a, 48
  %p = inttoptr i32 %b to ptr
  %v = load i32, ptr %p
  %w = add i32 %v, %b
  ret i32 %w
}";
    for cpu in [Cpu::M68000, Cpu::M68020] {
        let out = text(cpu, true, body);
        assert!(out.contains("lea\t(48,%a"), "{cpu:?}:\n{out}");
    }
}
