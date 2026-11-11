//! `va_list` is `char *` on the z80: its slot is promoted to a register.

use llrm_z80::driver::{Level, Request, program};

/// `sum(int n, ...)` kept its `va_list` in a frame slot the pin's SROA cannot
/// promote (the intrinsics are lowered in selection): -Os was 76 B with an IX
/// frame, six `(ix` accesses and a `push hl; pop hl; push hl` round trip per
/// argument. SDCC keeps the pointer in a register.
#[test]
fn a_variadic_function_keeps_its_argument_pointer_in_a_register() {
    let ir = include_str!("data/va_list_sum_os.ll");
    let program = program(ir, &Request::new(Level::Os)).unwrap_or_else(|why| panic!("{why}"));
    let text = llrm_mir::print::module(&program.modules[0]);
    assert!(!text.contains("alloca"), "`sum` keeps its va_list in a slot:\n{text}");
    assert!(text.contains("llrm.vararg.address"), "va_start is not a store of the first argument's address:\n{text}");
}
