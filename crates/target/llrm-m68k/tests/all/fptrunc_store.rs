//! A `float` stored from a `double` is converted, then stored as four bytes.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "
target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
define void @g(ptr %q, ptr %p) {
  %x = load double, ptr %q
  %r = fptrunc double %x to float
  store float %r, ptr %p
  ret void
}
";

/// `store (fptrunc x)` was combined into a truncating store of the `double`,
/// which the target has no instruction for: no `__truncdfsf2` call, eight bytes
/// written over a four-byte object.
#[test]
fn a_double_stored_as_a_float_is_converted_by_the_library_first() {
    for level in [Level::O0, Level::O2] {
        let out = assembly(IR, cpu_of("68000").unwrap(), level).expect("compiles");
        assert!(out.contains("__truncdfsf2"), "no conversion call:\n{out}");
    }
}
