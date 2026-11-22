//! The product of two zero-extended words is the `__umulhisi3` call at -O1, in
//! a loop too.

use llrm_z80::driver::{Level, Request};
use llrm_z80::expand::price_emitted;

/// shellsort -O1: the checksum loop's `(unsigned long)values[i] * (i + 1)`
/// kept two inline `__mulhi3` sequences in the loop: the function was 523 B
/// with them and is 376 B with the call (the cell 560 B / 448,157 T against
/// 407 B / 391,597 T; SDCC's best is 488 B).
#[test]
fn a_widening_product_in_a_loop_at_o1_is_the_call() {
    let ir = include_str!("data/shellsort_o1.ll");
    let lines = llrm_z80::compile::lines(ir, "bench_shellsort", 0, &Request::new(Level::O1))
        .unwrap_or_else(|why| panic!("{why}"));
    let (bytes, _) = price_emitted(&lines, true);
    assert!(bytes <= 450, "bench_shellsort is {bytes} B at -O1; with the products inline it is 523 B");
}
