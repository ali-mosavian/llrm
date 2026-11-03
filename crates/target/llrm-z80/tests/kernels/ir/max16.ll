source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local i16 @max16(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !2
  store i16 %1, ptr %4, align 2, !tbaa !2
  %5 = load i16, ptr %3, align 2, !tbaa !2
  %6 = load i16, ptr %4, align 2, !tbaa !2
  %7 = icmp ugt i16 %5, %6
  br i1 %7, label %8, label %10

8:                                                ; preds = %2
  %9 = load i16, ptr %3, align 2, !tbaa !2
  br label %12

10:                                               ; preds = %2
  %11 = load i16, ptr %4, align 2, !tbaa !2
  br label %12

12:                                               ; preds = %10, %8
  %13 = phi i16 [ %9, %8 ], [ %11, %10 ]
  ret i16 %13
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
