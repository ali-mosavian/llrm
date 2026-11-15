//! A long division and the remainder of the same operands are one call: the
//! runtime's divide leaves the remainder in `__sr` (`__divmodsi4` for the
//! signed). `has_div_rem_op` was false for them, so DivRemPairs made `x % c` of
//! `x - x / c * c`: `use(x / 5, x % 5)` was a call and 100 bytes of shifts and
//! adds, `x % 100` a second call, `__mulsi3` (div_constants_long: 24,535 B
//! against cc65's 13,862, 2026-11-15).

mod common;

use common::pipeline::datalayout;
use llrm_6502::dagisel::hooks::Cpu;
use llrm_6502::driver::{Level, Request, assembly};

fn asm(
    ir: &str,
    level: Level,
) -> String {
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    let text = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
    text.split(".segment").next().unwrap().to_owned()
}

fn pair_of(
    bits: u32,
    signed: bool,
    divisor: &str,
) -> String {
    let (div, rem) = if signed { ("sdiv", "srem") } else { ("udiv", "urem") };
    format!(
        "declare void @use(i{bits}, i{bits})\n\
         define void @f(i{bits} %x, i{bits} %y) {{\n  %q = {div} i{bits} %x, {divisor}\n  %r = {rem} i{bits} %x, {divisor}\n  call void @use(i{bits} %q, i{bits} %r)\n  ret void\n}}\n"
    )
}

fn pair(
    signed: bool,
    divisor: &str,
) -> String {
    pair_of(32, signed, divisor)
}

fn calls(
    text: &str,
    routine: &str,
) -> usize {
    text.lines().filter(|line| line.trim().starts_with(&format!("jsr {routine}"))).count()
}

/// Four divisors (a variable, a byte, a power-ish constant and one above a
/// byte), both signs, four levels: one runtime call divides and the remainder
/// is read from `__sr`; no second divide, no multiply.
#[test]
fn a_division_and_its_remainder_are_one_call() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        for signed in [false, true] {
            for divisor in ["%y", "5", "100", "1000"] {
                let text = asm(&pair(signed, divisor), level);
                let divides: usize = ["__udivsi3", "__divsi3", "__umodsi3", "__modsi3", "__divmodsi4", "__mulsi3"]
                    .iter()
                    .map(|routine| calls(&text, routine))
                    .sum();
                assert_eq!(divides, 1, "{level:?} signed {signed} / {divisor}:\n{text}");
                assert!(text.contains("__sr"), "{level:?} signed {signed} / {divisor}:\n{text}");
            }
        }
    }
}

/// The pair of a constant divisor is shorter than the two it was (the old text
/// was 164 B for `/ 5`).
#[test]
fn the_pair_by_five_stays_under_a_bound() {
    for (signed, bound) in [(false, 100), (true, 100)] {
        let text = asm(&pair(signed, "5"), Level::O2);
        let bytes = llrm_6502::timing::estimate(Cpu::Mos6502, &text).expect("prices").bytes;
        assert!(bytes <= bound, "signed {signed}: {bytes} B:\n{text}");
    }
}

/// A remainder alone is still a remainder call (no quotient is made for it).
#[test]
fn a_lone_remainder_is_not_a_pair() {
    let text = asm("define i32 @f(i32 %x, i32 %y) {\n  %r = srem i32 %x, %y\n  ret i32 %r\n}\n", Level::O2);
    assert_eq!(calls(&text, "__modsi3"), 1, "{text}");
    assert_eq!(calls(&text, "__divmodsi4"), 0, "{text}");
}

fn long_cell(
    name: &str,
    routine: &str,
    a: u32,
    b: u32,
    cell: &str,
) -> u32 {
    let mut setup = String::new();
    for (to, value) in [("__sa", a), ("__sb", b)] {
        for at in 0..4 {
            setup += &format!("  lda #{}\n  sta {to}+{at}\n", (value >> (8 * at)) & 0xFF);
        }
    }
    (0..4)
        .map(|at| {
            let main = format!("{setup}  jsr {routine}\n  lda {cell}+{at}\n");
            (common::run_with_runtime(&format!("{name}-{cell}-{at}"), &main) as u32) << (8 * at)
        })
        .sum()
}

