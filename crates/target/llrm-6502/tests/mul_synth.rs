//! A multiply of 32 bits by a constant of three signed digits is two adds and
//! the shifts when the module has no other multiply call (`x * 11` was a
//! 2,200-clock `__mulsi3` call; gcc synthesizes it). The decision is the
//! module's, as the routine is linked once: -O1 keeps every byte it had,
//! -O2/-O3 also take growth that the removed calls pay for at 3 clocks a byte,
//! -Os takes only a smaller module.

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
    name: &str,
    width: u32,
    constant: i64,
    attrs: &str,
) -> String {
    format!(
        "define i{width} @{name}(i{width} %x) {attrs} nounwind {{\n  %r = mul i{width} %x, {constant}\n  ret i{width} %r\n}}\n"
    )
}

const VARIABLE: &str = "define i32 @by_argument(i32 %x, i32 %y) nounwind {\n  %r = mul i32 %x, %y\n  ret i32 %r\n}\n";

/// `x * 11` is `8 + 2 + 1`: shifts and adds at -O1, -O2 and, the module being
/// smaller, -Os.
#[test]
fn a_three_digit_constant_alone_is_shifts_and_adds() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        let text = asm(&by("by_k", 32, 11, ""), level);
        assert!(!text.contains("__mulsi3"), "{level:?}:\n{text}");
    }
}

/// The non-adjacent form is shorter in digits only when it is (`23 = 32 - 8 -
/// 1`); a negative constant is negated after; trailing zeros are one shift of
/// the sum.
#[test]
fn signed_digits_a_sign_and_trailing_zeros_are_synthesized() {
    for constant in [23, -11, 44] {
        let text = asm(&by("by_k", 32, constant, ""), Level::O2);
        assert!(!text.contains("__mulsi3"), "{constant}:\n{text}");
    }
}

/// The routine is linked for the variable multiply anyway: the longer expansion
/// would only add bytes.
#[test]
fn beside_a_variable_multiply_the_call_stays_at_every_level() {
    for level in [Level::O1, Level::O2, Level::Os] {
        let text = asm(&format!("{}{VARIABLE}", by("by_k", 32, 11, "")), level);
        let body = &text[text.find("by_k:").unwrap()..text.find("by_argument:").unwrap_or(text.len())];
        assert!(body.contains("jsr __mulsi3"), "{level:?}:\n{text}");
    }
}

/// Eight multiplies grow the module by more than the routine they free: -O1 (no
/// byte may rise) and -Os keep the calls; -O2 takes the growth, the eight calls
/// saving 17,600 clocks for under 3 per byte.
#[test]
fn growth_past_the_routine_is_taken_at_speed_only() {
    let module: String = [11, 13, 19, 21, 25, 35, 37, 41].iter().map(|k| by(&format!("by_{k}"), 32, *k, "")).collect();
    for (level, expands) in [(Level::O1, false), (Level::Os, false), (Level::O2, true)] {
        let text = asm(&module, level);
        assert_eq!(!text.contains("__mulsi3"), expands, "{level:?}:\n{text}");
    }
}

/// Four digits are three adds, and a word is a call of a few bytes: both stay
/// calls.
#[test]
fn four_digits_and_a_word_stay_calls() {
    assert!(asm(&by("by_k", 32, 91, ""), Level::O2).contains("jsr __mulsi3"));
    assert!(asm(&by("by_k", 16, 11, ""), Level::O2).contains("__mulhi3"));
}

/// The sequences against repeated addition on sim65: `x * k` for 4,096 words
/// with varying high bytes, `k` in 11, 23, -11, 44.
#[test]
fn the_sequences_agree_with_repeated_addition() {
    let constants = [11i64, 23, -11, 44];
    let name = |k: i64| format!("by_{}", k.unsigned_abs() + if k < 0 { 1000 } else { 0 });
    let ir: String = constants.iter().map(|k| by(&name(*k), 32, *k, "")).collect();
    let compiled = asm(&ir, Level::O2);
    assert!(!compiled.contains("__mulsi3"), "{compiled}");
    let functions: String =
        compiled.lines().filter(|l| !l.starts_with("rc") || !l.contains(" = $")).map(|l| format!("{l}\n")).collect();
    let mut main = String::from("  lda #0\n  sta bad\n  sta ia\nouter:\n  lda #0\n  sta ib\ninner:\n");
    main += "  lda ia\n  sta xb0\n  sta xb3\n  lda ib\n  sta xb1\n  sta xb2\n";
    for (n, k) in constants.iter().enumerate() {
        main += &format!("  lda xb2\n  sta rc2\n  lda xb3\n  sta rc3\n  ldx xb1\n  lda xb0\n  jsr {}\n", name(*k));
        main += "  sta r0\n  stx r1\n  lda rc2\n  sta r2\n  lda rc3\n  sta r3\n";
        main +=
            &format!("  lda #0\n  sta a0\n  sta a1\n  sta a2\n  sta a3\n  ldy #{}\nadd{n}:\n  clc\n", k.unsigned_abs());
        for b in 0..4 {
            main += &format!("  lda a{b}\n  adc xb{b}\n  sta a{b}\n");
        }
        main += &format!("  dey\n  bne add{n}\n");
        if *k < 0 {
            main += "  sec\n";
            for b in 0..4 {
                main += &format!("  lda #0\n  sbc a{b}\n  sta a{b}\n");
            }
        }
        for b in 0..4 {
            main += &format!("  lda r{b}\n  cmp a{b}\n  bne bad{n}\n");
        }
        main += &format!("  beq ok{n}\nbad{n}:\n  lda #{}\n  sta bad\nok{n}:\n", n + 1);
    }
    main += "  lda ib\n  clc\n  adc #61\n  sta ib\n  bcs next\n  jmp inner\nnext:\n  inc ia\n  beq done\n  jmp outer\ndone:\n  lda bad\n";
    let mut extra = format!("{functions}.segment \"BSS\"\nia: .res 1\nib: .res 1\nbad: .res 1\n");
    for v in ["xb", "a", "r"] {
        for b in 0..4 {
            extra += &format!("{v}{b}: .res 1\n");
        }
    }
    extra += ".segment \"CODE\"\n";
    for target in ["sim6502", "sim65c02"] {
        assert_eq!(common::run_on(target, &format!("mulsynth-{target}"), &main, &extra).0, 0, "{target}");
    }
}
