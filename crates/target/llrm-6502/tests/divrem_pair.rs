//! A long division and the remainder of the same operands are one call: the
//! runtime's divide leaves the remainder in rc4..rc7 (`__divmodsi4`,
//! `__udivmodsi4`). `has_div_rem_op` was false for them, so DivRemPairs made `x
//! % c` of `x - x / c * c`: `use(x / 5, x % 5)` was a call and 100 bytes of
//! shifts and adds, `x % 100` a second call, `__mulsi3` (div_constants_long:
//! 24,535 B against cc65's 13,862, 2026-11-15).

mod common;

use common::Result32;
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
    // a call in tail position is a `jmp`
    text.lines()
        .filter(|line| ["jsr", "jmp"].iter().any(|op| line.trim().starts_with(&format!("{op} {routine}"))))
        .count()
}

/// Four divisors (a variable, a byte, a power-ish constant and one above a
/// byte), both signs, four levels: one runtime call divides and the remainder
/// is read from rc4..rc7; no second divide, no multiply.
#[test]
fn a_division_and_its_remainder_are_one_call() {
    for level in [Level::O1, Level::O2, Level::O3, Level::Os] {
        for signed in [false, true] {
            for divisor in ["%y", "5", "100", "1000"] {
                let text = asm(&pair(signed, divisor), level);
                let divides: usize =
                    ["__udivsi3", "__divsi3", "__umodsi3", "__modsi3", "__divmodsi4", "__udivmodsi4", "__mulsi3"]
                        .iter()
                        .map(|routine| calls(&text, routine))
                        .sum();
                assert_eq!(divides, 1, "{level:?} signed {signed} / {divisor}:\n{text}");
                assert!(text.contains("divmodsi4"), "{level:?} signed {signed} / {divisor}:\n{text}");
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

/// The signed routines of every variant give the quotient in A:X:rc2:rc3 and
/// the remainder, of the dividend's sign, in rc4..rc7: negative dividends and
/// divisors, 1, -1, the extremes, a divisor of a byte and above.
#[test]
fn the_signed_pair_routines_leave_both_results() {
    let cases: [(i32, i32); 5] = [(-100000, 7), (-7, -2), (7, -2), (i32::MIN, 1), (i32::MAX, -255)];
    for routine in ["__divmodsi4", "__divmodsi4_fast", "__divmodsi4_fast_small", "__divmodsi4_fast_lean", "__divmodsi4_fast_byte"] {
        for (a, b) in cases {
            if routine.ends_with("_byte") && !(1..=255).contains(&b) {
                continue;
            }
            let name = format!("{routine}-{a}-{b}").replace('-', "m");
            let q = common::long_call(&format!("{name}-q"), routine, a as u32, b as u32, Result32::Value);
            let r = common::long_call(&format!("{name}-r"), routine, a as u32, b as u32, Result32::Remainder);
            assert_eq!((q as i32, r as i32), (a / b, a % b), "{routine} {a} / {b}");
        }
    }
}

/// The unsigned pair routines leave the remainder in rc4..rc7 as well.
#[test]
fn the_unsigned_pair_routines_leave_the_remainder_in_rc4() {
    for routine in ["__udivmodsi4", "__udivmodsi4_fast", "__udivmodsi4_fast_small", "__udivmodsi4_fast_lean", "__udivmodsi4_fast_byte"] {
        for (a, b) in [(4_000_000_000u32, 7u32), (4_000_000_000, 3_000_000_000), (u32::MAX, 255)] {
            if routine.ends_with("_byte") && !(1..=255).contains(&b) {
                continue;
            }
            let name = format!("{routine}-{a}-{b}");
            let q = common::long_call(&format!("{name}-q"), routine, a, b, Result32::Value);
            let r = common::long_call(&format!("{name}-r"), routine, a, b, Result32::Remainder);
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

/// The IR of a function that divides each `(a, b)` of `cases` in `bits` bits,
/// the remainder first or the quotient first, and returns 1 where a result
/// differs from the one worked out here (the operands are loaded from globals,
/// volatile: nothing folds).
fn probe(
    bits: u32,
    signed: bool,
    remainder_first: bool,
    cases: &[(i64, i64)],
) -> String {
    let ir = |value: i64| match bits {
        16 => i64::from(value as u16 as i16),
        _ => i64::from(value as u32 as i32),
    };
    let (div, rem) = if signed { ("sdiv", "srem") } else { ("udiv", "urem") };
    let (mut globals, mut body) = (String::new(), String::new());
    for (n, &(a, b)) in cases.iter().enumerate() {
        globals += &format!("@a{n} = global i{bits} {}\n@b{n} = global i{bits} {}\n", ir(a), ir(b));
        body += &format!("  %x{n} = load volatile i{bits}, ptr @a{n}\n  %y{n} = load volatile i{bits}, ptr @b{n}\n");
        let make = |op: &str, name: &str| format!("  %{name}{n} = {op} i{bits} %x{n}, %y{n}\n");
        body += &if remainder_first { make(rem, "r") + &make(div, "q") } else { make(div, "q") + &make(rem, "r") };
        body += &format!(
            "  %qe{n} = icmp ne i{bits} %q{n}, {}\n  %re{n} = icmp ne i{bits} %r{n}, {}\n  %e{n} = or i1 %qe{n}, %re{n}\n  %w{n} = zext i1 %e{n} to i8\n",
            ir(a / b),
            ir(a % b)
        );
        body += &if n == 0 {
            format!("  %acc{n} = or i8 %w{n}, 0\n")
        } else {
            format!("  %acc{n} = or i8 %w{n}, %acc{}\n", n - 1)
        };
    }
    format!("{globals}define i8 @probe() {{\n{body}  ret i8 %acc{}\n}}\n", cases.len() - 1)
}

/// The compiled pair gives the same quotient and remainder as the two
/// operations, the remainder written first or the quotient first (the other
/// order is a different path of the pairing: the first call's results go to the
/// second instruction's words): 16 and 32 bits, both signs, negative operands,
/// 1 and -1, the extremes, a divisor of a byte and a wider one, at -O2 and -Os
/// (the compact routines).
#[test]
fn a_compiled_pair_gives_the_quotient_and_the_remainder_in_either_order() {
    let signed32: &[(i64, i64)] =
        &[(-100000, 7), (-7, -2), (7, -2), (i64::from(i32::MIN), 3), (i64::from(i32::MAX), -255), (123456789, 70000)];
    let unsigned32: &[(i64, i64)] =
        &[(4_000_000_000, 7), (4_000_000_000, 3_000_000_000), (4_294_967_295, 255), (100, 100)];
    let signed16: &[(i64, i64)] = &[(-1000, 7), (7, -2), (-1000, -300), (-32768, 3), (32767, -1), (30000, 300)];
    let unsigned16: &[(i64, i64)] = &[(50000, 7), (60000, 40000), (65535, 255), (40000, 300)];
    for level in [Level::O2, Level::Os] {
        for (bits, signed, cases) in
            [(32, true, signed32), (32, false, unsigned32), (16, true, signed16), (16, false, unsigned16)]
        {
            for remainder_first in [false, true] {
                let ir = format!(
                    "target datalayout = \"{}\"\n{}",
                    datalayout(),
                    probe(bits, signed, remainder_first, cases)
                );
                // the calls are counted in what the backend wrote: outlining
                // moves some into a body
                let mut request = Request::new(level);
                request.passes.push("-fno-outline".to_owned());
                let text = assembly(&ir, &request).unwrap_or_else(|e| panic!("{e}"));
                let code: String = text
                    .lines()
                    .filter(|l| !l.starts_with("rc") || !l.contains(" = $"))
                    .map(|l| format!("{l}\n"))
                    .collect();
                let divides = text
                    .lines()
                    .filter(|l| {
                        [
                            "jsr __udivsi3",
                            "jsr __divsi3",
                            "jsr __divmodsi4",
                            "jsr __udivmodsi4",
                            "jsr __udivhi3",
                            "jsr __divhi3",
                            "jsr __divmodhi4",
                            "jsr __umodsi3",
                            "jsr __modsi3",
                            "jsr __umodhi3",
                            "jsr __modhi3",
                            "jsr __mulsi3",
                            "jsr __mulhi3",
                        ]
                        .iter()
                        .any(|c| l.trim().starts_with(c))
                    })
                    .count();
                let what = format!("{level:?} {bits} bits signed {signed} remainder first {remainder_first}");
                let calls: Vec<&str> = text.lines().filter(|l| l.contains("jsr")).collect();
                assert_eq!(divides, cases.len(), "{what}: one divide a pair: {calls:?}");
                let name = format!("divrem-probe-{level:?}-{bits}-{signed}-{remainder_first}");
                assert_eq!(common::run_with(&name, "  jsr probe\n", &code), 0, "{what}: a result is wrong");
            }
        }
    }
}
