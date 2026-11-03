source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

@buf = external dso_local global [0 x i8], align 1

define dso_local zeroext i8 @find(i8 noundef zeroext %0) #0 {
  %2 = alloca i8, align 1
  %3 = alloca i8, align 1
  store i8 %0, ptr %2, align 1, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 1, ptr %3) #2
  store i8 0, ptr %3, align 1, !tbaa !2
  br label %4

4:                                                ; preds = %13, %1
  %5 = load i8, ptr %3, align 1, !tbaa !2
  %6 = zext i8 %5 to i16
  %7 = getelementptr inbounds nuw [0 x i8], ptr @buf, i16 0, i16 %6
  %8 = load i8, ptr %7, align 1, !tbaa !2
  %9 = zext i8 %8 to i16
  %10 = load i8, ptr %2, align 1, !tbaa !2
  %11 = zext i8 %10 to i16
  %12 = icmp ne i16 %9, %11
  br i1 %12, label %13, label %16

13:                                               ; preds = %4
  %14 = load i8, ptr %3, align 1, !tbaa !2
  %15 = add i8 %14, 1
  store i8 %15, ptr %3, align 1, !tbaa !2
  br label %4, !llvm.loop !5

16:                                               ; preds = %4
  %17 = load i8, ptr %3, align 1, !tbaa !2
  call void @llvm.lifetime.end.p0(i64 1, ptr %3) #2
  ret i8 %17
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
