//! A `float` stored from a `double` is converted, then stored as four bytes.

mod common;

use common::pipeline::datalayout;
use llrm_6502::driver::{Level, Request, assembly};

/// `store (fptrunc x)` was combined into a truncating store of the `double`,
/// which the target has no instruction for: no `__truncdfsf2` call, eight bytes
/// written over a four-byte object.
#[test]
fn a_double_stored_as_a_float_is_converted_by_the_library_first() {
    let ir = "define void @g(ptr %q, ptr %p) {\n  %x = load double, ptr %q\n  %r = fptrunc double %x to float\n  store float %r, ptr %p\n  ret void\n}\n";
    let ir = format!("target datalayout = \"{}\"\n{ir}", datalayout());
    for level in [Level::O0, Level::O2] {
        let out = assembly(&ir, &Request::new(level)).unwrap_or_else(|e| panic!("{e}"));
        assert!(out.contains("__truncdfsf2"), "no conversion call:\n{out}");
    }
}
