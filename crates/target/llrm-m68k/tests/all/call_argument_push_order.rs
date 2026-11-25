//! A call's argument stores were scheduled in any order, and a stored register
//! reused for the next argument was redefined before the last store, so the
//! pusher kept the reserved call area: QuickrBasic `arena_grow` stored
//! `move.l %d0, (4,%sp)` / `move.l %a1, (%sp)` for `memset`, 12 B of frame and
//! 14 B over the pushes. The stores run in push order (gcc's `emit_push_insn`).

use llrm_m68k::driver::{Level, assembly, cpu_of};

const ARENA_GROW: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
declare ptr @arena_alloc(ptr, i32)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
define ptr @arena_grow(ptr %0, ptr %1, i32 %2, i32 %3) optsize {
  %5 = icmp eq ptr %1, null
  br i1 %5, label %6, label %8
6:
  %7 = tail call ptr @arena_alloc(ptr %0, i32 %3)
  br label %34
8:
  %9 = add i32 %2, 7
  %10 = and i32 %9, -8
  %11 = getelementptr inbounds i8, ptr %1, i32 %10
  %12 = getelementptr inbounds i8, ptr %0, i32 8
  %13 = load ptr, ptr %12, align 2
  %14 = icmp eq ptr %11, %13
  br i1 %14, label %15, label %30
15:
  %16 = getelementptr inbounds i8, ptr %0, i32 4
  %17 = load ptr, ptr %16, align 2
  %18 = getelementptr inbounds i8, ptr %17, i32 4
  %19 = load ptr, ptr %18, align 4
  %20 = ptrtoint ptr %19 to i32
  %21 = ptrtoint ptr %1 to i32
  %22 = sub i32 %20, %21
  %23 = add i32 %3, 7
  %24 = and i32 %23, -8
  %25 = icmp ult i32 %22, %24
  br i1 %25, label %30, label %26
26:
  %27 = getelementptr inbounds i8, ptr %1, i32 %24
  store ptr %27, ptr %12, align 2
  %28 = getelementptr inbounds i8, ptr %1, i32 %2
  %29 = sub i32 %3, %2
  tail call void @llvm.memset.p0.i32(ptr align 1 %28, i8 0, i32 %29, i1 false)
  br label %34
30:
  %31 = tail call ptr @arena_alloc(ptr %0, i32 %3)
  %32 = icmp eq ptr %31, null
  br i1 %32, label %34, label %33
33:
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 1 %31, ptr align 1 %1, i32 %2, i1 false)
  br label %34
34:
  %35 = phi ptr [ %1, %26 ], [ %31, %33 ], [ null, %30 ], [ %7, %6 ]
  ret ptr %35
}"#;

#[test]
fn a_call_with_a_reused_argument_register_is_pushed_not_stored() {
    for cpu in ["68000", "68020"] {
        let out = assembly(ARENA_GROW, cpu_of(cpu).unwrap(), Level::Os).expect("compiles");
        let stores = out
            .lines()
            .filter(|line| {
                let line = line.trim();
                line.starts_with("move.l")
                    && (line.ends_with(", (%sp)") || line.ends_with(", (4,%sp)") || line.ends_with(", (8,%sp)"))
            })
            .count();
        assert_eq!(stores, 0, "{cpu}: {stores} argument stores kept in the call area:\n{out}");
    }
}
