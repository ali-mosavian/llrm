source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @reverse(ptr noundef %0, i8 noundef zeroext %1) #0 {
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
  %8 = load i8, ptr %4, align 1, !tbaa !7
  %9 = zext i8 %8 to i16
  %10 = sub nsw i16 %9, 1
  %11 = trunc i16 %10 to i8
  store i8 %11, ptr %6, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %7) #2
  br label %12

12:                                               ; preds = %18, %2
  %13 = load i8, ptr %5, align 1, !tbaa !7
  %14 = zext i8 %13 to i16
  %15 = load i8, ptr %6, align 1, !tbaa !7
  %16 = zext i8 %15 to i16
  %17 = icmp slt i16 %14, %16
  br i1 %17, label %18, label %42

18:                                               ; preds = %12
  %19 = load ptr, ptr %3, align 2, !tbaa !2
  %20 = load i8, ptr %5, align 1, !tbaa !7
  %21 = zext i8 %20 to i16
  %22 = getelementptr inbounds nuw i8, ptr %19, i16 %21
  %23 = load i8, ptr %22, align 1, !tbaa !7
  store i8 %23, ptr %7, align 1, !tbaa !7
  %24 = load ptr, ptr %3, align 2, !tbaa !2
  %25 = load i8, ptr %6, align 1, !tbaa !7
  %26 = zext i8 %25 to i16
  %27 = getelementptr inbounds nuw i8, ptr %24, i16 %26
  %28 = load i8, ptr %27, align 1, !tbaa !7
  %29 = load ptr, ptr %3, align 2, !tbaa !2
  %30 = load i8, ptr %5, align 1, !tbaa !7
  %31 = zext i8 %30 to i16
  %32 = getelementptr inbounds nuw i8, ptr %29, i16 %31
  store i8 %28, ptr %32, align 1, !tbaa !7
  %33 = load i8, ptr %7, align 1, !tbaa !7
  %34 = load ptr, ptr %3, align 2, !tbaa !2
  %35 = load i8, ptr %6, align 1, !tbaa !7
  %36 = zext i8 %35 to i16
  %37 = getelementptr inbounds nuw i8, ptr %34, i16 %36
  store i8 %33, ptr %37, align 1, !tbaa !7
  %38 = load i8, ptr %5, align 1, !tbaa !7
  %39 = add i8 %38, 1
  store i8 %39, ptr %5, align 1, !tbaa !7
  %40 = load i8, ptr %6, align 1, !tbaa !7
  %41 = add i8 %40, -1
  store i8 %41, ptr %6, align 1, !tbaa !7
  br label %12, !llvm.loop !8

42:                                               ; preds = %12
  %43 = load ptr, ptr %3, align 2, !tbaa !2
  %44 = getelementptr inbounds i8, ptr %43, i16 0
  %45 = load i8, ptr %44, align 1, !tbaa !7
  call void @llvm.lifetime.end.p0(i64 1, ptr %7) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %6) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %5) #2
  ret i8 %45
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
