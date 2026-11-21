//! data_int64 -Os (537 B / 105501 T against 801 B / 19296 T at -O1, SDCC 982 B / 70515 T): `main` held a 64-bit divide
//! and constant 64-bit shifts, so its alternative was the inline shifts alone and the speed divide was never priced:
//! `__s_divdi3` is 99000 T a call. The speed routines ride along with the shifts, and the price takes the one that
//! gives up the smaller part of an axis.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

#[test]
fn a_long_long_divide_beside_constant_shifts_is_priced_with_the_speed_routine_at_os() {
    let ir = include_str!("data/data_int64_os.ll");
    let asm = text(ir, "main", 0, &Request::new(Level::Os)).expect("compiles");
    assert!(asm.contains("call __divdi3") && !asm.contains("__s_divdi3"), "{asm}");
}
