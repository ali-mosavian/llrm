; ModuleID = 'kernels/gcd.c'
source_filename = "kernels/gcd.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

; Function Attrs: nounwind
define dso_local i32 @k(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  br label %5

5:                                                ; preds = %21, %2
  %6 = load i32, ptr %3, align 2, !tbaa !8
  %7 = load i32, ptr %4, align 2, !tbaa !8
  %8 = icmp ne i32 %6, %7
  br i1 %8, label %9, label %22

9:                                                ; preds = %5
  %10 = load i32, ptr %3, align 2, !tbaa !8
  %11 = load i32, ptr %4, align 2, !tbaa !8
  %12 = icmp sgt i32 %10, %11
  br i1 %12, label %13, label %17

13:                                               ; preds = %9
  %14 = load i32, ptr %4, align 2, !tbaa !8
  %15 = load i32, ptr %3, align 2, !tbaa !8
  %16 = sub nsw i32 %15, %14
  store i32 %16, ptr %3, align 2, !tbaa !8
  br label %21

17:                                               ; preds = %9
  %18 = load i32, ptr %3, align 2, !tbaa !8
  %19 = load i32, ptr %4, align 2, !tbaa !8
  %20 = sub nsw i32 %19, %18
  store i32 %20, ptr %4, align 2, !tbaa !8
  br label %21

21:                                               ; preds = %17, %13
  br label %5, !llvm.loop !9

22:                                               ; preds = %5
  %23 = load i32, ptr %3, align 2, !tbaa !8
  ret i32 %23
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

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
!8 = !{!5, !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
