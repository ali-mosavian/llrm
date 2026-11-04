source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local i16 @fletcher(ptr noundef %0, i8 noundef zeroext %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i8, align 1
  %5 = alloca i8, align 1
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  store ptr %0, ptr %3, align 2, !tbaa !2
  store i8 %1, ptr %4, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %5) #2
  store i8 0, ptr %5, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %6) #2
  store i8 0, ptr %6, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %7) #2
  store i8 0, ptr %7, align 1, !tbaa !7
  br label %8

8:                                                ; preds = %49, %2
  %9 = load i8, ptr %7, align 1, !tbaa !7
  %10 = zext i8 %9 to i16
  %11 = load i8, ptr %4, align 1, !tbaa !7
  %12 = zext i8 %11 to i16
  %13 = icmp slt i16 %10, %12
  br i1 %13, label %14, label %52

14:                                               ; preds = %8
  %15 = load ptr, ptr %3, align 2, !tbaa !2
  %16 = load i8, ptr %7, align 1, !tbaa !7
  %17 = zext i8 %16 to i16
  %18 = getelementptr inbounds nuw i8, ptr %15, i16 %17
  %19 = load i8, ptr %18, align 1, !tbaa !7
  %20 = zext i8 %19 to i16
  %21 = load i8, ptr %5, align 1, !tbaa !7
  %22 = zext i8 %21 to i16
  %23 = add nsw i16 %22, %20
  %24 = trunc i16 %23 to i8
  store i8 %24, ptr %5, align 1, !tbaa !7
  %25 = load i8, ptr %5, align 1, !tbaa !7
  %26 = zext i8 %25 to i16
  %27 = icmp sge i16 %26, 255
  br i1 %27, label %28, label %33

28:                                               ; preds = %14
  %29 = load i8, ptr %5, align 1, !tbaa !7
  %30 = zext i8 %29 to i16
  %31 = sub nsw i16 %30, 255
  %32 = trunc i16 %31 to i8
  store i8 %32, ptr %5, align 1, !tbaa !7
  br label %33

33:                                               ; preds = %28, %14
  %34 = load i8, ptr %5, align 1, !tbaa !7
  %35 = zext i8 %34 to i16
  %36 = load i8, ptr %6, align 1, !tbaa !7
  %37 = zext i8 %36 to i16
  %38 = add nsw i16 %37, %35
  %39 = trunc i16 %38 to i8
  store i8 %39, ptr %6, align 1, !tbaa !7
  %40 = load i8, ptr %6, align 1, !tbaa !7
  %41 = zext i8 %40 to i16
  %42 = icmp sge i16 %41, 255
  br i1 %42, label %43, label %48

43:                                               ; preds = %33
  %44 = load i8, ptr %6, align 1, !tbaa !7
  %45 = zext i8 %44 to i16
  %46 = sub nsw i16 %45, 255
  %47 = trunc i16 %46 to i8
  store i8 %47, ptr %6, align 1, !tbaa !7
  br label %48

48:                                               ; preds = %43, %33
  br label %49

49:                                               ; preds = %48
  %50 = load i8, ptr %7, align 1, !tbaa !7
  %51 = add i8 %50, 1
  store i8 %51, ptr %7, align 1, !tbaa !7
  br label %8, !llvm.loop !8

52:                                               ; preds = %8
  %53 = load i8, ptr %6, align 1, !tbaa !7
  %54 = zext i8 %53 to i16
  %55 = shl i16 %54, 8
  %56 = load i8, ptr %5, align 1, !tbaa !7
  %57 = zext i8 %56 to i16
  %58 = or i16 %55, %57
  call void @llvm.lifetime.end.p0(i64 1, ptr %7) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %6) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %5) #2
  ret i16 %58
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
