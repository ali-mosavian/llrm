source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local i16 @scmp(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca ptr, align 2
  store ptr %0, ptr %3, align 2, !tbaa !2
  store ptr %1, ptr %4, align 2, !tbaa !2
  br label %5

5:                                                ; preds = %20, %2
  %6 = load ptr, ptr %3, align 2, !tbaa !2
  %7 = load i8, ptr %6, align 1, !tbaa !7
  %8 = zext i8 %7 to i16
  %9 = icmp ne i16 %8, 0
  br i1 %9, label %10, label %18

10:                                               ; preds = %5
  %11 = load ptr, ptr %3, align 2, !tbaa !2
  %12 = load i8, ptr %11, align 1, !tbaa !7
  %13 = zext i8 %12 to i16
  %14 = load ptr, ptr %4, align 2, !tbaa !2
  %15 = load i8, ptr %14, align 1, !tbaa !7
  %16 = zext i8 %15 to i16
  %17 = icmp eq i16 %13, %16
  br label %18

18:                                               ; preds = %10, %5
  %19 = phi i1 [ false, %5 ], [ %17, %10 ]
  br i1 %19, label %20, label %25

20:                                               ; preds = %18
  %21 = load ptr, ptr %3, align 2, !tbaa !2
  %22 = getelementptr inbounds nuw i8, ptr %21, i32 1
  store ptr %22, ptr %3, align 2, !tbaa !2
  %23 = load ptr, ptr %4, align 2, !tbaa !2
  %24 = getelementptr inbounds nuw i8, ptr %23, i32 1
  store ptr %24, ptr %4, align 2, !tbaa !2
  br label %5, !llvm.loop !8

25:                                               ; preds = %18
  %26 = load ptr, ptr %3, align 2, !tbaa !2
  %27 = load i8, ptr %26, align 1, !tbaa !7
  %28 = zext i8 %27 to i16
  %29 = load ptr, ptr %4, align 2, !tbaa !2
  %30 = load i8, ptr %29, align 1, !tbaa !7
  %31 = zext i8 %30 to i16
  %32 = sub nsw i16 %28, %31
  ret i16 %32
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"p1 omnipotent char", !4, i64 0}
!4 = !{!"any pointer", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!5, !5, i64 0}
!8 = distinct !{!8, !9, !10}
!9 = !{!"llvm.loop.mustprogress"}
!10 = !{!"llvm.loop.unroll.disable"}
