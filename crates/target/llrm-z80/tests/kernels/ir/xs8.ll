source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @xs8(i8 noundef zeroext %0, i8 noundef zeroext %1) #0 {
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  store i8 %0, ptr %3, align 1, !tbaa !2
  store i8 %1, ptr %4, align 1, !tbaa !2
  br label %5

5:                                                ; preds = %9, %2
  %6 = load i8, ptr %4, align 1, !tbaa !2
  %7 = add i8 %6, -1
  store i8 %7, ptr %4, align 1, !tbaa !2
  %8 = icmp ne i8 %6, 0
  br i1 %8, label %9, label %31

9:                                                ; preds = %5
  %10 = load i8, ptr %3, align 1, !tbaa !2
  %11 = zext i8 %10 to i16
  %12 = shl i16 %11, 3
  %13 = load i8, ptr %3, align 1, !tbaa !2
  %14 = zext i8 %13 to i16
  %15 = xor i16 %14, %12
  %16 = trunc i16 %15 to i8
  store i8 %16, ptr %3, align 1, !tbaa !2
  %17 = load i8, ptr %3, align 1, !tbaa !2
  %18 = zext i8 %17 to i16
  %19 = ashr i16 %18, 5
  %20 = load i8, ptr %3, align 1, !tbaa !2
  %21 = zext i8 %20 to i16
  %22 = xor i16 %21, %19
  %23 = trunc i16 %22 to i8
  store i8 %23, ptr %3, align 1, !tbaa !2
  %24 = load i8, ptr %3, align 1, !tbaa !2
  %25 = zext i8 %24 to i16
  %26 = shl i16 %25, 1
  %27 = load i8, ptr %3, align 1, !tbaa !2
  %28 = zext i8 %27 to i16
  %29 = xor i16 %28, %26
  %30 = trunc i16 %29 to i8
  store i8 %30, ptr %3, align 1, !tbaa !2
  br label %5, !llvm.loop !5

31:                                               ; preds = %5
  %32 = load i8, ptr %3, align 1, !tbaa !2
  ret i8 %32
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"omnipotent char", !4, i64 0}
!4 = !{!"Simple C/C++ TBAA"}
!5 = distinct !{!5, !6}
!6 = !{!"llvm.loop.mustprogress"}
