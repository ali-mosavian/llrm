//! `__builtin_constant_p` at `-O0` left `llvm.is.constant.i32` for the backend,
//! which refused it ("intrinsic call llvm.is.constant.i32"): builtins_table -O0
//! did not compile. The driver now lowers it at every level, as the pin does.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare i1 @llvm.is.constant.i32(i32)
define i32 @classify(i32 %x) {
  %c = call i1 @llvm.is.constant.i32(i32 %x)
  %z = zext i1 %c to i32
  %r = add i32 %z, %x
  ret i32 %r
}";

#[test]
fn is_constant_compiles_at_every_level() {
    for level in [Level::O0, Level::O1, Level::O2, Level::Os] {
        for cpu in ["68000", "68020"] {
            let out = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{cpu}: {why}"));
            assert!(!out.contains("is.constant"), "{cpu}:\n{out}");
        }
    }
}
