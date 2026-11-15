//! The pin infers function attributes inside the inliner walk, after the early
//! `InstCombine`; stamping `memory(none)` on a helper before it let that
//! `InstCombine` sink the helper's calls, loads and `fadd` into the right arm
//! of an `&&`, and the loop kept its branches (`sfm16_dbg` -O1, 21,241
//! executed instructions against the pin's 21,017).

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::{Level, assembly};

/// `bench_sfm16_dbg`'s frontend IR, its table opaque.
const AND_OF_TESTS: &str = "%union.anon = type { i32 }
%union.anon.0 = type { i32 }
@V = external constant [400 x [11 x i32]]
declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)

define internal float @bits(i32 noundef %0) nounwind {
  %2 = alloca i32, align 4
  %3 = alloca %union.anon, align 4
  store i32 %0, ptr %2, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  %4 = load i32, ptr %2, align 4
  store i32 %4, ptr %3, align 4
  %5 = load float, ptr %3, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  ret float %5
}

define internal i32 @ubits(float noundef %0) nounwind {
  %2 = alloca float, align 4
  %3 = alloca %union.anon.0, align 4
  store float %0, ptr %2, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  %4 = load float, ptr %2, align 4
  store float %4, ptr %3, align 4
  %5 = load i32, ptr %3, align 4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  ret i32 %5
}

define i32 @bench(i32 noundef %0) nounwind {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca float, align 4
  %7 = alloca float, align 4
  %8 = alloca float, align 4
  %9 = alloca i32, align 4
  store i32 %0, ptr %2, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) nounwind
  store i32 0, ptr %4, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) nounwind
  store i32 -1, ptr %5, align 4
  store i32 0, ptr %3, align 4
  br label %10

10:
  %11 = load i32, ptr %3, align 4
  %12 = load i32, ptr %2, align 4
  %13 = icmp slt i32 %11, %12
  br i1 %13, label %14, label %56

14:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %6) nounwind
  %15 = load i32, ptr %3, align 4
  %16 = getelementptr inbounds [400 x [11 x i32]], ptr @V, i32 0, i32 %15
  %17 = getelementptr inbounds [11 x i32], ptr %16, i32 0, i32 0
  %18 = load i32, ptr %17, align 4
  %19 = call float @bits(i32 noundef %18)
  store float %19, ptr %6, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %7) nounwind
  %20 = load i32, ptr %3, align 4
  %21 = getelementptr inbounds [400 x [11 x i32]], ptr @V, i32 0, i32 %20
  %22 = getelementptr inbounds [11 x i32], ptr %21, i32 0, i32 1
  %23 = load i32, ptr %22, align 4
  %24 = call float @bits(i32 noundef %23)
  store float %24, ptr %7, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %8) nounwind
  %25 = load float, ptr %6, align 4
  %26 = load float, ptr %7, align 4
  %27 = fadd float %25, %26
  store float %27, ptr %8, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %9) nounwind
  %28 = load float, ptr %8, align 4
  %29 = call i32 @ubits(float noundef %28)
  store i32 %29, ptr %9, align 4
  %30 = load i32, ptr %3, align 4
  %31 = getelementptr inbounds [400 x [11 x i32]], ptr @V, i32 0, i32 %30
  %32 = getelementptr inbounds [11 x i32], ptr %31, i32 0, i32 2
  %33 = load i32, ptr %32, align 4
  %34 = lshr i32 %33, 23
  %35 = and i32 %34, 255
  %36 = icmp ne i32 %35, 255
  br i1 %36, label %37, label %52

37:
  %38 = load i32, ptr %9, align 4
  %39 = load i32, ptr %3, align 4
  %40 = getelementptr inbounds [400 x [11 x i32]], ptr @V, i32 0, i32 %39
  %41 = getelementptr inbounds [11 x i32], ptr %40, i32 0, i32 2
  %42 = load i32, ptr %41, align 4
  %43 = icmp ne i32 %38, %42
  br i1 %43, label %44, label %52

44:
  %45 = load i32, ptr %4, align 4
  %46 = icmp ne i32 %45, 0
  br i1 %46, label %49, label %47

47:
  %48 = load i32, ptr %3, align 4
  store i32 %48, ptr %5, align 4
  br label %49

49:
  %50 = load i32, ptr %4, align 4
  %51 = add nsw i32 %50, 1
  store i32 %51, ptr %4, align 4
  br label %52

52:
  call void @llvm.lifetime.end.p0(i64 -1, ptr %9) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %8) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %7) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %6) nounwind
  br label %53

53:
  %54 = load i32, ptr %3, align 4
  %55 = add nsw i32 %54, 1
  store i32 %55, ptr %3, align 4
  br label %10

56:
  %57 = load i32, ptr %4, align 4
  %58 = mul nsw i32 %57, 1000
  %59 = load i32, ptr %5, align 4
  %60 = add nsw i32 %58, %59
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  ret i32 %60
}
";

#[test]
fn an_and_of_tests_over_inlined_helpers_keeps_no_branch_but_the_loop_exit() {
    let text = assembly(AND_OF_TESTS, Width::W32, Options { level: Level::O1, ..Options::default() }).unwrap();
    // The entry guard and the loop's exit.
    assert_eq!(text.matches("br_if").count(), 2, "{text}");
}
