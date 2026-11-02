source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

@buf = external dso_local global [0 x i8], align 1

define dso_local zeroext i8 @fill(i8 noundef zeroext %0) #0 {
  %2 = alloca i8, align 1
  %3 = alloca i8, align 1
  store i8 %0, ptr %2, align 1, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 1, ptr %3) #2
  store i8 0, ptr %3, align 1, !tbaa !2
  br label %4

4:                                                ; preds = %15, %1
  %5 = load i8, ptr %3, align 1, !tbaa !2
  %6 = zext i8 %5 to i16
  %7 = load i8, ptr %2, align 1, !tbaa !2
  %8 = zext i8 %7 to i16
  %9 = icmp slt i16 %6, %8
  br i1 %9, label %10, label %18

10:                                               ; preds = %4
  %11 = load i8, ptr %3, align 1, !tbaa !2
  %12 = load i8, ptr %3, align 1, !tbaa !2
  %13 = zext i8 %12 to i16
  %14 = getelementptr inbounds nuw [0 x i8], ptr @buf, i16 0, i16 %13
  store i8 %11, ptr %14, align 1, !tbaa !2
  br label %15

15:                                               ; preds = %10
  %16 = load i8, ptr %3, align 1, !tbaa !2
  %17 = add i8 %16, 1
  store i8 %17, ptr %3, align 1, !tbaa !2
  br label %4, !llvm.loop !5

18:                                               ; preds = %4
  %19 = load i8, ptr %2, align 1, !tbaa !2
  %20 = zext i8 %19 to i16
  %21 = sub nsw i16 %20, 1
  %22 = getelementptr inbounds [0 x i8], ptr @buf, i16 0, i16 %21
  %23 = load i8, ptr %22, align 1, !tbaa !2
  call void @llvm.lifetime.end.p0(i64 1, ptr %3) #2
  ret i8 %23
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
!5 = distinct !{!5, !6, !7}
!6 = !{!"llvm.loop.mustprogress"}
!7 = !{!"llvm.loop.unroll.disable"}
