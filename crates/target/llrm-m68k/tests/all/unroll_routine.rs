//! The -Os unroll trial priced the loop and its six-fold unrolling by their own
//! bytes. Unrolled, `seen * 3 + i` six times is `seen * 729 + 543`: a call of
//! `__mulsi3` at the 68000, 42 B against the loop's 60 B, which links the
//! routine's 54 B (switch_empty_cases 68000 -Os: 96 B against gcc's 48). The
//! build that calls it pays for it.

use llrm_m68k::driver::{Level, assembly, cpu_of};

/// tests/run/c/switch_empty_cases.c, as clang emits it before any pass.
const SWITCH_EMPTY_CASES: &str =
    "target datalayout = \"E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16\"
@seen = internal global i32 0, align 2
define dso_local i32 @main() nounwind optsize {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  store i32 0, ptr %1, align 2
  store i32 0, ptr %2, align 2
  br label %3
3:
  %4 = load i32, ptr %2, align 2
  %5 = icmp slt i32 %4, 6
  br i1 %5, label %6, label %11
6:
  %7 = load i32, ptr %2, align 2
  call void @classify(i32 noundef %7)
  br label %8
8:
  %9 = load i32, ptr %2, align 2
  %10 = add nsw i32 %9, 1
  store i32 %10, ptr %2, align 2
  br label %3
11:
  %12 = load i32, ptr @seen, align 2
  call void @report(i32 noundef %12)
  ret i32 0
}
define internal void @classify(i32 noundef %0) nounwind optsize {
  %2 = alloca i32, align 2
  store i32 %0, ptr %2, align 2
  %3 = load i32, ptr %2, align 2
  switch i32 %3, label %5 [
    i32 1, label %5
    i32 2, label %5
    i32 3, label %4
    i32 4, label %4
  ]
4:
  br label %5
5:
  %6 = load i32, ptr %2, align 2
  %7 = add nsw i32 %6, 1
  call void @bump(i32 noundef %7)
  ret void
}
declare void @report(i32 noundef)
define internal void @bump(i32 noundef %0) nounwind optsize {
  %2 = alloca i32, align 2
  store i32 %0, ptr %2, align 2
  %3 = load i32, ptr @seen, align 2
  %4 = mul nsw i32 %3, 3
  %5 = load i32, ptr %2, align 2
  %6 = add nsw i32 %4, %5
  store i32 %6, ptr @seen, align 2
  ret void
}";

#[test]
fn an_unrolling_that_links_the_multiply_routine_pays_for_its_bytes() {
    let asm = assembly(SWITCH_EMPTY_CASES, cpu_of("68000").unwrap(), Level::Os).expect("compiles");
    assert!(
        !asm.contains("__mulsi3"),
        "the loop was unrolled into a multiply by 729, which links __mulsi3:
{asm}"
    );
}
