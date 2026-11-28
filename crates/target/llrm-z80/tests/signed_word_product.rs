//! The product of two sign-extended shorts was the 32 x 32 `__mulsi3`: bench
//! dot -O1 7,297,501 T against SDCC's 3,315,390. It is the 16 x 16 routine
//! `__umulhisi3` with the sign corrections (`expandMUL_LOHI`): 3,472,835 T. The
//! model prices the module at 5,522,047 clocks before and 1,478,881 after. The
//! signed routine `__mulhisi3` (magnitudes under 256 take 8 steps, no sign
//! fixups in the loop) runs it in 2,226,497 T.

use llrm_z80::compile::{effective, module_priced, text};
use llrm_z80::driver::{Level, Request};

#[test]
fn dot_at_o1_multiplies_through_the_word_routine() {
    let ir = include_str!("data/dot_signed_product_o1.ll");
    let request = effective(ir, &Request::new(Level::O1)).expect("chooses");
    let (bytes, clocks) = module_priced(ir, &request).expect("prices");
    assert!(clocks <= 2_000_000, "-O1 {bytes} B / {clocks} clocks: the product is `__mulsi3` again");
}

/// The product is the signed routine's call: `__umulhisi3` with the sign fixups
/// of the caller was 3,472,835 T, 4.7% over SDCC's bar.
#[test]
fn dot_at_o1_calls_the_signed_word_routine() {
    let ir = include_str!("data/dot_signed_product_o1.ll");
    let request = effective(ir, &Request::new(Level::O1)).expect("chooses");
    let asm = text(ir, "bench_dot", 0, &request).expect("compiles");
    assert!(asm.contains("call __mulhisi3") && !asm.contains("__mulsi3"), "bench_dot:\n{asm}");
}
