source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local i16 @fib(i8 noundef zeroext %0) #0 {
  %2 = alloca i8, align 1
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i8 %0, ptr %2, align 1, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 2, ptr %3) #2
  store i16 0, ptr %3, align 2, !tbaa !5
  call void @llvm.lifetime.start.p0(i64 2, ptr %4) #2
  store i16 1, ptr %4, align 2, !tbaa !5
  call void @llvm.lifetime.start.p0(i64 2, ptr %5) #2
  br label %6

6:                                                ; preds = %10, %1
  %7 = load i8, ptr %2, align 1, !tbaa !2
  %8 = add i8 %7, -1
  store i8 %8, ptr %2, align 1, !tbaa !2
  %9 = icmp ne i8 %7, 0
  br i1 %9, label %10, label %16

10:                                               ; preds = %6
  %11 = load i16, ptr %3, align 2, !tbaa !5
  %12 = load i16, ptr %4, align 2, !tbaa !5
  %13 = add i16 %11, %12
  store i16 %13, ptr %5, align 2, !tbaa !5
  %14 = load i16, ptr %4, align 2, !tbaa !5
  store i16 %14, ptr %3, align 2, !tbaa !5
  %15 = load i16, ptr %5, align 2, !tbaa !5
  store i16 %15, ptr %4, align 2, !tbaa !5
  br label %6, !llvm.loop !7

16:                                               ; preds = %6
  %17 = load i16, ptr %3, align 2, !tbaa !5
  call void @llvm.lifetime.end.p0(i64 2, ptr %5) #2
  call void @llvm.lifetime.end.p0(i64 2, ptr %4) #2
  call void @llvm.lifetime.end.p0(i64 2, ptr %3) #2
  ret i16 %17
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) #1

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) #1

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!2 = !{!3, !3, i64 0}
!3 = !{!"omnipotent char", !4, i64 0}
!4 = !{!"Simple C/C++ TBAA"}
!5 = !{!6, !6, i64 0}
!6 = !{!"int", !3, i64 0}
!7 = distinct !{!7, !8, !9}
!8 = !{!"llvm.loop.mustprogress"}
!9 = !{!"llvm.loop.unroll.disable"}
