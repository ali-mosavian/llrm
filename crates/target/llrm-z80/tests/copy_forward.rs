//! A byte copy read again is read from its source, and a step by two is two
//! `dec`s.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

fn fib() -> String {
    text(include_str!("data/fib_recursive.ll"), "main", 0, &Request::new(Level::Os)).expect("compiles")
}

/// `n < 2` (signed, a constant whose low byte is 2) copied `l` to `c` and read
/// `c`: `ld c, l ... ld a, c`, 3 B and 7 T on every call of fib -Os (the bar's
/// compare is 3 B shorter).
#[test]
fn a_signed_compare_reads_the_low_byte_from_where_it_is() {
    let asm = fib();
    assert!(!asm.contains("ld a, c"), "{asm}");
}

/// `sext i16 to i32` of a result in `de` built the sign in `a`, copied it to
/// `b` and `c`, and copied those to `d` and `e`: 2 B more than `ld e, a; ld d,
/// a` (fib -Os: 57 B against SDCC's 54).
#[test]
fn a_sign_byte_is_not_copied_through_a_pair_that_nothing_reads() {
    let asm = fib();
    assert!(!asm.contains("ld c, a"), "{asm}");
}

/// `n - 2` after a call was `ld hl, #65534; pop bc; add hl, bc` (21 T for what
/// two `dec hl` do in 12): the constant sat in `hl` and the operand in the
/// register, the add's other order (fib -Os: 2.70M clocks, SDCC 2.58M).
#[test]
fn a_constant_added_from_the_other_side_is_a_step() {
    let asm = fib();
    assert!(!asm.contains("65534"), "{asm}");
}
