//! A copy of constant bytes is immediate stores of them, the first byte in
//! the high bits of a wider store on a big-endian target: bytes 1, 2, 3, 4
//! were stored as 0x0201 and 0x0403 (`bitfield_overhang_init` -O0 read 131331
//! for 66051).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
@c = private unnamed_addr constant [4 x i8] [i8 1, i8 2, i8 3, i8 4], align 2
define void @f(ptr %d) {
  call void @llvm.memcpy.p0.p0.i32(ptr align 2 %d, ptr align 2 @c, i32 4, i1 false)
  ret void
}
";

#[test]
fn copied_constant_bytes_keep_their_order() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O0, Level::O2] {
            let asm = assembly(IR, cpu_of(cpu).unwrap(), level).unwrap_or_else(|why| panic!("{why}"));
            let wrong = ["#513", "#1027", "#67305985"];
            assert!(wrong.iter().all(|w| !asm.contains(w)), "{cpu} {level:?}: little-endian constants:\n{asm}");
            assert!(asm.contains("#258") || asm.contains("#16909060"), "{cpu} {level:?}: no 0x0102 stored:\n{asm}");
        }
    }
}
