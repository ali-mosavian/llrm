//! A `float` stored from a `double` is converted, then stored as four bytes.

use llrm_z80::compile::text;
use llrm_z80::driver::{Level, Request};

/// `store (fptrunc x)` was combined into a truncating store of the `double`
/// that z80 has no instruction for: no `__truncdfsf2` call, eight bytes written
/// over a four-byte object (`float A = vd; (long)(A * 16)` read 0, twin of
/// object_extent_loop 2026-11-12).
#[test]
fn a_double_stored_as_a_float_is_converted_by_the_library_first() {
    for level in [Level::O0, Level::O2] {
        let asm = text(include_str!("kernels/ir/fptrunc_store.ll"), "g", 0, &Request::new(level)).expect("compiles");
        let stores = asm.lines().filter(|line| line.trim_start().starts_with("ld (hl),")).count();
        assert!(asm.contains("call __truncdfsf2") && stores <= 4, "{stores} byte stores:\n{asm}");
    }
}
