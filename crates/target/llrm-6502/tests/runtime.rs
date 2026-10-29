//! The spike runtime (`runtime/mos_rt.s`) assembled with ca65 and run on sim65.
//! Every expected value is worked out by hand (64-bit integer arithmetic on
//! paper), not read from the routines.

mod common;

/// `lda #v; sta reg` for a byte; a word is two of them, low byte first.
fn set(
    register: &str,
    value: u16,
    wide: bool,
) -> String {
    let mut text = format!("  lda #{}\n  sta {register}\n", value & 0xFF);
    if wide {
        let high = format!("rc{}", register[2..].parse::<u32>().unwrap() + 1);
        text += &format!("  lda #{}\n  sta {high}\n", value >> 8);
    }
    text
}

/// The routine's result: A for a byte, A (low) and X (high) for a word.
fn byte(
    name: &str,
    setup: &str,
    routine: &str,
) -> i32 {
    common::run_with_runtime(name, &format!("{setup}  jsr {routine}\n"))
}

fn word(
    name: &str,
    setup: &str,
    routine: &str,
) -> i32 {
    let lo = common::run_with_runtime(&format!("{name}-lo"), &format!("{setup}  jsr {routine}\n"));
    let hi = common::run_with_runtime(&format!("{name}-hi"), &format!("{setup}  jsr {routine}\n  txa\n"));
    hi << 8 | lo
}

fn two_bytes(
    a: u16,
    b: u16,
) -> String {
    set("rc2", a, false) + &set("rc3", b, false)
}

fn two_words(
    a: u16,
    b: u16,
) -> String {
    set("rc2", a, true) + &set("rc4", b, true)
}

/// The low byte of the product: 7 * 6 = 42, 200 * 2 = 400 = 0x190 -> 0x90, 255
/// * 255 = 65025 = 0xFE01 -> 0x01.
#[test]
fn multiply_bytes_gives_the_low_byte() {
    for (a, b, want) in [(7, 6, 42), (200, 2, 0x90), (255, 255, 0x01)] {
        assert_eq!(byte(&format!("mulqi-{a}-{b}"), &two_bytes(a, b), "__mulqi3"), want, "{a} * {b}");
    }
}

/// 300 * 5 = 1500 = 0x05DC; 255 * 257 = 65535; 1000 * 1000 = 1000000 = 15 *
/// 65536 + 16960.
#[test]
fn multiply_words_gives_the_low_word() {
    // 0 either side; 65535 * 65535 = 4294836225 = 65534 * 65536 + 1; 300 * 300
    // = 90000 = 65536 + 24464; a small multiplier stops the loop early, a
    // multiplier with bit 15 set runs all sixteen
    for (a, b, want) in [
        (300, 5, 1500),
        (255, 257, 65535),
        (1000, 1000, 16960),
        (0, 500, 0),
        (500, 0, 0),
        (65535, 65535, 1),
        (300, 300, 24464),
        (1, 40000, 40000),
        (40000, 1, 40000),
        (1234, 56, 3568),
        (56, 1234, 3568),
        (65535, 2, 65534),
        (2, 65535, 65534),
    ] {
        assert_eq!(word(&format!("mulhi-{a}-{b}"), &two_words(a, b), "__mulhi3"), want, "{a} * {b}");
    }
}

/// 200 / 7 = 28 r 4; 255 / 16 = 15 r 15; 200 / 130 = 1 r 70 (a divisor over 127
/// needs the carry out of the shifted remainder); 100 / 0: all ones and the
/// dividend.
#[test]
fn unsigned_byte_division() {
    for (a, b, quotient, remainder) in [(200, 7, 28, 4), (255, 16, 15, 15), (200, 130, 1, 70), (100, 0, 255, 100)] {
        let n = format!("{a}-{b}");
        assert_eq!(byte(&format!("udivqi-{n}"), &two_bytes(a, b), "__udivqi3"), quotient, "{a} / {b}");
        assert_eq!(byte(&format!("umodqi-{n}"), &two_bytes(a, b), "__umodqi3"), remainder, "{a} % {b}");
    }
}

