; ModuleID = 'mc.ll'
source_filename = "/home/alim/targets/ct-src/tests/run/c/mul_constants.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

@values = external hidden global [8 x i32], align 2

; Function Attrs: noinline nounwind
define hidden i32 @sum3UL() #0 {
  %1 = alloca i32, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  store i32 0, ptr %1, align 2
  store i32 -1640531527, ptr %3, align 2
  store i32 0, ptr %2, align 2
  br label %4

4:                                                ; preds = %15, %0
  %5 = load i32, ptr %2, align 2
  %6 = icmp ult i32 %5, 8
  br i1 %6, label %7, label %18

7:                                                ; preds = %4
  %8 = load i32, ptr %2, align 2
  %9 = trunc i32 %8 to i16
  %10 = getelementptr inbounds nuw [8 x i32], ptr @values, i16 0, i16 %9
  %11 = load i32, ptr %10, align 2
  %12 = mul i32 %11, 3
  %13 = load i32, ptr %1, align 2
  %14 = add i32 %13, %12
  store i32 %14, ptr %1, align 2
  br label %15

15:                                               ; preds = %7
  %16 = load i32, ptr %2, align 2
  %17 = add i32 %16, 1
  store i32 %17, ptr %2, align 2
  br label %4, !llvm.loop !2

18:                                               ; preds = %4
  store i32 0, ptr %2, align 2
  br label %19

19:                                               ; preds = %31, %18
  %20 = load i32, ptr %2, align 2
  %21 = icmp ult i32 %20, 200
  br i1 %21, label %22, label %34

22:                                               ; preds = %19
  %23 = load i32, ptr %3, align 2
  %24 = mul i32 %23, 1664525
  %25 = add i32 %24, 1013904223
  store i32 %25, ptr %3, align 2
  %26 = load i32, ptr %1, align 2
  %27 = mul i32 %26, 31
  %28 = load i32, ptr %3, align 2
  %29 = mul i32 %28, 3
  %30 = add i32 %27, %29
  store i32 %30, ptr %1, align 2
  br label %31

31:                                               ; preds = %22
  %32 = load i32, ptr %2, align 2
  %33 = add i32 %32, 1
  store i32 %33, ptr %2, align 2
  br label %19, !llvm.loop !4

34:                                               ; preds = %19
  %35 = load i32, ptr %1, align 2
  ret i32 %35
}

attributes #0 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = distinct !{!2, !3}
!3 = !{!"llvm.loop.mustprogress"}
!4 = distinct !{!4, !3}
