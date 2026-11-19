//! The estimate a choice between two builds reads (`expand::price`, `price_emitted`) is the assembled size.

use llrm_z80::compile::effective;
use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::{Line, price};

/// switch_peeled_table, from clang -Oz. The estimate priced `add hl,hl` at 2 B (1 assembled), so the -Os build, whose
/// loop shifts through five of them, came out equal to -Oz at 251 B; -Oz kept 250 B against the 246 B of -Os.
const IR: &str = include_str!("data/oz_best_of_estimate.ll");

/// Each instruction with the bytes sdasz80 gives it. The table had no row for the accumulator forms (`ld a,(de)` read
/// as `ld b,(de)`), `add hl,hl`, `ld (nn),bc` or `xor (hl)`, and priced each missing form at 2 B.
#[test]
fn an_instruction_is_priced_at_its_assembled_bytes() {
    let rows = [
        ("add hl, hl", 1),
        ("ld a, (de)", 1),
        ("ld a, (bc)", 1),
        ("ld (bc), a", 1),
        ("ld (de), a", 1),
        ("xor a, (hl)", 1),
        ("ld (_dst+5), bc", 4),
        ("ld (_sum), de", 4),
        ("ld (_dst+7), a", 3),
        ("ld a, (_src+9)", 3),
        // forms the old key already priced: the accumulator in the table's own spelling
        ("add a, a", 1),
        ("adc a, b", 1),
        ("sbc a, a", 1),
        ("ld b, a", 1),
        ("ld a, b", 1),
        ("ld a, #5", 2),
        ("sub a, #5", 2),
        ("cp a, (hl)", 1),
        ("add a, #5", 2),
    ];
    for (text, bytes) in rows {
        assert_eq!(price(&[Line::Inst(text.into())]).0, bytes, "`{text}`");
    }
}

#[test]
fn oz_keeps_the_os_build_when_that_is_smaller_assembled() {
    let chosen = effective(IR, &Request::new(Level::Oz)).expect("compiles").level;
    assert_eq!(chosen, Level::Os);
}
