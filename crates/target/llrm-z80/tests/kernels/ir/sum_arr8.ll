source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@buf = external dso_local global [0 x i8], align 1

define dso_local zeroext i8 @sum_arr(i8 noundef zeroext %0) #0 {
  %2 = alloca i8, align 1
  %3 = alloca i8, align 1
  %4 = alloca i8, align 1
  store i8 %0, ptr %2, align 1, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 1, ptr %3) #2
  store i8 0, ptr %3, align 1, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 1, ptr %4) #2
  store i8 0, ptr %4, align 1, !tbaa !2
  br label %5

5:                                                ; preds = %21, %1
  %6 = load i8, ptr %4, align 1, !tbaa !2
  %7 = zext i8 %6 to i16
  %8 = load i8, ptr %2, align 1, !tbaa !2
  %9 = zext i8 %8 to i16
  %10 = icmp slt i16 %7, %9
  br i1 %10, label %11, label %24

11:                                               ; preds = %5
  %12 = load i8, ptr %4, align 1, !tbaa !2
  %13 = zext i8 %12 to i16
  %14 = getelementptr inbounds nuw [0 x i8], ptr @buf, i16 0, i16 %13
  %15 = load i8, ptr %14, align 1, !tbaa !2
  %16 = zext i8 %15 to i16
  %17 = load i8, ptr %3, align 1, !tbaa !2
  %18 = zext i8 %17 to i16
  %19 = add nsw i16 %18, %16
  %20 = trunc i16 %19 to i8
  store i8 %20, ptr %3, align 1, !tbaa !2
  br label %21

21:                                               ; preds = %11
  %22 = load i8, ptr %4, align 1, !tbaa !2
  %23 = add i8 %22, 1
  store i8 %23, ptr %4, align 1, !tbaa !2
  br label %5, !llvm.loop !5

24:                                               ; preds = %5
  %25 = load i8, ptr %3, align 1, !tbaa !2
  call void @llvm.lifetime.end.p0(i64 1, ptr %4) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %3) #2
  ret i8 %25
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
