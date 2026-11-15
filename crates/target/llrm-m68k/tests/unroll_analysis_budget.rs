//! A loop of a great many trips is not simulated trip by trip to price an unroll.

use std::time::Instant;

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::driver::{Level, assembly};

/// `shl_of_a_zero_extended_dividend` as the front end wrote it, with its trip
/// count left open.
const IR: &str = r#"target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @f(i32 noundef %0, i32 noundef %1) noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i64, align 4
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) nounwind
  %6 = load i32, ptr %3, align 2, !tbaa !8
  %7 = zext i32 %6 to i64
  %8 = shl i64 %7, 27
  store i64 %8, ptr %5, align 4, !tbaa !10
  %9 = load i64, ptr %5, align 4, !tbaa !10
  %10 = load i32, ptr %4, align 2, !tbaa !8
  %11 = zext i32 %10 to i64
  %12 = udiv i64 %9, %11
  %13 = trunc i64 %12 to i32
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) nounwind
  ret i32 %13
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = alloca i64, align 4
  %2 = alloca i64, align 4
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  store i64 0, ptr %1, align 4, !tbaa !10
  call void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  store i64 88172645463325252, ptr %2, align 4, !tbaa !10
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  store i32 0, ptr %3, align 2, !tbaa !8
  br label %6

6:
  %7 = load i32, ptr %3, align 2, !tbaa !8
  %8 = icmp ult i32 %7, @TRIPS@
  br i1 %8, label %9, label %37

9:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) nounwind
  %10 = load i64, ptr %2, align 4, !tbaa !10
  %11 = shl i64 %10, 13
  %12 = load i64, ptr %2, align 4, !tbaa !10
  %13 = xor i64 %12, %11
  store i64 %13, ptr %2, align 4, !tbaa !10
  %14 = load i64, ptr %2, align 4, !tbaa !10
  %15 = lshr i64 %14, 7
  %16 = load i64, ptr %2, align 4, !tbaa !10
  %17 = xor i64 %16, %15
  store i64 %17, ptr %2, align 4, !tbaa !10
  %18 = load i64, ptr %2, align 4, !tbaa !10
  %19 = shl i64 %18, 17
  %20 = load i64, ptr %2, align 4, !tbaa !10
  %21 = xor i64 %20, %19
  store i64 %21, ptr %2, align 4, !tbaa !10
  %22 = load i64, ptr %2, align 4, !tbaa !10
  %23 = trunc i64 %22 to i32
  store i32 %23, ptr %4, align 2, !tbaa !8
  %24 = load i64, ptr %2, align 4, !tbaa !10
  %25 = lshr i64 %24, 32
  %26 = trunc i64 %25 to i32
  %27 = or i32 %26, 1
  store i32 %27, ptr %5, align 2, !tbaa !8
  %28 = load i32, ptr %4, align 2, !tbaa !8
  %29 = load i32, ptr %5, align 2, !tbaa !8
  %30 = call i32 @f(i32 noundef %28, i32 noundef %29) nobuiltin "no-builtins"
  %31 = zext i32 %30 to i64
  %32 = load i64, ptr %1, align 4, !tbaa !10
  %33 = add i64 %32, %31
  store i64 %33, ptr %1, align 4, !tbaa !10
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) nounwind
  br label %34

34:
  %35 = load i32, ptr %3, align 2, !tbaa !8
  %36 = add i32 %35, 1
  store i32 %36, ptr %3, align 2, !tbaa !8
  br label %6, !llvm.loop !12

37:
  %38 = load i64, ptr %1, align 4, !tbaa !10
  %39 = and i64 %38, 2147483647
  %40 = trunc i64 %39 to i32
  call void @report(i32 noundef %40) nobuiltin "no-builtins"
  %41 = call i32 @f(i32 noundef -1, i32 noundef 3) nobuiltin "no-builtins"
  %42 = and i32 %41, 2147483647
  call void @report(i32 noundef %42) nobuiltin "no-builtins"
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !6, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long long", !6, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
"#;

fn seconds(trips: u32) -> f64 {
    let text = IR.replace("@TRIPS@", &trips.to_string());
    (0..2)
        .map(|_| {
            let start = Instant::now();
            assembly(&text, Cpu::M68000, Level::O2).expect("compiles");
            start.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min)
}

/// 200000 trips compiled for 41 s of CPU, 49 s of wall time of it in the unroll
/// analysis walking every trip to price an unroll no bar admits: doubling the
/// trips doubled the time. It must not.
#[test]
fn doubling_the_trips_does_not_double_the_compile() {
    let (small, large) = (seconds(50_000), seconds(100_000));
    eprintln!("50000: {small:.3}s, 100000: {large:.3}s");
    assert!(large < 1.5 * small + 0.5, "50000 trips {small}s, 100000 trips {large}s");
}