/// 50000 / 7 = 7142 r 6 (7 * 7142 = 49994); 60000 / 40000 = 1 r 20000; 65535 /
/// 256 = 255 r 255.
#[test]
fn unsigned_word_division() {
    for (a, b, quotient, remainder) in [
        (50000, 7, 7142, 6),
        (60000, 40000, 1, 20000),
        (65535, 256, 255, 255),
        // a byte divisor takes the short remainder path; 129 and up carry out
        // of the shift
        (12345, 10, 1234, 5),
        (65535, 1, 65535, 0),
        (65535, 255, 257, 0),
        (40000, 200, 200, 0),
        (65535, 129, 508, 3),
        (65535, 128, 511, 127),
        (255, 255, 1, 0),
        (0, 9, 0, 0),
    ] {
        let n = format!("{a}-{b}");
        assert_eq!(word(&format!("udivhi-{n}"), &two_words(a, b), "__udivhi3"), quotient, "{a} / {b}");
        assert_eq!(word(&format!("umodhi-{n}"), &two_words(a, b), "__umodhi3"), remainder, "{a} % {b}");
    }
}

/// Division truncates toward zero and the remainder has the dividend's sign: -7
/// / 2 = -3 r -1, 7 / -2 = -3 r 1, -7 / -2 = 3 r -1. As bytes: -7 = 249, -3 =
/// 253, -1 = 255, -2 = 254.
#[test]
fn signed_byte_division() {
    for (a, b, quotient, remainder) in [(249, 2, 253, 255), (7, 254, 253, 1), (249, 254, 3, 255)] {
        let n = format!("{a}-{b}");
        assert_eq!(byte(&format!("divqi-{n}"), &two_bytes(a, b), "__divqi3"), quotient, "{a} / {b}");
        assert_eq!(byte(&format!("modqi-{n}"), &two_bytes(a, b), "__modqi3"), remainder, "{a} % {b}");
    }
}

/// -1000 / 7 = -142 r -6; 65536 - 1000 = 64536, 65536 - 142 = 65394, 65536 - 6
/// = 65530.
#[test]
fn signed_word_division() {
    assert_eq!(word("divhi", &two_words(64536, 7), "__divhi3"), 65394);
    assert_eq!(word("modhi", &two_words(64536, 7), "__modhi3"), 65530);
}

/// 0x03 << 4 = 0x30; 0xF0 >> 4 = 0x0F logical, 0xFF arithmetic; a count of 0
/// changes nothing.
#[test]
fn byte_shifts_by_a_variable() {
    assert_eq!(byte("shl8", &two_bytes(0x03, 4), "__ashlqi3"), 0x30);
    assert_eq!(byte("lshr8", &two_bytes(0xF0, 4), "__lshrqi3"), 0x0F);
    assert_eq!(byte("ashr8", &two_bytes(0xF0, 4), "__ashrqi3"), 0xFF);
    assert_eq!(byte("shl8-0", &two_bytes(0x5A, 0), "__ashlqi3"), 0x5A);
}

/// 0x0123 << 8 = 0x2300; 0x8100 >> 7 = 0x0102 logical, 0xFF02 arithmetic;
/// 0x1234 << 0 = 0x1234.
#[test]
fn word_shifts_by_a_variable() {
    let value = |v, k| set("rc2", v, true) + &set("rc4", k, false);
    assert_eq!(word("shl16", &value(0x0123, 8), "__ashlhi3"), 0x2300);
    assert_eq!(word("lshr16", &value(0x8100, 7), "__lshrhi3"), 0x0102);
    assert_eq!(word("ashr16", &value(0x8100, 7), "__ashrhi3"), 0xFF02);
    assert_eq!(word("shl16-0", &value(0x1234, 0), "__ashlhi3"), 0x1234);
}

/// `__sa` and `__sb` loaded with `a` and `b`, `routine` called, `__sa` read
/// back byte by byte.
fn long(
    name: &str,
    routine: &str,
    a: u32,
    b: u32,
) -> u32 {
    let mut setup = String::new();
    for (cell, value) in [("__sa", a), ("__sb", b)] {
        for at in 0..4 {
            setup += &format!("  lda #{}\n  sta {cell}+{at}\n", (value >> (8 * at)) & 0xFF);
        }
    }
    (0..4)
        .map(|at| {
            let main = format!("{setup}  jsr {routine}\n  lda __sa+{at}\n");
            (common::run_with_runtime(&format!("{name}-{at}"), &main) as u32) << (8 * at)
        })
        .sum()
}