/// The signed routines of every variant give the quotient in `__sa` and the
/// remainder, of the dividend's sign, in `__sr`: negative dividends and
/// divisors, 1, -1, the extremes, a divisor of a byte and above.
#[test]
fn the_signed_pair_routines_leave_both_results() {
    let cases: [(i32, i32); 5] = [(-100000, 7), (-7, -2), (7, -2), (i32::MIN, 1), (i32::MAX, -255)];
    for routine in ["__divmodsi4", "__divmodsi4_fast", "__divmodsi4_fast_small", "__divmodsi4_fast_byte"] {
        for (a, b) in cases {
            if routine.ends_with("_byte") && !(1..=255).contains(&b) {
                continue;
            }
            let name = format!("{routine}-{a}-{b}").replace('-', "m");
            let q = long_cell(&name, routine, a as u32, b as u32, "__sa");
            let r = long_cell(&name, routine, a as u32, b as u32, "__sr");
            assert_eq!((q as i32, r as i32), (a / b, a % b), "{routine} {a} / {b}");
        }
    }
}

/// The unsigned routines leave the remainder in `__sr` as well.
#[test]
fn the_unsigned_routines_leave_the_remainder_in_sr() {
    for routine in ["__udivsi3", "__udivsi3_fast", "__udivsi3_fast_small", "__udivsi3_fast_byte"] {
        for (a, b) in [(4_000_000_000u32, 7u32), (4_000_000_000, 3_000_000_000), (u32::MAX, 255)] {
            if routine.ends_with("_byte") && !(1..=255).contains(&b) {
                continue;
            }
            let name = format!("{routine}-{a}-{b}");
            let q = long_cell(&name, routine, a, b, "__sa");
            let r = long_cell(&name, routine, a, b, "__sr");
            assert_eq!((q, r), (a / b, a % b), "{routine} {a} / {b}");
        }
    }
}

/// The signed word pair is one `__divmodhi4` (the compact `c` at -Os): `x % c`
/// was `x - x / c * c`, a divide, a multiply by the constant and a subtract
/// (div_constants_short, +177 B against cc65 at -O2 and -Os).
#[test]
fn a_signed_word_division_and_its_remainder_are_one_call() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        for divisor in ["%y", "5", "1000"] {
            let text = asm(&pair_of(16, true, divisor), level);
            let routines = ["__divhi3", "__modhi3", "__mulhi3", "__divmodhi4"];
            let calls: usize = routines.iter().map(|routine| calls(&text, routine)).sum();
            assert_eq!(calls, 1, "{level:?} / {divisor}:\n{text}");
            assert!(text.contains("jsr __divmodhi4"), "{level:?} / {divisor}:\n{text}");
        }
    }
}

fn set(
    register: &str,
    value: u16,
) -> String {
    format!(
        "  lda #{}\n  sta {register}\n  lda #{}\n  sta rc{}\n",
        value & 0xFF,
        value >> 8,
        register[2..].parse::<u32>().unwrap() + 1
    )
}

/// The word read back after `routine`: A:X for the quotient, `rc6:rc7` for the
/// remainder.
fn read(
    name: &str,
    routine: &str,
    a: i16,
    b: i16,
    low: &str,
    high: &str,
) -> i16 {
    let setup = set("rc2", a as u16) + &set("rc4", b as u16);
    let lo = common::run_with_runtime(&format!("{name}-lo"), &format!("{setup}  jsr {routine}\n  {low}\n"));
    let hi = common::run_with_runtime(&format!("{name}-hi"), &format!("{setup}  jsr {routine}\n  {high}\n"));
    (hi << 8 | lo) as u16 as i16
}

/// Truncated toward zero, the remainder of the dividend's sign, in both
/// variants: negative operands, a divisor over 255 (the bit loop), 1 and -1,
/// the extremes (not -32768 / -1).
#[test]
fn the_signed_word_pair_routines_leave_both_results() {
    for routine in ["__divmodhi4", "__divmodhi4c"] {
        for (a, b) in [(-1000i16, 7i16), (7, -2), (-1000, -300), (i16::MIN, 1), (i16::MAX, -1)] {
            let name = format!("{routine}-{a}-{b}").replace('-', "m");
            let q = read(&name, routine, a, b, "", "txa");
            assert_eq!(q, a / b, "{routine} {a} / {b}");
            let r = read(&format!("{name}r"), routine, a, b, "lda rc6", "lda rc7");
            assert_eq!(r, a % b, "{routine} {a} % {b}");
        }
    }
}
