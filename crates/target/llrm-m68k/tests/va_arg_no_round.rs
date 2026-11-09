//! `va_arg(ap, int)` rounded the list pointer to the type's alignment, 2:
//! `addq.l #1,%d; and.w #65534,%d`, 6 B per `va_arg` (variadic -Os 68000 +26 B
//! over gcc in `walk`, +34 in `sum`). Every stack argument sits in a
//! 4-byte slot (`getMinStackArgumentAlignment`), so no rounding is owed.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const SUM: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
define i32 @f(i32 %n, ...) {
  %ap = alloca ptr, align 2
  call void @llvm.va_start.p0(ptr %ap)
  %v = va_arg ptr %ap, i32
  call void @llvm.va_end.p0(ptr %ap)
  ret i32 %v
}
declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
"#;

#[test]
fn a_va_arg_int_reads_the_next_slot_without_rounding_the_list_pointer() {
    for cpu in ["68000", "68020"] {
        for level in [Level::O2, Level::Os] {
            let out = assembly(SUM, cpu_of(cpu).unwrap(), level).expect("compiles");
            assert!(!out.contains("65534") && !out.contains("and."), "{cpu}: the list pointer is rounded:\n{out}");
        }
    }
}
