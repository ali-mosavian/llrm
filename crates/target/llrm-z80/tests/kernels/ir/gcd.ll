source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @gcd(i8 noundef zeroext %0, i8 noundef zeroext %1) #0 {
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  store i8 %0, ptr %3, align 1, !tbaa !2
  store i8 %1, ptr %4, align 1, !tbaa !2
  br label %5

5:                                                ; preds = %31, %2
  %6 = load i8, ptr %3, align 1, !tbaa !2
  %7 = zext i8 %6 to i16
  %8 = load i8, ptr %4, align 1, !tbaa !2
  %9 = zext i8 %8 to i16
  %10 = icmp ne i16 %7, %9
  br i1 %10, label %11, label %32

11:                                               ; preds = %5
  %12 = load i8, ptr %3, align 1, !tbaa !2
  %13 = zext i8 %12 to i16
  %14 = load i8, ptr %4, align 1, !tbaa !2
  %15 = zext i8 %14 to i16
  %16 = icmp sgt i16 %13, %15
  br i1 %16, label %17, label %24

17:                                               ; preds = %11
  %18 = load i8, ptr %4, align 1, !tbaa !2
  %19 = zext i8 %18 to i16
  %20 = load i8, ptr %3, align 1, !tbaa !2
  %21 = zext i8 %20 to i16
  %22 = sub nsw i16 %21, %19
  %23 = trunc i16 %22 to i8
  store i8 %23, ptr %3, align 1, !tbaa !2
  br label %31

24:                                               ; preds = %11
  %25 = load i8, ptr %3, align 1, !tbaa !2
  %26 = zext i8 %25 to i16
  %27 = load i8, ptr %4, align 1, !tbaa !2
  %28 = zext i8 %27 to i16
  %29 = sub nsw i16 %28, %26
  %30 = trunc i16 %29 to i8
  store i8 %30, ptr %4, align 1, !tbaa !2
  br label %31

31:                                               ; preds = %24, %17
  br label %5, !llvm.loop !5

32:                                               ; preds = %5
  %33 = load i8, ptr %3, align 1, !tbaa !2
  ret i8 %33
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"omnipotent char", !4, i64 0}
!4 = !{!"Simple C/C++ TBAA"}
!5 = distinct !{!5, !6, !7}
!6 = !{!"llvm.loop.mustprogress"}
!7 = !{!"llvm.loop.unroll.disable"}
