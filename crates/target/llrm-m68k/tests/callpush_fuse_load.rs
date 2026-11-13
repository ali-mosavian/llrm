//! byval_inline -Os (68000/68020, +4 B against gcc, 4 cells): `report(g.a[0])` loaded a stack word into `%a1` and
//! stored it into the call area (`move.l (4,%sp), %a1; move.l %a1, (%sp)`, 6 B). The area stays a store (a push
//! costs a cleanup per call), and nothing moved the load into the store, which reads memory itself:
//! `move.l (4,%sp), (%sp)`, 4 B.

use llrm_m68k::driver::{Level, assembly, cpu_of};

const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
@shared = global [10 x i32] zeroinitializer, align 2
declare void @report(i32) optsize
define i32 @main() optsize {
  %g = alloca [10 x i32], align 2
  br label %fill
fill:
  %i = phi i32 [ %next, %fill ], [ 1, %0 ]
  %at = getelementptr i32, ptr %g, i32 %i
  %sh = getelementptr i32, ptr @shared, i32 %i
  store i32 %i, ptr %at, align 2
  store i32 %i, ptr %sh, align 2
  %next = add i32 %i, 1
  %done = icmp eq i32 %next, 10
  br i1 %done, label %use, label %fill
use:
  %p9 = getelementptr i8, ptr %g, i32 36
  %v9 = load i32, ptr %p9
  %r9 = add i32 %v9, 99
  call void @report(i32 %r9)
  %v0 = load i32, ptr %g, align 2
  call void @report(i32 %v0)
  %s9 = load i32, ptr getelementptr (i8, ptr @shared, i32 36)
  %q9 = add i32 %s9, 99
  call void @report(i32 %q9)
  %s0 = load i32, ptr @shared, align 2
  call void @report(i32 %s0)
  %p3 = getelementptr i8, ptr %g, i32 12
  %v3 = load i32, ptr %p3
  %sum = add i32 %v3, %v9
  call void @report(i32 %sum)
  ret i32 0
}
"#;

#[test]
fn a_loaded_word_stored_into_the_call_area_is_moved_memory_to_memory() {
    for cpu in ["68000", "68020"] {
        let asm = assembly(IR, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let detours = asm.lines().filter(|line| line.trim_start().starts_with("move.l\t%a1, (%sp)")).count();
        assert_eq!(detours, 0, "{cpu}: a loaded word goes through %a1 to the call area:\n{asm}");
        let fused = asm.lines().filter(|line| line.trim_start().starts_with("move.l\t(") && line.ends_with(", (%sp)")).count();
        assert!(fused >= 2, "{cpu}: {fused} memory-to-memory stores into the call area:\n{asm}");
    }
}
