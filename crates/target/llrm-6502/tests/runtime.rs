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

/// A multiplier of 0, 1 or 255 and the full eight bits: the loop ends when the
/// multiplier is spent.
#[test]
fn multiply_bytes_ends_with_the_multiplier() {
    for (a, b, want) in [(200, 0, 0), (0, 200, 0), (200, 1, 200), (1, 255, 255), (255, 1, 255), (3, 128, 128)] {
        assert_eq!(byte(&format!("mulqi-end-{a}-{b}"), &two_bytes(a, b), "__mulqi3"), want, "{a} * {b}");
    }
}

/// `7 * 1` ran the eight trips of a fixed count (`dex; bne`): a multiplier of 1
/// is one trip, the loop ends on the flags `lsr` leaves.
#[test]
fn multiply_bytes_with_a_small_multiplier_is_one_trip() {
    let spent = routine_clocks("mulqi-small", &two_bytes(7, 1));
    assert!(spent <= 50, "the routine and its jsr took {spent} clocks");
}

/// An early exit read from `ldx rc3` costs a clock a trip over `dex`: `0xB3 *
/// i` for sixteen `i` ran 96 clocks over the counter loop (`huffman_decode`).
/// The multiplier with bit 7 set takes eight trips and must not be slower.
#[test]
fn multiply_bytes_by_a_full_width_multiplier_is_no_slower_than_a_counter() {
    let spent = routine_clocks("mulqi-wide", &two_bytes(3, 0xB3));
    assert!(spent <= WIDE_MULTIPLIER_COUNTER_LOOP, "the routine and its jsr took {spent} clocks");
}

/// The counter loop's clocks for `3 * 0xB3`, `jsr` and `rts` included.
const WIDE_MULTIPLIER_COUNTER_LOOP: i64 = 179;

fn routine_clocks(
    name: &str,
    setup: &str,
) -> i64 {
    let with = common::cycles_with(name, &format!("{setup}  jsr __mulqi3\n"), "");
    let without = common::cycles_with(&format!("{name}-none"), setup, "");
    with - without
}

