//! The low bit of a byte added to another is the carry of a shift: `popcount`'s
//! `c += x & 1; x >>= 1` was `txa; and #1; clc; adc c; sta c; txa; lsr a; tax`,
//! 26 B against SDCC's 23 (2026-11-09).

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::function::select_function;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};
use llrm_6502::select::function_text;
use llrm_6502::timing::estimate;

const POPCOUNT: &str = "define i8 @popcount(i8 %x) {
entry:
  %z = icmp eq i8 %x, 0
  br i1 %z, label %out, label %l
l:
  %v = phi i8 [%x, %entry], [%s, %l]
  %c = phi i8 [0, %entry], [%c1, %l]
  %b = and i8 %v, 1
  %c1 = add i8 %c, %b
  %s = lshr i8 %v, 1
  %k = icmp eq i8 %s, 0
  br i1 %k, label %out, label %l
out:
  %r = phi i8 [0, %entry], [%c1, %l]
  ret i8 %r
}
";

fn body(level: Level) -> String {
    let ir = format!("target datalayout = \"{}\"\n{POPCOUNT}", datalayout());
    let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    let from = text.find("popcount:").unwrap();
    text[from..].split(".segment").next().unwrap().to_owned()
}

/// The bit comes out of `lsr a` and goes in by `adc #0`; no `and #1` is left.
#[test]
fn a_bit_count_loop_takes_the_bit_from_the_carry() {
    for level in [Level::O2, Level::Os] {
        let text = body(level);
        assert!(text.contains("lsr a") && text.contains("adc #0") && !text.contains("and #1"), "{text}");
        let bytes = estimate(Cpu::Mos6502, &text).unwrap_or_else(|e| panic!("{e}\n{text}")).bytes;
        assert!(bytes <= 22, "{bytes} B\n{text}");
    }
}

/// `x + (y & 1)` with no shift of `y` in sight: the same add, exact for every
/// byte of `y`, and `x` wrapping.
#[test]
fn a_byte_plus_the_low_bit_of_another_runs() {
    let ir = "define i8 @plus(i8 %x, i8 %y) {\n  %b = and i8 %y, 1\n  %r = add i8 %x, %b\n  ret i8 %r\n}";
    let text = format!("target datalayout = \"e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8\"\n{ir}");
    let module = llrm_mir::parse::module(&text).expect("parses");
    let selection = select_function(&module, "plus", Cpu::Mos6502).expect("selects");
    let function = function_text(&module, "plus", &selection).expect("emits");
    for (x, y) in [(7u8, 0u8), (7, 1), (7, 2), (7, 255), (255, 1), (255, 2), (0, 0x81)] {
        let setup = format!("  ldx #{y}\n  lda #{x}\n");
        let got = common::run_with(&format!("plus-{x}-{y}"), &format!("{setup}  jsr plus\n"), &function);
        assert_eq!(got, i32::from(x.wrapping_add(y & 1)), "{x} + ({y} & 1)");
    }
}

/// The combiner offers every target node to the hook: the one with a single
/// operand was indexed past its end (the `_` arm took it for a multiply).
#[test]
fn a_node_with_one_operand_is_not_read_as_a_multiply() {
    body(Level::O1);
}
