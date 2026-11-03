source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @schr(ptr noundef %0, i8 noundef zeroext %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i8, align 1
  %5 = alloca ptr, align 2
  store ptr %0, ptr %3, align 2, !tbaa !2
  store i8 %1, ptr %4, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 2, ptr %5) #2
  %6 = load ptr, ptr %3, align 2, !tbaa !2
  store ptr %6, ptr %5, align 2, !tbaa !2
  br label %7

7:                                                ; preds = %21, %2
  %8 = load ptr, ptr %5, align 2, !tbaa !2
  %9 = load i8, ptr %8, align 1, !tbaa !7
  %10 = zext i8 %9 to i16
  %11 = icmp ne i16 %10, 0
  br i1 %11, label %12, label %19

12:                                               ; preds = %7
  %13 = load ptr, ptr %5, align 2, !tbaa !2
  %14 = load i8, ptr %13, align 1, !tbaa !7
  %15 = zext i8 %14 to i16
  %16 = load i8, ptr %4, align 1, !tbaa !7
  %17 = zext i8 %16 to i16
  %18 = icmp ne i16 %15, %17
  br label %19

19:                                               ; preds = %12, %7
  %20 = phi i1 [ false, %7 ], [ %18, %12 ]
  br i1 %20, label %21, label %24

21:                                               ; preds = %19
  %22 = load ptr, ptr %5, align 2, !tbaa !2
  %23 = getelementptr inbounds nuw i8, ptr %22, i32 1
  store ptr %23, ptr %5, align 2, !tbaa !2
  br label %7, !llvm.loop !8

24:                                               ; preds = %19
  %25 = load ptr, ptr %5, align 2, !tbaa !2
  %26 = load i8, ptr %25, align 1, !tbaa !7
  %27 = zext i8 %26 to i16
  %28 = icmp ne i16 %27, 0
  br i1 %28, label %29, label %35

29:                                               ; preds = %24
  %30 = load ptr, ptr %5, align 2, !tbaa !2
  %31 = load ptr, ptr %3, align 2, !tbaa !2
  %32 = ptrtoint ptr %30 to i16
  %33 = ptrtoint ptr %31 to i16
  %34 = sub i16 %32, %33
  br label %36

35:                                               ; preds = %24
  br label %36

36:                                               ; preds = %35, %29
  %37 = phi i16 [ %34, %29 ], [ 255, %35 ]
  %38 = trunc i16 %37 to i8
  call void @llvm.lifetime.end.p0(i64 2, ptr %5) #2
  ret i8 %38
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
!8 = distinct !{!8, !9, !10}
!9 = !{!"llvm.loop.mustprogress"}
!10 = !{!"llvm.loop.unroll.disable"}
