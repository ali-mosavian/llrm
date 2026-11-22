//! shellsort -O2 (560 B / 448K T against 407 B / 392K T at -O1, SDCC 488 B):
//! the checksum loop's `(unsigned long) values[i] * (i + 1)` was four
//! `__mulhi3` calls and the glue between them where -O1 has the one
//! `__umulhisi3` call. A function with such a product is also selected with
//! the call and the module price picks the smaller.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_word_product_is_the_call_where_that_is_smaller_and_no_slower_at_o2() {
    let ir = include_str!("data/shellsort_o2.ll");
    for level in [Level::O2, Level::O3] {
        let asm = text(ir, "bench_shellsort", 0, &Request::new(level)).expect("compiles");
        let calls = asm.matches("call __mulhi3").count();
        assert_eq!(calls, 0, "{level:?}: {calls} partial products in the checksum loop:\n{asm}");
    }
}
