//! `zext (trunc x)` of a byte that is 0 or 1 is that byte
//! (`visitZERO_EXTEND`, the truncate arm), not a select that tests it.

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::timing::estimate;

fn function(
    predicate: &str,
    level: Level,
) -> String {
    let ir = format!(
        "target datalayout = \"{}\"
define i16 @f(i32 %a, i32 %b) {{
  %c = icmp {predicate} i32 %a, %b
  %r = zext i1 %c to i16
  ret i16 %r
}}",
        datalayout()
    );
    let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("f:").unwrap_or_else(|| panic!("no f in\n{text}"));
    text[from..].split(".segment").next().unwrap().to_owned()
}

/// `a >= b` on longs ended in `ldy #0; cmp #0; beq; ldy #1; tya` after the
/// `and #1; eor #1` that already held 0 or 1: +3 instructions and a branch per
/// compare (long_compare_chain +32 B, +9,100 clk at -O1, 2026-11-10). The
/// signed and the unsigned order, and the operands swapped, make the bit the
/// same way.
#[test]
fn the_not_below_of_two_longs_is_the_bit_it_computed_not_a_select_on_it() {
    for predicate in ["sge", "sle", "uge", "ule"] {
        for level in [Level::O1, Level::Os] {
            let asm = function(predicate, level);
            let branches: Vec<&str> =
                asm.lines().map(str::trim).filter(|l| l.starts_with(['b', 'j']) && !l.starts_with("bit")).collect();
            assert!(branches.is_empty(), "{predicate} {level:?}: the bit was tested again: {branches:?}\n{asm}");
            let bytes = estimate(Cpu::Mos6502, &asm).unwrap_or_else(|e| panic!("{e}\n{asm}")).bytes;
            assert!(bytes <= 56, "{predicate} {level:?}: {bytes} B\n{asm}");
        }
    }
}
