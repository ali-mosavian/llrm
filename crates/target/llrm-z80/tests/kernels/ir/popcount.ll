source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @popcount(i8 noundef zeroext %0) #0 {
  %2 = alloca i8, align 1
  %3 = alloca i8, align 1
  store i8 %0, ptr %2, align 1, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 1, ptr %3) #2
  store i8 0, ptr %3, align 1, !tbaa !2
  br label %4

4:                                                ; preds = %7, %1
  %5 = load i8, ptr %2, align 1, !tbaa !2
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %7, label %19

7:                                                ; preds = %4
  %8 = load i8, ptr %2, align 1, !tbaa !2
  %9 = zext i8 %8 to i16
  %10 = and i16 %9, 1
  %11 = load i8, ptr %3, align 1, !tbaa !2
  %12 = zext i8 %11 to i16
  %13 = add nsw i16 %12, %10
  %14 = trunc i16 %13 to i8
  store i8 %14, ptr %3, align 1, !tbaa !2
  %15 = load i8, ptr %2, align 1, !tbaa !2
  %16 = zext i8 %15 to i16
  %17 = ashr i16 %16, 1
  %18 = trunc i16 %17 to i8
  store i8 %18, ptr %2, align 1, !tbaa !2
  br label %4, !llvm.loop !5

19:                                               ; preds = %4
  %20 = load i8, ptr %3, align 1, !tbaa !2
  call void @llvm.lifetime.end.p0(i64 1, ptr %3) #2
  ret i8 %20
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
