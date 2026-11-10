//! A multiply by a constant of three signed digits is two adds and the shifts, not a call
//! (`x * 11` was a 2,200-clock `__mulsi3` call; gcc synthesizes it). Priced against the call per
//! width: a size build and a constant that takes too many shifted bits keep the call.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

fn asm(
    functions: &str,
    level: Level,
) -> String {
    let ir = format!("target datalayout = \"{}\"\n{functions}", datalayout());
    assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"))
}

fn by(
    width: u32,
    constant: i64,
    attrs: &str,
) -> String {
    format!("define i{width} @by_k(i{width} %x) {attrs} nounwind {{\n  %r = mul i{width} %x, {constant}\n  ret i{width} %r\n}}\n")
}

fn calls(width: u32) -> &'static str {
    match width {
        8 => "__mulqi3",
        16 => "__mulhi3",
        _ => "__mulsi3",
    }
}

/// `x * 11` is `8 + 2 + 1`; at every width it is shifts and adds at speed.
#[test]
fn a_three_digit_constant_is_shifts_and_adds_at_speed() {
    for width in [16, 32] {
        let text = asm(&by(width, 11, ""), Level::O2);
        assert!(!text.contains(calls(width)), "i{width}:\n{text}");
    }
}

/// The non-adjacent form is shorter in digits only when it is: `23 = 32 - 8 - 1`.
#[test]
fn a_constant_of_signed_digits_is_synthesized_too() {
    let text = asm(&by(16, 23, ""), Level::O2);
    assert!(!text.contains("__mulhi3"), "{text}");
}

/// A negative constant and one with trailing zeros: `-11`, `44 = 11 << 2`.
#[test]
fn a_negative_constant_and_trailing_zeros_are_synthesized() {
    for constant in [-11, 44] {
        let text = asm(&by(16, constant, ""), Level::O2);
        assert!(!text.contains("__mulhi3"), "{constant}:\n{text}");
    }
}

/// Four digits are three adds: the call stays.
#[test]
fn a_constant_of_four_digits_stays_a_call() {
    let text = asm(&by(32, 91, ""), Level::O2);
    assert!(text.contains("jsr __mulsi3"), "{text}");
}

/// A function tuned for size keeps the call while the routine is linked anyway (a variable multiply
/// beside it): the steps are more bytes than the call at every width.
#[test]
fn a_size_function_keeps_the_call() {
    let variable = "define i32 @by_argument(i32 %x, i32 %y) minsize nounwind {\n  %r = mul i32 %x, %y\n  ret i32 %r\n}\n";
    let text = asm(&format!("{}{variable}", by(32, 11, "minsize")), Level::Os);
    let body = &text[text.find("by_k:").unwrap()..text.find("by_argument:").unwrap_or(text.len())];
    assert!(body.contains("jsr __mulsi3"), "{text}");
}

/// A byte multiplied by `145 = 128 + 16 + 1` shifts seven bits: 60 clocks saved for 21 more bytes is
/// under the 3 clocks a byte the matrix prices, so the call stays.
#[test]
fn the_call_wins_where_the_shifts_do_not_pay() {
    let text = asm(&by(8, 145, ""), Level::O2);
    assert!(text.contains("__mulqi3"), "{text}");
}

/// The synthesized sequences against the routine on sim65: `x * k` for 4,096 words, `k` in 11, 23, -11, 44.
#[test]
fn the_sequences_agree_with_the_routine() {
    let constants = [11i64, 23, -11, 44];
    let mut ir = String::from(
        "define i16 @oracle(i16 %x, i16 %y) nounwind {\n  %r = mul i16 %x, %y\n  ret i16 %r\n}\n",
    );
    for k in constants {
        ir += &by(16, k, "").replace("@by_k", &format!("@by_{}", k.unsigned_abs() + if k < 0 { 1000 } else { 0 }));
    }
    let compiled = asm(&ir, Level::O2);
    assert!(!compiled.contains("jsr __mulhi3") || compiled.matches("__mulhi3").count() <= 2, "{compiled}");
    let functions: String =
        compiled.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
    let mut main = String::from("  lda #0\n  sta bad\n  sta ia\nouter:\n  lda #0\n  sta ib\ninner:\n");
    for (n, k) in constants.iter().enumerate() {
        let name = format!("by_{}", k.unsigned_abs() + if *k < 0 { 1000 } else { 0 });
        let word = (*k as i32 & 0xFFFF) as u16;
        main += &format!(
            "  lda ia\n  sta rc2\n  lda ib\n  sta rc3\n  jsr {name}\n  sta lo\n  stx hi\n  lda ia\n  sta rc2\n  lda ib\n  sta rc3\n  \
             lda #<{word}\n  sta rc4\n  lda #>{word}\n  sta rc5\n  jsr oracle\n  cmp lo\n  bne bad{n}\n  cpx hi\n  beq ok{n}\nbad{n}:\n  lda #{code}\n  sta bad\nok{n}:\n",
            code = n + 1
        );
    }
    main += "  lda ib\n  clc\n  adc #17\n  sta ib\n  bcs next\n  jmp inner\nnext:\n  inc ia\n  beq done\n  jmp outer\ndone:\n  lda bad\n";
    let extra = format!("{functions}.segment \"BSS\"\nia: .res 1\nib: .res 1\nlo: .res 1\nhi: .res 1\nbad: .res 1\n.segment \"CODE\"\n");
    for target in ["sim6502", "sim65c02"] {
        assert_eq!(common::run_on(target, &format!("mulsynth-{target}"), &main, &extra).0, 0, "{target}");
    }
    let cycles = common::cycles_on("sim6502", "mulsynth-cycles", &main, &extra);
    assert!(cycles > 1_000_000, "the driver ran {cycles} cycles");
}
