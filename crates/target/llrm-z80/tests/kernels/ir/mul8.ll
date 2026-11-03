source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @mul8(i8 noundef zeroext %0, i8 noundef zeroext %1) #0 {
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  store i8 %0, ptr %3, align 1, !tbaa !2
  store i8 %1, ptr %4, align 1, !tbaa !2
  %5 = load i8, ptr %3, align 1, !tbaa !2
  %6 = zext i8 %5 to i16
  %7 = load i8, ptr %4, align 1, !tbaa !2
  %8 = zext i8 %7 to i16
  %9 = mul nsw i16 %6, %8
  %10 = trunc i16 %9 to i8
  ret i8 %10
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"omnipotent char", !4, i64 0}
!4 = !{!"Simple C/C++ TBAA"}