/// Low 32 bits of the product: 70000 * 3 = 0x33450; 65535 * 65537 = 0xFFFFFFFF;
/// 100000 * 100000 = 10^10 = 0x2_540BE400, so 0x540BE400.
#[test]
fn multiply_longs_gives_the_low_32_bits() {
    for (a, b, want) in [(70000, 3, 0x33450), (65535, 65537, 0xFFFF_FFFF), (100000, 100000, 0x540B_E400)] {
        assert_eq!(long(&format!("mulsi-{a}-{b}"), "__mulsi3", a, b), want, "{a} * {b}");
    }
}

/// 4000000000 / 7 = 571428571 (0x220F4EDB) r 3; 4000000000 / 3000000000 = 1 r
/// 1000000000 (a divisor above 2^31 needs the carry out of the shifted
/// remainder); 123456789 / 1 = itself r 0.
#[test]
fn unsigned_long_division() {
    for (a, b, quotient, remainder) in [
        (4_000_000_000u32, 7, 0x220F_4EDB, 3),
        (4_000_000_000, 3_000_000_000, 1, 1_000_000_000),
        (123_456_789, 1, 123_456_789, 0),
    ] {
        assert_eq!(long(&format!("udivsi-{a}-{b}"), "__udivsi3", a, b), quotient, "{a} / {b}");
        assert_eq!(long(&format!("umodsi-{a}-{b}"), "__umodsi3", a, b), remainder, "{a} % {b}");
    }
}

/// Truncated toward zero, the remainder with the dividend's sign: -100000 / 7 =
/// -14285 r -5; -7 / -2 = 3 r -1; 7 / -2 = -3 r 1.
#[test]
fn signed_long_division() {
    for (a, b, quotient, remainder) in [(-100000i32, 7i32, -14285i32, -5i32), (-7, -2, 3, -1), (7, -2, -3, 1)] {
        let name = format!("{a}-{b}").replace('-', "m");
        assert_eq!(long(&format!("divsi-{name}"), "__divsi3", a as u32, b as u32), quotient as u32, "{a} / {b}");
        assert_eq!(long(&format!("modsi-{name}"), "__modsi3", a as u32, b as u32), remainder as u32, "{a} % {b}");
    }
}

/// 1 << 31 = 0x80000000; 0x12345678 << 4 = 0x23456780; 0x80000000 >> 31 is 1
/// logical and 0xFFFFFFFF arithmetic; 0x12345678 >> 8 = 0x123456; a count of 0
/// changes nothing.
#[test]
fn long_shifts_by_a_variable() {
    assert_eq!(long("shl-31", "__ashlsi3", 1, 31), 0x8000_0000);
    assert_eq!(long("shl-4", "__ashlsi3", 0x1234_5678, 4), 0x2345_6780);
    assert_eq!(long("lshr-31", "__lshrsi3", 0x8000_0000, 31), 1);
    assert_eq!(long("ashr-31", "__ashrsi3", 0x8000_0000, 31), 0xFFFF_FFFF);
    assert_eq!(long("lshr-8", "__lshrsi3", 0x1234_5678, 8), 0x0012_3456);
    assert_eq!(long("shl-0", "__ashlsi3", 0x1234_5678, 0), 0x1234_5678);
}

/// A word multiply by a small multiplier ran all sixteen iterations: 1234 * 56
/// took about 640 cycles (bench set 2, `mul16`). It stops when the multiplier
/// has no bits left.
#[test]
fn a_small_multiplier_stops_the_word_multiply_early() {
    let empty = common::cycles_with("mulhi-empty", "", "");
    let setup = two_words(1234, 56);
    let cycles = common::cycles_with("mulhi-cycles", &format!("{setup}  jsr __mulhi3\n"), "") - empty;
    let setup_only = common::cycles_with("mulhi-setup", &setup, "") - empty;
    assert!(cycles - setup_only < 330, "1234 * 56 took {} cycles", cycles - setup_only);
}
