source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @dotx(ptr noundef %0, ptr noundef %1, i8 noundef zeroext %2) #0 {
  %4 = alloca ptr, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  store ptr %0, ptr %4, align 2, !tbaa !2
  store ptr %1, ptr %5, align 2, !tbaa !2
  store i8 %2, ptr %6, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %7) #2
  store i8 0, ptr %7, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %8) #2
  store i8 0, ptr %8, align 1, !tbaa !7
  br label %9

9:                                                ; preds = %33, %3
  %10 = load i8, ptr %8, align 1, !tbaa !7
  %11 = zext i8 %10 to i16
  %12 = load i8, ptr %6, align 1, !tbaa !7
  %13 = zext i8 %12 to i16
  %14 = icmp slt i16 %11, %13
  br i1 %14, label %15, label %36

15:                                               ; preds = %9
  %16 = load ptr, ptr %4, align 2, !tbaa !2
  %17 = load i8, ptr %8, align 1, !tbaa !7
  %18 = zext i8 %17 to i16
  %19 = getelementptr inbounds nuw i8, ptr %16, i16 %18
  %20 = load i8, ptr %19, align 1, !tbaa !7
  %21 = zext i8 %20 to i16
  %22 = load ptr, ptr %5, align 2, !tbaa !2
  %23 = load i8, ptr %8, align 1, !tbaa !7
  %24 = zext i8 %23 to i16
  %25 = getelementptr inbounds nuw i8, ptr %22, i16 %24
  %26 = load i8, ptr %25, align 1, !tbaa !7
  %27 = zext i8 %26 to i16
  %28 = xor i16 %21, %27
  %29 = load i8, ptr %7, align 1, !tbaa !7
  %30 = zext i8 %29 to i16
  %31 = add nsw i16 %30, %28
  %32 = trunc i16 %31 to i8
  store i8 %32, ptr %7, align 1, !tbaa !7
  br label %33

33:                                               ; preds = %15
  %34 = load i8, ptr %8, align 1, !tbaa !7
  %35 = add i8 %34, 1
  store i8 %35, ptr %8, align 1, !tbaa !7
  br label %9, !llvm.loop !8

36:                                               ; preds = %9
  %37 = load i8, ptr %7, align 1, !tbaa !7
  call void @llvm.lifetime.end.p0(i64 1, ptr %8) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %7) #2
  ret i8 %37
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
