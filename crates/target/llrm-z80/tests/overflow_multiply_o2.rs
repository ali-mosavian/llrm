//! overflow_builtins -O2/-O3 (10890 B / 943576 T against 7493 B / 795223 T at
//! -O1): its `llvm.*mul.with.overflow` of 32 and 64 bits were 168 `__mulhi3`
//! calls in `main` where -O1 has 72 and 24 `__umulhisi3`. A function with such
//! a multiply is also selected with the product call, and the module price
//! picks the smaller.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn overflow_checked_products_are_the_call_at_o2() {
    let ir = include_str!("data/overflow_builtins_o2.ll");
    let asm = text(ir, "main", 0, &Request::new(Level::O2)).expect("compiles");
    let partial = asm.matches("call __mulhi3").count();
    assert!(partial <= 100, "{partial} partial products in main");
}
