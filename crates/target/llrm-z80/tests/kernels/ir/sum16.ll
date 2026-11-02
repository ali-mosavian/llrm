source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local i16 @sum16(ptr noundef %0, i8 noundef zeroext %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i8, align 1
  %5 = alloca i16, align 2
  %6 = alloca i8, align 1
  store ptr %0, ptr %3, align 2, !tbaa !2
  store i8 %1, ptr %4, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 2, ptr %5) #2
  store i16 0, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 1, ptr %6) #2
  store i8 0, ptr %6, align 1, !tbaa !7
  br label %7

7:                                                ; preds = %22, %2
  %8 = load i8, ptr %6, align 1, !tbaa !7
  %9 = zext i8 %8 to i16
  %10 = load i8, ptr %4, align 1, !tbaa !7
  %11 = zext i8 %10 to i16
  %12 = icmp slt i16 %9, %11
  br i1 %12, label %13, label %25

13:                                               ; preds = %7
  %14 = load ptr, ptr %3, align 2, !tbaa !2
  %15 = load i8, ptr %6, align 1, !tbaa !7
  %16 = zext i8 %15 to i16
  %17 = getelementptr inbounds nuw i8, ptr %14, i16 %16
  %18 = load i8, ptr %17, align 1, !tbaa !7
  %19 = zext i8 %18 to i16
  %20 = load i16, ptr %5, align 2, !tbaa !8
  %21 = add i16 %20, %19
  store i16 %21, ptr %5, align 2, !tbaa !8
  br label %22

22:                                               ; preds = %13
  %23 = load i8, ptr %6, align 1, !tbaa !7
  %24 = add i8 %23, 1
  store i8 %24, ptr %6, align 1, !tbaa !7
  br label %7, !llvm.loop !10

25:                                               ; preds = %7
  %26 = load i16, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(i64 1, ptr %6) #2
  call void @llvm.lifetime.end.p0(i64 2, ptr %5) #2
  ret i16 %26
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
!3 = !{!"p1 omnipotent char", !4, i64 0}
!4 = !{!"any pointer", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!5, !5, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"int", !5, i64 0}
!10 = distinct !{!10, !11, !12}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!"llvm.loop.unroll.disable"}
