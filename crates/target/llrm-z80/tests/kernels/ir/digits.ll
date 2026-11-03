source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @digits(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i8, align 1
  store i16 %0, ptr %2, align 2, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 1, ptr %3) #2
  store i8 1, ptr %3, align 1, !tbaa !6
  br label %4

4:                                                ; preds = %7, %1
  %5 = load i16, ptr %2, align 2, !tbaa !2
  %6 = icmp uge i16 %5, 10
  br i1 %6, label %7, label %12

7:                                                ; preds = %4
  %8 = load i16, ptr %2, align 2, !tbaa !2
  %9 = udiv i16 %8, 10
  store i16 %9, ptr %2, align 2, !tbaa !2
  %10 = load i8, ptr %3, align 1, !tbaa !6
  %11 = add i8 %10, 1
  store i8 %11, ptr %3, align 1, !tbaa !6
  br label %4, !llvm.loop !7

12:                                               ; preds = %4
  %13 = load i8, ptr %3, align 1, !tbaa !6
  call void @llvm.lifetime.end.p0(i64 1, ptr %3) #2
  ret i8 %13
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
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!4, !4, i64 0}
!7 = distinct !{!7, !8, !9}
!8 = !{!"llvm.loop.mustprogress"}
!9 = !{!"llvm.loop.unroll.disable"}
