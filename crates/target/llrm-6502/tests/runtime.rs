//! The spike runtime (`runtime/mos_rt.s`) assembled with ca65 and run on sim65. Every expected value
//! is worked out by hand (64-bit integer arithmetic on paper), not read from the routines.

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

/// The low byte of the product: 7 * 6 = 42, 200 * 2 = 400 = 0x190 -> 0x90, 255 * 255 = 65025 =
/// 0xFE01 -> 0x01.
#[test]
fn multiply_bytes_gives_the_low_byte() {
    for (a, b, want) in [(7, 6, 42), (200, 2, 0x90), (255, 255, 0x01)] {
        assert_eq!(byte(&format!("mulqi-{a}-{b}"), &two_bytes(a, b), "__mulqi3"), want, "{a} * {b}");
    }
}

/// 300 * 5 = 1500 = 0x05DC; 255 * 257 = 65535; 1000 * 1000 = 1000000 = 15 * 65536 + 16960.
#[test]
fn multiply_words_gives_the_low_word() {
    for (a, b, want) in [(300, 5, 1500), (255, 257, 65535), (1000, 1000, 16960)] {
        assert_eq!(word(&format!("mulhi-{a}-{b}"), &two_words(a, b), "__mulhi3"), want, "{a} * {b}");
    }
}

/// 200 / 7 = 28 r 4; 255 / 16 = 15 r 15; 200 / 130 = 1 r 70 (a divisor over 127 needs the carry out
/// of the shifted remainder); 100 / 0: all ones and the dividend.
#[test]
fn unsigned_byte_division() {
    for (a, b, quotient, remainder) in [(200, 7, 28, 4), (255, 16, 15, 15), (200, 130, 1, 70), (100, 0, 255, 100)] {
        let n = format!("{a}-{b}");
        assert_eq!(byte(&format!("udivqi-{n}"), &two_bytes(a, b), "__udivqi3"), quotient, "{a} / {b}");
        assert_eq!(byte(&format!("umodqi-{n}"), &two_bytes(a, b), "__umodqi3"), remainder, "{a} % {b}");
    }
}

/// 50000 / 7 = 7142 r 6 (7 * 7142 = 49994); 60000 / 40000 = 1 r 20000; 65535 / 256 = 255 r 255.
#[test]
fn unsigned_word_division() {
    for (a, b, quotient, remainder) in [(50000, 7, 7142, 6), (60000, 40000, 1, 20000), (65535, 256, 255, 255)] {
        let n = format!("{a}-{b}");
        assert_eq!(word(&format!("udivhi-{n}"), &two_words(a, b), "__udivhi3"), quotient, "{a} / {b}");
        assert_eq!(word(&format!("umodhi-{n}"), &two_words(a, b), "__umodhi3"), remainder, "{a} % {b}");
    }
}

/// Division truncates toward zero and the remainder has the dividend's sign: -7 / 2 = -3 r -1,
/// 7 / -2 = -3 r 1, -7 / -2 = 3 r -1. As bytes: -7 = 249, -3 = 253, -1 = 255, -2 = 254.
#[test]
fn signed_byte_division() {
    for (a, b, quotient, remainder) in [(249, 2, 253, 255), (7, 254, 253, 1), (249, 254, 3, 255)] {
        let n = format!("{a}-{b}");
        assert_eq!(byte(&format!("divqi-{n}"), &two_bytes(a, b), "__divqi3"), quotient, "{a} / {b}");
        assert_eq!(byte(&format!("modqi-{n}"), &two_bytes(a, b), "__modqi3"), remainder, "{a} % {b}");
    }
}

/// -1000 / 7 = -142 r -6; 65536 - 1000 = 64536, 65536 - 142 = 65394, 65536 - 6 = 65530.
#[test]
fn signed_word_division() {
    assert_eq!(word("divhi", &two_words(64536, 7), "__divhi3"), 65394);
    assert_eq!(word("modhi", &two_words(64536, 7), "__modhi3"), 65530);
}

/// 0x03 << 4 = 0x30; 0xF0 >> 4 = 0x0F logical, 0xFF arithmetic; a count of 0 changes nothing.
#[test]
fn byte_shifts_by_a_variable() {
    assert_eq!(byte("shl8", &two_bytes(0x03, 4), "__ashlqi3"), 0x30);
    assert_eq!(byte("lshr8", &two_bytes(0xF0, 4), "__lshrqi3"), 0x0F);
    assert_eq!(byte("ashr8", &two_bytes(0xF0, 4), "__ashrqi3"), 0xFF);
    assert_eq!(byte("shl8-0", &two_bytes(0x5A, 0), "__ashlqi3"), 0x5A);
}

/// 0x0123 << 8 = 0x2300; 0x8100 >> 7 = 0x0102 logical, 0xFF02 arithmetic; 0x1234 << 0 = 0x1234.
#[test]
fn word_shifts_by_a_variable() {
    let value = |v, k| set("rc2", v, true) + &set("rc4", k, false);
    assert_eq!(word("shl16", &value(0x0123, 8), "__ashlhi3"), 0x2300);
    assert_eq!(word("lshr16", &value(0x8100, 7), "__lshrhi3"), 0x0102);
    assert_eq!(word("ashr16", &value(0x8100, 7), "__ashrhi3"), 0xFF02);
    assert_eq!(word("shl16-0", &value(0x1234, 0), "__ashlhi3"), 0x1234);
}