/// The full product of two bytes, A low and X high: 255 * 255 = 65025 = 0xFE01,
/// 200 * 100 = 0x4E20, 0 either side is 0.
#[test]
fn widening_multiply_of_bytes_gives_the_full_product() {
    for (a, b, want) in [(255, 255, 0xFE01), (200, 100, 0x4E20), (0, 9, 0), (9, 0, 0), (1, 255, 255), (128, 2, 256)] {
        assert_eq!(word(&format!("umulqihi-{a}-{b}"), &two_bytes(a, b), "__umulqihi3"), want, "{a} * {b}");
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

/// The divide leaves the remainder behind for a caller that wants both: a byte
/// divide in `rc6` (the quotient in A), a word divide in `rc6:rc7` (the
/// quotient in A:X). 200 / 7 = 28 r 4; 50000 / 7 = 7142 r 6.
#[test]
fn division_leaves_the_remainder_in_rc6_and_rc7() {
    let setup = two_bytes(200, 7);
    assert_eq!(common::run_with_runtime("udivmodqi-q", &format!("{setup}  jsr __udivmodqi4\n")), 28);
    assert_eq!(common::run_with_runtime("udivmodqi-r", &format!("{setup}  jsr __udivmodqi4\n  lda rc6\n")), 4);
    let setup = two_words(50000, 7);
    assert_eq!(common::run_with_runtime("udivhi-r-lo", &format!("{setup}  jsr __udivhi3\n  lda rc6\n")), 6);
    assert_eq!(common::run_with_runtime("udivhi-r-hi", &format!("{setup}  jsr __udivhi3\n  lda rc7\n")), 0);
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

/// The compact word divide (`__udivhi3c`, `__umodhi3c`: the bit loop alone,
/// picked at -Os) agrees with the fast one for every case above, divisors of a
/// byte and under, a divisor of 0x8000 and over (the remainder carries out of
/// 16 bits), and zero (the all-ones quotient, the dividend as remainder).
#[test]
fn the_compact_word_division_agrees_with_the_fast_one() {
    for (a, b, quotient, remainder) in [
        (50000, 7, 7142, 6),
        (60000, 40000, 1, 20000),
        (65535, 256, 255, 255),
        (12345, 10, 1234, 5),
        (65535, 1, 65535, 0),
        (65535, 255, 257, 0),
        (65535, 129, 508, 3),
        (255, 255, 1, 0),
        (0, 9, 0, 0),
        (65535, 32768, 1, 32767),
        (65535, 65535, 1, 0),
        (40000, 0, 65535, 40000),
        (7, 2, 3, 1),
        (999, 1000, 0, 999),
        (1000, 999, 1, 1),
        (32768, 32768, 1, 0),
        (65535, 2, 32767, 1),
        (65535, 16385, 3, 16380),
    ] {
        let n = format!("c{a}-{b}");
        assert_eq!(word(&format!("udivhic-{n}"), &two_words(a, b), "__udivhi3c"), quotient, "{a} / {b}");
        assert_eq!(word(&format!("umodhic-{n}"), &two_words(a, b), "__umodhi3c"), remainder, "{a} % {b}");
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

/// The compact signed divide (`__divhi3c`, `__modhi3c`: the same sign handling
/// over the bit loop alone) gives the fast routines' answers, for divisors
/// under and over 256: -1000 / 7 = -142 r -6, 7 / -2 = -3 r 1, -1000 / -300 = 3
/// r -100, 30000 / 300 = 100 r 0.
#[test]
fn compact_signed_word_division_matches_the_fast_routines() {
    for (a, b, quotient, remainder) in
        [(64536u16, 7u16, 65394i32, 65530i32), (7, 65534, 65533, 1), (64536, 65236, 3, 65436), (30000, 300, 100, 0)]
    {
        let n = format!("{a}-{b}");
        assert_eq!(word(&format!("divhic-{n}"), &two_words(a, b), "__divhi3c"), quotient, "{a} / {b} compact");
        assert_eq!(word(&format!("modhic-{n}"), &two_words(a, b), "__modhi3c"), remainder, "{a} % {b} compact");
        assert_eq!(word(&format!("divhif-{n}"), &two_words(a, b), "__divhi3"), quotient, "{a} / {b} fast");
        assert_eq!(word(&format!("modhif-{n}"), &two_words(a, b), "__modhi3"), remainder, "{a} % {b} fast");
    }
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

/// The multiplier's bytes above its last set one are not walked: the carries
/// of the early exit sit between bytes, so a multiplier with zero bytes below,
/// between and above its set bytes is tried, and so are 0 and 2^31.
#[test]
fn multiply_longs_stops_when_the_multiplier_is_spent() {
    for (a, b) in [
        (123_456_789u32, 0u32),
        (0, 99),
        (7, 0x100),
        (7, 0x0100_0000),
        (0x0102_0304, 0x0001_0001),
        (0x0102_0304, 0x8000_0000),
        (0x0102_0304, 0x8000_0001),
        (3, 0x00FF_FF00),
    ] {
        for routine in ["__mulsi3", "__mulsi3_fast"] {
            assert_eq!(
                long(&format!("mulsi-exit-{a}-{b}{routine}"), routine, a, b),
                a.wrapping_mul(b),
                "{routine}: {a} * {b}"
            );
        }
    }
}

/// The 16 x 16 -> 32 products: unsigned 65535 * 65535 = 0xFFFE0001, signed
/// -32768 * -32768 = 0x40000000, -3 * 7 = -21, 0x7FFF * 0x8001 (= -32767)
/// negative; a multiplier under 256 ends after its first byte (0xFFFF * 0xFF,
/// 0x8000 * 0x80). The signed routine takes the unsigned product less the other
/// operand from the high word for each negative operand.
#[test]
fn widening_multiplies_give_the_32_bit_product() {
    for (a, b) in [
        (0u32, 0u32),
        (1, 0xFFFF),
        (0xFFFF, 0xFFFF),
        (0x8000, 0x8000),
        (0x7FFF, 0x8001),
        (0xFFFD, 7),
        (0xFFFF, 0xFF),
        (0x1234, 0x0100),
        (0x8000, 0x0080),
    ] {
        assert_eq!(long(&format!("umulhisi-{a}-{b}"), "__umulhisi3", a, b), a * b, "{a} * {b}");
        let signed = i32::from(a as u16 as i16) * i32::from(b as u16 as i16);
        assert_eq!(long(&format!("mulhisi-{a}-{b}"), "__mulhisi3", a, b), signed as u32, "{a} * {b} signed");
    }
}

/// A multiplier under 256 is added for each set bit and the walk ends with its
/// last (fcmp_all: 384 calls of `(F) * (k + 1)`, 414 clocks each): the carries
/// run through all three product bytes (0xFFFF * 0xFF), and a multiplier of 3
/// costs 194 clocks, not the 419 of 16 bits. The signed entry takes the byte
/// from the high word of a negative multiplicand.
#[test]
fn a_byte_multiplier_ends_with_its_last_set_bit() {
    for (a, b) in [(0xFFFFu32, 0xFFu32), (0xABCD, 0x55), (0x1234, 0)] {
        assert_eq!(long(&format!("umulbyte-{a}-{b}"), "__umulhisi3_byte", a, b), a * b, "{a} * {b}");
    }
    for (a, b) in [(0xFFFFu32, 0xFFu32), (0xABCD, 0x55), (0x7FFF, 0x81)] {
        let signed = i32::from(a as u16 as i16) * b as i32;
        assert_eq!(long(&format!("mulbyte-{a}-{b}"), "__mulhisi3_byte", a, b), signed as u32, "{a} * {b} signed");
    }
    let setup = "  lda #0\n  sta __sa+1\n  sta __sb+1\n  lda #100\n  sta __sa\n  lda #3\n  sta __sb\n";
    let with = common::cycles_with("umulbyte-clocks", &format!("{setup}  jsr __umulhisi3_byte\n"), "");
    let without = common::cycles_with("umulbyte-clocks-none", setup, "");
    assert!(with - without < 250, "{} clocks", with - without);
}

/// `x * 15` walked all 32 bits of the multiplier: 2054 clocks, 10x gcc's loop
/// in shrink_wrapped_frame (`n * i` over a 16 trip loop). The speed variant
/// stops at the last set byte of 15: a third of that.
#[test]
fn multiply_longs_by_a_byte_is_not_a_walk_of_32_bits() {
    let setup = "  lda #0\n  sta __sa+1\n  sta __sa+2\n  sta __sa+3\n  sta __sb+1\n  sta __sb+2\n  sta __sb+3\n  lda #100\n  sta __sa\n  lda #15\n  sta __sb\n";
    let with = common::cycles_with("mulsi-clocks", &format!("{setup}  jsr __mulsi3_fast\n"), "");
    let without = common::cycles_with("mulsi-clocks-none", setup, "");
    assert!(with - without < 1000, "{} clocks", with - without);
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

/// The speed variants move whole bytes while the count is 8 or more, then
/// shift the rest bit by bit. Every count from 0 to 40 against the shift a
/// 32-bit integer does (a count of 32 or more empties the register, or fills it
/// with the sign).
#[test]
fn the_fast_long_shifts_agree_with_the_bit_loops_for_every_count() {
    let value = 0x8123_45E7u32;
    for n in [0u32, 1, 7, 8, 9, 15, 16, 17, 23, 24, 25, 31, 32, 33, 40] {
        let shl = if n < 32 { value << n } else { 0 };
        let shr = if n < 32 { value >> n } else { 0 };
        let sar = ((value as i32) >> n.min(31)) as u32;
        assert_eq!(long(&format!("fshl-{n}"), "__ashlsi3_fast", value, n), shl, "{value:#x} << {n}");
        assert_eq!(long(&format!("fshr-{n}"), "__lshrsi3_fast", value, n), shr, "{value:#x} >> {n}");
        assert_eq!(long(&format!("fsar-{n}"), "__ashrsi3_fast", value, n), sar, "{value:#x} >>s {n}");
    }
}

/// `__ashlsi3` shifted one bit per trip of a loop of 25 clocks: a shift by 21,
/// the average one of f32 add (`__ashlsi3` was 18% of it), took about 530
/// clocks. The speed variant is two byte moves and five bits, about 260; the
/// bound is 300, since a page cross anywhere in `mos_rt.s` moves it by a clock
/// (inserting 29 lines of unrelated routines made 259 into 260 and failed
/// `< 260`), and the bit loop it must beat is at 530.
#[test]
fn a_long_shift_by_21_takes_whole_byte_steps() {
    let empty = common::cycles_with("fshl-empty", "", "");
    let setup = "  lda #1\n  sta __sa\n  lda #21\n  sta __sb\n";
    let setup_only = common::cycles_with("fshl-setup", setup, "") - empty;
    let fast = common::cycles_with("fshl-21", &format!("{setup}  jsr __ashlsi3_fast\n"), "") - empty - setup_only;
    assert!(fast < 300, "1 << 21 took {fast} clocks");
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

/// A 768-byte buffer in BSS, filled with a pattern that differs in each page.
const BUFFER: &str = ".segment \"BSS\"\nbuf: .res 768\n.segment \"CODE\"\n";

/// Pattern byte `i`: the index low byte, xor $A5 on page 1, xor $A5 $3C on page
/// 2.
fn pattern(i: usize) -> u8 {
    let low = i as u8;
    match i >> 8 {
        0 => low,
        1 => low ^ 0xA5,
        _ => low ^ 0xA5 ^ 0x3C,
    }
}

/// Fills `buf` with the pattern, runs `call` and returns a checksum of all 768
/// bytes: rotated left once and xored with each byte in turn, so a byte in the
/// wrong place changes it.
fn memory_checksum(
    name: &str,
    call: &str,
) -> i32 {
    let fill =
        "  ldx #0\n@f: txa\n  sta buf,x\n  eor #$A5\n  sta buf+256,x\n  eor #$3C\n  sta buf+512,x\n  inx\n  bne @f\n";
    let mut sum = String::from("  lda #0\n  sta rc8\n");
    for page in 0..3 {
        sum += &format!(
            "  ldx #0\n@s{page}: lda rc8\n  asl a\n  adc #0\n  eor buf+{},x\n  sta rc8\n  inx\n  bne @s{page}\n",
            page * 256
        );
    }
    sum += "  lda rc8\n";
    let main = format!("{fill}{call}{sum}");
    common::run_with(name, &main, BUFFER)
}

/// The checksum of the pattern after `edit`, worked out on the host.
fn expected(edit: impl FnOnce(&mut [u8])) -> i32 {
    let mut memory: Vec<u8> = (0..768).map(pattern).collect();
    edit(&mut memory);
    i32::from(memory.iter().fold(0u8, |sum, byte| sum.rotate_left(1) ^ byte))
}

/// `routine(buf + dst, buf + src, n)` for memcpy and memmove, `routine(buf +
/// dst, c, n)` for memset (`c` in A, `n` in rs2).
fn mem_call(
    routine: &str,
    dst: usize,
    second: &str,
    n: usize,
) -> String {
    let (low, high) = if routine == "memset" { (4, 5) } else { (6, 7) };
    format!(
        "  lda #<(buf+{dst})\n  sta rc2\n  lda #>(buf+{dst})\n  sta rc3\n  lda #{}\n  sta rc{low}\n  lda #{}\n  sta rc{high}\n{second}  jsr {routine}\n",
        n & 0xFF,
        n >> 8
    )
}

fn source(src: usize) -> String {
    format!("  lda #<(buf+{src})\n  sta rc4\n  lda #>(buf+{src})\n  sta rc5\n")
}

/// Whole pages, a partial page, one byte and none: a fill of n bytes writes
/// exactly n, each the byte given.
#[test]
fn memset_fills_exactly_n_bytes() {
    for (dst, n) in [(10, 0), (10, 1), (10, 255), (10, 256), (10, 257), (10, 300), (0, 768)] {
        let call = mem_call("memset", dst, "  lda #$EE\n", n);

        let got = memory_checksum(&format!("memset-{dst}-{n}"), &call);

        assert_eq!(got, expected(|m| m[dst..dst + n].fill(0xEE)), "memset(buf+{dst}, $EE, {n})");
    }
}

#[test]
fn memcpy_copies_exactly_n_bytes() {
    for n in [0, 1, 255, 256, 257, 300] {
        let call = mem_call("memcpy", 400, &source(10), n);

        let got = memory_checksum(&format!("memcpy-{n}"), &call);

        assert_eq!(got, expected(|m| m.copy_within(10..10 + n, 400)), "memcpy(buf+400, buf+10, {n})");
    }
}

/// Overlap in each direction, equal pointers, disjoint ranges, and page-sized
/// counts.
#[test]
fn memmove_handles_overlap_in_both_directions() {
    for (dst, src, n) in [
        (20, 10, 300),
        (10, 20, 300),
        (10, 10, 300),
        (310, 10, 300),
        (11, 10, 256),
        (11, 10, 512),
        (11, 10, 1),
        (10, 11, 512),
        (11, 10, 0),
    ] {
        let call = mem_call("memmove", dst, &source(src), n);

        let got = memory_checksum(&format!("memmove-{dst}-{src}-{n}"), &call);

        assert_eq!(got, expected(|m| m.copy_within(src..src + n, dst)), "memmove(buf+{dst}, buf+{src}, {n})");
    }
}

/// Each returns its destination in A:X, as the C library does: the difference
/// from `buf + 10` is zero.
#[test]
fn the_memory_routines_return_the_destination() {
    for routine in ["memset", "memcpy", "memmove"] {
        let second = if routine == "memset" { "  lda #1\n".to_owned() } else { source(400) };
        let call = mem_call(routine, 10, &second, 300);
        let check = "  sec\n  sbc #<(buf+10)\n  sta rc8\n  txa\n  sbc #>(buf+10)\n  ora rc8\n";

        let got = common::run_with(&format!("{routine}-returns"), &format!("{call}{check}"), BUFFER);

        assert_eq!(got, 0, "{routine} did not return dst");
    }
}

/// The cases of the 32-bit division tests: edges, then a fixed pseudo-random
/// walk over dividend and divisor widths (a divisor below 256 and one that
/// skips dividend bytes take other paths in the speed routines).
fn division_cases() -> Vec<(u32, u32)> {
    let mut cases = vec![
        (0, 0),
        (0, 5),
        (5, 0),
        (1, 1),
        (u32::MAX, 1),
        (u32::MAX, u32::MAX),
        (0x8000_0000, u32::MAX),
        (0x8000_0000, 1),
        (3_000_000, 100_003),
        (100_003, 100_003),
        (100_002, 100_003),
        (255, 1),
        (256, 255),
        (65_535, 257),
        (0x0100_0000, 3),
        (0x7FFF_FFFF, 0x10000),
    ];
    for d in [2u32, 3, 5, 7, 10, 100, 127, 128, 129, 254, 255, 256, 257] {
        for a in [0, 1, d - 1, d, d + 1, 255, 256, 65_535, 65_536, 0x7FFF_FFFF, 0x8000_0000, u32::MAX, 123_456_789] {
            cases.push((a, d));
        }
    }
    let mut state = 0x2545_F491u32;
    let mut next = move || {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        state
    };
    for _ in 0..60 {
        let wa = [1, 7, 8, 9, 15, 16, 17, 24, 25, 31, 32][(next() >> 8) as usize % 11];
        let wb = [1, 3, 8, 9, 16, 17, 24, 32][(next() >> 8) as usize % 8];
        let (a, b) = (next(), next());
        cases.push((a >> (32 - wa), b >> (32 - wb)));
    }
    cases
}

/// Runs `routine` over `cases`, `want` the result each should leave in
/// `__sa`: the number of cases it got wrong, 255 at most.
fn division_mismatches(
    name: &str,
    routine: &str,
    cases: &[(u32, u32)],
    want: impl Fn(u32, u32) -> u32,
) -> i32 {
    assert!(cases.len() < 256);
    let mut table = String::from(".segment \"RODATA\"\nthe_cases:\n");
    for &(a, b) in cases {
        table += &format!("  .dword {a}, {b}, {}\n", want(a, b));
    }
    table += ".segment \"CODE\"\n";
    let mut main = String::from(
        "  lda #<the_cases\n  sta rc2\n  lda #>the_cases\n  sta rc3\n  lda #0\n  sta rc4\n  sta rc5\nthe_next:\n",
    );
    for (cell, from) in [("__sa", 0), ("__sb", 4)] {
        for at in 0..4 {
            main += &format!("  ldy #{}\n  lda (rc2),y\n  sta {cell}+{at}\n", from + at);
        }
    }
    main += &format!("  jsr {routine}\n");
    for at in 0..4 {
        main += &format!("  ldy #{}\n  lda (rc2),y\n  cmp __sa+{at}\n  bne the_wrong\n", 8 + at);
    }
    main += "  jmp the_right\nthe_wrong:\n  inc rc4\nthe_right:\n  clc\n  lda rc2\n  adc #12\n  sta rc2\n  bcc the_same_page\n  inc rc3\nthe_same_page:\n  inc rc5\n";
    main += &format!("  lda rc5\n  cmp #{}\n  beq the_end\n  jmp the_next\nthe_end:\n  lda rc4\n", cases.len());
    common::run_with(name, &main, &table)
}

/// The speed variants of the 32-bit division and the small ones give the
/// same results: quotient and remainder, unsigned and signed, a zero divisor
/// (all ones over the dividend, as the walk of 32 trips left it), INT_MIN / -1
/// and the dividend bytes that are skipped.
#[test]
fn long_division_routines_agree_with_the_machine_on_edges_and_a_walk() {
    let cases = division_cases();
    let unsigned = |a: u32, b: u32| if b == 0 { (u32::MAX, a) } else { (a / b, a % b) };
    let signed = |a: u32, b: u32| {
        let (x, y) = (a as i32, b as i32);
        if y == 0 {
            (if x < 0 { 1 } else { u32::MAX }, a)
        } else {
            (x.wrapping_div(y) as u32, x.wrapping_rem(y) as u32)
        }
    };
    for suffix in ["", "_fast"] {
        for (routine, quotient, which) in
            [("udivsi3", true, 0), ("umodsi3", false, 0), ("divsi3", true, 1), ("modsi3", false, 1)]
        {
            let name = format!("__{routine}{suffix}");
            let wrong = division_mismatches(&format!("div-{routine}{suffix}"), &name, &cases, |a, b| {
                let (q, r) = if which == 0 { unsigned(a, b) } else { signed(a, b) };
                if quotient { q } else { r }
            });
            assert_eq!(wrong, 0, "{name}: {wrong} of {} cases wrong", cases.len());
        }
    }
}

/// `x % 100003` on a 6-digit x walked all 32 bits: 4192 clocks a call, 1.8x
/// gcc's, and loop_interchange -O2 spent 13.9M of its clocks on 3072 of
/// them. The speed variant takes the dividend's bytes into the remainder
/// while it stays below the divisor.
#[test]
fn a_long_modulo_by_a_17_bit_divisor_does_not_walk_32_trips() {
    let setup = "  lda #$C0\n  sta __sa\n  lda #$C6\n  sta __sa+1\n  lda #$2D\n  sta __sa+2\n  lda #0\n  sta __sa+3\n  lda #$A3\n  sta __sb\n  lda #$86\n  sta __sb+1\n  lda #1\n  sta __sb+2\n  lda #0\n  sta __sb+3\n";
    let with = common::cycles_with("modsi-clocks", &format!("{setup}  jsr __modsi3_fast\n"), "");
    let without = common::cycles_with("modsi-clocks-none", setup, "");
    assert!(with - without < 1800, "{} clocks", with - without);
}

/// `x % 100003` on dividends like loop_interchange's checksum (below 31 *
/// 100003 + 65535) cost 931 clocks a call against llvm-mos's 769: the
/// remainder was built beside a quotient nothing reads (four `rol` a trip) and
/// copied back. The speed remainder keeps the dividend in place and builds no
/// quotient. Seven calls took 6065 clocks before, 4639 now.
#[test]
fn a_long_modulo_builds_no_quotient_and_copies_no_remainder() {
    let load = |at: &str, v: u32| {
        (0..4).map(|k| format!("  lda #{}\n  sta {at}+{k}\n", (v >> (8 * k)) & 255)).collect::<String>()
    };
    let dividends = [3_100_000u32, 1_550_001, 700_000, 250_000, 100_003, 100_002, 4_000_000_000];
    let mut total = 0;
    for a in dividends {
        let setup = format!("{}{}", load("__sa", a), load("__sb", 100_003));
        let name = format!("mod-q-{a}");
        let with = common::cycles_with(&format!("{name}-with"), &format!("{setup}  jsr __umodsi3_fast\n"), "");
        total += with - common::cycles_with(&format!("{name}-none"), &setup, "");
        assert_eq!(long(&name, "__umodsi3_fast", a, 100_003), a % 100_003, "{a} % 100003");
    }
    assert!(total < 5200, "{total} clocks for {} calls", dividends.len());
}

/// `(x * 3 + p) % 1000003` ran 8 trips of the bit loop for a 22-bit dividend
/// and a 20-bit divisor whatever the quotient (1189 clocks a call, 2.2x
/// llvm-mos's loop around it): it started from whole bytes. The pin's
/// compiler-rt `__udivXi3` runs for the quotient's bit length.
#[test]
fn a_long_modulo_runs_for_the_quotients_bits_not_the_dividends_bytes() {
    let clocks = |name: &str, a: u32, b: u32| {
        let load = |at: &str, v: u32| {
            (0..4).map(|k| format!("  lda #{}\n  sta {at}+{k}\n", (v >> (8 * k)) & 255)).collect::<String>()
        };
        let setup = format!("{}{}", load("__sa", a), load("__sb", b));
        let with = common::cycles_with(&format!("{name}-with"), &format!("{setup}  jsr __modsi3_fast\n"), "");
        with - common::cycles_with(&format!("{name}-none"), &setup, "")
    };
    let two_bits = clocks("modsi-q2", 3_000_009, 1_000_003);
    let none = clocks("modsi-q0", 999_999, 1_000_003);
    assert!(two_bits < 800, "3000009 % 1000003: {two_bits} clocks");
    assert!(none < 350, "999999 % 1000003: {none} clocks");
}

/// Every program with a division linked the wide path (+141 B over the old
/// routine), `a % 5` included: its divisor is a byte and runs the byte loop. A
/// byte-divisor entry links the byte loop only.
#[test]
fn a_byte_divisor_entry_links_the_byte_loop_and_not_the_wide_path() {
    for routine in ["__udivsi3_fast_byte", "__umodsi3_fast_byte", "__divsi3_fast_byte", "__modsi3_fast_byte"] {
        let text = common::pipeline::reachable_runtime(&format!("  jsr {routine}\n"));
        assert!(text.contains("udivmod32_fast_byte:"), "{routine} does not reach the byte loop");
        assert!(!text.contains("udivmod32_wide:"), "{routine} links the wide path");
    }
}

/// The byte entries agree with the dividend's and divisor's arithmetic (a
/// negative dividend included) for divisors 1..=255.
#[test]
fn the_byte_divisor_entries_divide_like_the_general_ones() {
    let cases = [(3_000_009u32, 7u32), (0xFFFF_FFFF, 255), (100, 100), (99, 100), (0x8000_0001, 3), (0, 9), (1, 1)];
    for (routine, general) in [
        ("__udivsi3_fast_byte", "__udivsi3_fast"),
        ("__umodsi3_fast_byte", "__umodsi3_fast"),
        ("__divsi3_fast_byte", "__divsi3_fast"),
        ("__modsi3_fast_byte", "__modsi3_fast"),
    ] {
        for (a, b) in cases {
            let name = format!("{routine}-{a}-{b}");
            assert_eq!(long(&name, routine, a, b), long(&format!("g{name}"), general, a, b), "{routine} {a} {b}");
        }
    }
}

/// The routines a cold module calls are the first speed variant: they link no
/// pin-structure wide path (141 B more) and divide like the hot ones.
#[test]
fn the_small_speed_division_links_no_wide_path_and_divides_like_the_hot_one() {
    let cases =
        [(3_000_009u32, 1_000_003u32), (0xFFFF_FFFF, 255), (40_000, 0xFFFF_FFFD), (70_000, 300), (99, 100), (7, 0)];
    for (routine, hot) in [
        ("__udivsi3_fast_small", "__udivsi3_fast"),
        ("__umodsi3_fast_small", "__umodsi3_fast"),
        ("__divsi3_fast_small", "__divsi3_fast"),
        ("__modsi3_fast_small", "__modsi3_fast"),
    ] {
        let text = common::pipeline::reachable_runtime(&format!("  jsr {routine}\n"));
        assert!(text.contains("udivmod32_fast_small:") && !text.contains("udivmod32_wide:"), "{routine}: {text}");
        let hot_text = common::pipeline::reachable_runtime(&format!("  jsr {hot}\n"));
        let wide = if routine.contains("mod") { "umod32_wide:" } else { "udivmod32_wide:" };
        assert!(hot_text.contains(wide) && !hot_text.contains("_small"), "{hot}: {hot_text}");
        for (a, b) in cases {
            let name = format!("{routine}-{a}-{b}");
            assert_eq!(long(&name, routine, a, b), long(&format!("h{name}"), hot, a, b), "{routine} {a} {b}");
        }
    }
}

/// The hot threshold is derived from what the pin-structure wide path saves a
/// call (`hotdivision::SAVED_CLOCKS`, the mean over these ten shapes; the
/// median is 224, the four shapes with a divisor above 16 bits that the corpus
/// sites hit save 328 on average). This re-measures the same ten shapes: it
/// shows the constant is not stale if the routines change, not that the ten
/// are the call mix of a program.
#[test]
fn the_wide_path_saves_a_call_what_the_hot_threshold_assumes() {
    let shapes = [
        (3_000_009u32, 1_000_003u32),
        (70_000, 300),
        (12_345_678, 257),
        (4_000_000_000, 65_537),
        (2_147_483_647, 1000),
        (100_000, 300),
        (1_000_000, 70_000),
        (99, 100_000),
        (4_000_000_000, 300),
        (16_777_216, 256),
    ];
    let clocks = |routine: &str, a: u32, b: u32| {
        let load = |at: &str, v: u32| {
            (0..4).map(|k| format!("  lda #{}\n  sta {at}+{k}\n", (v >> (8 * k)) & 255)).collect::<String>()
        };
        let setup = format!("{}{}", load("__sa", a), load("__sb", b));
        let with = common::cycles_with(&format!("{routine}-{a}-{b}"), &format!("{setup}  jsr {routine}\n"), "");
        with - common::cycles_with(&format!("none-{routine}-{a}-{b}"), &setup, "")
    };
    let saved: i64 =
        shapes.iter().map(|&(a, b)| clocks("__udivsi3_fast_small", a, b) - clocks("__udivsi3_fast", a, b)).sum();
    let mean = saved as f64 / shapes.len() as f64;
    assert!(mean >= llrm_6502::hotdivision::SAVED_CLOCKS * 0.9, "the wide path saves {mean} clocks a call");
}

/// `hotmultiply::SAVED_CLOCKS`: what `__mulsi3_fast` saves a call, averaged
/// over multipliers of one to four bytes, in sim65 clocks.
#[test]
fn the_fast_multiply_saves_what_hotmultiply_prices() {
    let call = |routine: &str, b: u32| {
        let mut setup = String::from("  lda #0\n  sta __sa+1\n  sta __sa+2\n  sta __sa+3\n  lda #100\n  sta __sa\n");
        for at in 0..4 {
            setup += &format!("  lda #{}\n  sta __sb+{at}\n", (b >> (8 * at)) & 0xFF);
        }
        common::cycles_with(&format!("mulsave-{routine}-{b}"), &format!("{setup}  jsr {routine}\n"), "")
            - common::cycles_with(&format!("mulsave-none-{b}"), &setup, "")
    };
    let shapes = [25u32, 0x1234, 0x0019_660D, 0x9E37_79B9];
    let saved: Vec<i64> = shapes.iter().map(|&b| call("__mulsi3", b) - call("__mulsi3_fast", b)).collect();
    let mean = saved.iter().sum::<i64>() as f64 / saved.len() as f64;
    assert_eq!(mean.round(), llrm_6502::hotmultiply::SAVED_CLOCKS);
}
