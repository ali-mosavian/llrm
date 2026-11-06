source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @minmax(ptr noundef %0, i8 noundef zeroext %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i8, align 1
  %5 = alloca i8, align 1
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  store ptr %0, ptr %3, align 2, !tbaa !2
  store i8 %1, ptr %4, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %5) #2
  store i8 -1, ptr %5, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %6) #2
  store i8 0, ptr %6, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %7) #2
  store i8 0, ptr %7, align 1, !tbaa !7
  br label %9

9:                                                ; preds = %37, %2
  %10 = load i8, ptr %7, align 1, !tbaa !7
  %11 = zext i8 %10 to i16
  %12 = load i8, ptr %4, align 1, !tbaa !7
  %13 = zext i8 %12 to i16
  %14 = icmp slt i16 %11, %13
  br i1 %14, label %15, label %40

15:                                               ; preds = %9
  call void @llvm.lifetime.start.p0(i64 1, ptr %8) #2
  %16 = load ptr, ptr %3, align 2, !tbaa !2
  %17 = load i8, ptr %7, align 1, !tbaa !7
  %18 = zext i8 %17 to i16
  %19 = getelementptr inbounds nuw i8, ptr %16, i16 %18
  %20 = load i8, ptr %19, align 1, !tbaa !7
  store i8 %20, ptr %8, align 1, !tbaa !7
  %21 = load i8, ptr %8, align 1, !tbaa !7
  %22 = zext i8 %21 to i16
  %23 = load i8, ptr %5, align 1, !tbaa !7
  %24 = zext i8 %23 to i16
  %25 = icmp slt i16 %22, %24
  br i1 %25, label %26, label %28

26:                                               ; preds = %15
  %27 = load i8, ptr %8, align 1, !tbaa !7
  store i8 %27, ptr %5, align 1, !tbaa !7
  br label %28

28:                                               ; preds = %26, %15
  %29 = load i8, ptr %8, align 1, !tbaa !7
  %30 = zext i8 %29 to i16
  %31 = load i8, ptr %6, align 1, !tbaa !7
  %32 = zext i8 %31 to i16
  %33 = icmp sgt i16 %30, %32
  br i1 %33, label %34, label %36

34:                                               ; preds = %28
  %35 = load i8, ptr %8, align 1, !tbaa !7
  store i8 %35, ptr %6, align 1, !tbaa !7
  br label %36

36:                                               ; preds = %34, %28
  call void @llvm.lifetime.end.p0(i64 1, ptr %8) #2
  br label %37

37:                                               ; preds = %36
  %38 = load i8, ptr %7, align 1, !tbaa !7
  %39 = add i8 %38, 1
  store i8 %39, ptr %7, align 1, !tbaa !7
  br label %9, !llvm.loop !8

40:                                               ; preds = %9
  %41 = load i8, ptr %6, align 1, !tbaa !7
  %42 = zext i8 %41 to i16
  %43 = load i8, ptr %5, align 1, !tbaa !7
  %44 = zext i8 %43 to i16
  %45 = sub nsw i16 %42, %44
  %46 = trunc i16 %45 to i8
  call void @llvm.lifetime.end.p0(i64 1, ptr %7) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %6) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %5) #2
  ret i8 %46
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
!8 = distinct !{!8, !9}
!9 = !{!"llvm.loop.mustprogress"}
