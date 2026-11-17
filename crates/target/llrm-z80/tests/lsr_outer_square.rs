//! DEVIATE LU32: an inner loop that starts at `i * i` must not make the outer
//! loop carry a quadratic induction pair.

use llrm_z80::compile::{prepare, text_in};
use llrm_z80::driver::{Level, Request};

/// bench/sieve: `for (m = i * i; m < limit; m += i) composite[m] = 1` inside a
/// loop over `i`. LSR expanded the start `{composite,+,5,+,2}` as two phis of
/// the outer loop, which already holds its counter, the 32-bit sum, the count
/// and the bound: 50 frame accesses and 170 instructions in `bench_sieve` at
/// -Os, 24 and 131 without them (sieve -Os 370 B against SDCC's 257).
const IR: &str = include_str!("data/lsr_outer_square.ll");

fn sieve(level: Level) -> String {
    let request = Request::new(level);
    let prepared = prepare(IR, &request).expect("prepares");
    let (number, name) = prepared.names().enumerate().find(|(_, name)| *name == "bench_sieve").expect("function");
    text_in(&prepared, name, number, &request).expect("compiles")
}

#[test]
fn an_outer_loop_does_not_carry_the_square_of_its_counter() {
    let text = sieve(Level::Os);
    let frame = text.matches("(ix)").count();
    let instructions = text.lines().filter(|line| line.starts_with('\t') && !line.starts_with("\t.")).count();
    assert!(frame <= 30, "{frame} frame accesses:\n{text}");
    assert!(instructions <= 145, "{instructions} instructions:\n{text}");
}
