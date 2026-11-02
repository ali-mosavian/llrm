source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @bsearch8(ptr noundef %0, i8 noundef zeroext %1, i8 noundef zeroext %2) #0 {
  %4 = alloca ptr, align 2
  %5 = alloca i8, align 1
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  %9 = alloca i8, align 1
  store ptr %0, ptr %4, align 2, !tbaa !2
  store i8 %1, ptr %5, align 1, !tbaa !7
  store i8 %2, ptr %6, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %7) #2
  store i8 0, ptr %7, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %8) #2
  %10 = load i8, ptr %5, align 1, !tbaa !7
  store i8 %10, ptr %8, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %9) #2
  br label %11

11:                                               ; preds = %41, %3
  %12 = load i8, ptr %7, align 1, !tbaa !7
  %13 = zext i8 %12 to i16
  %14 = load i8, ptr %8, align 1, !tbaa !7
  %15 = zext i8 %14 to i16
  %16 = icmp slt i16 %13, %15
  br i1 %16, label %17, label %42

17:                                               ; preds = %11
  %18 = load i8, ptr %7, align 1, !tbaa !7
  %19 = zext i8 %18 to i16
  %20 = load i8, ptr %8, align 1, !tbaa !7
  %21 = zext i8 %20 to i16
  %22 = add nsw i16 %19, %21
  %23 = ashr i16 %22, 1
  %24 = trunc i16 %23 to i8
  store i8 %24, ptr %9, align 1, !tbaa !7
  %25 = load ptr, ptr %4, align 2, !tbaa !2
  %26 = load i8, ptr %9, align 1, !tbaa !7
  %27 = zext i8 %26 to i16
  %28 = getelementptr inbounds nuw i8, ptr %25, i16 %27
  %29 = load i8, ptr %28, align 1, !tbaa !7
  %30 = zext i8 %29 to i16
  %31 = load i8, ptr %6, align 1, !tbaa !7
  %32 = zext i8 %31 to i16
  %33 = icmp slt i16 %30, %32
  br i1 %33, label %34, label %39

34:                                               ; preds = %17
  %35 = load i8, ptr %9, align 1, !tbaa !7
  %36 = zext i8 %35 to i16
  %37 = add nsw i16 %36, 1
  %38 = trunc i16 %37 to i8
  store i8 %38, ptr %7, align 1, !tbaa !7
  br label %41

39:                                               ; preds = %17
  %40 = load i8, ptr %9, align 1, !tbaa !7
  store i8 %40, ptr %8, align 1, !tbaa !7
  br label %41

41:                                               ; preds = %39, %34
  br label %11, !llvm.loop !8

42:                                               ; preds = %11
  %43 = load i8, ptr %7, align 1, !tbaa !7
  call void @llvm.lifetime.end.p0(i64 1, ptr %9) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %8) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %7) #2
  ret i8 %43
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
