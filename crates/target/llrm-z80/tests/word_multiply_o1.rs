//! word_sign_zero_guard -O1 (SDCC 461 B / 32,430 T): the `* 1000`, `* 100`
//! and `* 10` of each report line were `__mulhi3` calls, 413 B / 51,413 T. The
//! shift-and-add chains cost 12 B more and run 43% faster (-O2: 425 B /
//! 29,116 T). -O1 held both axes still and refused a chain a byte bigger; it
//! now takes it where the clocks it saves repay the bytes at the strict rate.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_word_times_a_constant_is_a_chain_at_o1_where_the_clocks_repay_the_bytes() {
    let ir = include_str!("data/word_sign_zero_guard_o1.ll");
    let asm = text(ir, "main", 0, &Request::new(Level::O1)).expect("compiles");
    assert!(!asm.contains("__mulhi3"), "main calls the routine at -O1:\n{asm}");
}
