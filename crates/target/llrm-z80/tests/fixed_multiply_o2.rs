//! fixed_point -O2/-O3 (15010 B / 1298178 T against 9170 B / 1028697 T at -O1):
//! its `llvm.*mul.fix.sat` products were 240 `__mulhi3` calls in `main` where
//! -O1 has 72 and 42 `__umulhisi3`. A function with a fixed-point multiply of
//! 32 bits or more is also selected with the product call, and the module price
//! picks the smaller.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn fixed_point_products_are_the_call_at_o2() {
    let ir = include_str!("data/fixed_point_o2.ll");
    for level in [Level::O2, Level::O3] {
        let asm = text(ir, "main", 0, &Request::new(level)).expect("compiles");
        let partial = asm.matches("call __mulhi3").count();
        assert!(partial <= 100, "{level:?}: {partial} partial products in main");
    }
}
