source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

@buf = external dso_local global [0 x i8], align 1

define dso_local zeroext i8 @sum4() #0 {
  %1 = alloca i8, align 1
  %2 = alloca i8, align 1
  call void @llvm.lifetime.start.p0(i64 1, ptr %1) #2
  store i8 0, ptr %1, align 1, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 1, ptr %2) #2
  store i8 0, ptr %2, align 1, !tbaa !2
  br label %3

3:                                                ; preds = %17, %0
  %4 = load i8, ptr %2, align 1, !tbaa !2
  %5 = zext i8 %4 to i16
  %6 = icmp slt i16 %5, 4
  br i1 %6, label %7, label %20

7:                                                ; preds = %3
  %8 = load i8, ptr %2, align 1, !tbaa !2
  %9 = zext i8 %8 to i16
  %10 = getelementptr inbounds nuw [0 x i8], ptr @buf, i16 0, i16 %9
  %11 = load i8, ptr %10, align 1, !tbaa !2
  %12 = zext i8 %11 to i16
  %13 = load i8, ptr %1, align 1, !tbaa !2
  %14 = zext i8 %13 to i16
  %15 = add nsw i16 %14, %12
  %16 = trunc i16 %15 to i8
  store i8 %16, ptr %1, align 1, !tbaa !2
  br label %17

17:                                               ; preds = %7
  %18 = load i8, ptr %2, align 1, !tbaa !2
  %19 = add i8 %18, 1
  store i8 %19, ptr %2, align 1, !tbaa !2
  br label %3, !llvm.loop !5

20:                                               ; preds = %3
  %21 = load i8, ptr %1, align 1, !tbaa !2
  call void @llvm.lifetime.end.p0(i64 1, ptr %2) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %1) #2
  ret i8 %21
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
