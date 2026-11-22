//! The pin's M68k lets a constant memset be 8 stores (4 at -Os), as memcpy and memmove: the shared
//! table was x86's, 16 (8), so a 64-byte clear was 16 inline `move.l` where the pin calls `memset`.

use llrm_m68k::driver::{Level, assembly, cpu_of};

fn clear(bytes: u32) -> String {
    format!(
        "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
target triple = \"m68k-unknown-linux-gnu\"
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
define void @f(ptr %p) {{
  call void @llvm.memset.p0.i32(ptr align 4 %p, i8 0, i32 {bytes}, i1 false)
  ret void
}}
"
    )
}

#[test]
fn a_64_byte_clear_is_a_call_and_a_32_byte_clear_is_stores() {
    for cpu in ["68000", "68020"] {
        let long = assembly(&clear(64), cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(long.contains("memset"), "{cpu}: 16 stores where the pin calls memset:\n{long}");
        let short = assembly(&clear(32), cpu_of(cpu).unwrap(), Level::O2).expect("compiles");
        assert!(!short.contains("memset"), "{cpu}: 8 stores are inline:\n{short}");
    }
}
