source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local i16 @crc16(ptr noundef %0, i8 noundef zeroext %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i8, align 1
  %5 = alloca i16, align 2
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  store ptr %0, ptr %3, align 2, !tbaa !2
  store i8 %1, ptr %4, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 2, ptr %5) #2
  store i16 -1, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(i64 1, ptr %6) #2
  call void @llvm.lifetime.start.p0(i64 1, ptr %7) #2
  store i8 0, ptr %6, align 1, !tbaa !7
  br label %8

8:                                                ; preds = %45, %2
  %9 = load i8, ptr %6, align 1, !tbaa !7
  %10 = zext i8 %9 to i16
  %11 = load i8, ptr %4, align 1, !tbaa !7
  %12 = zext i8 %11 to i16
  %13 = icmp slt i16 %10, %12
  br i1 %13, label %14, label %48

14:                                               ; preds = %8
  %15 = load ptr, ptr %3, align 2, !tbaa !2
  %16 = load i8, ptr %6, align 1, !tbaa !7
  %17 = zext i8 %16 to i16
  %18 = getelementptr inbounds nuw i8, ptr %15, i16 %17
  %19 = load i8, ptr %18, align 1, !tbaa !7
  %20 = zext i8 %19 to i16
  %21 = shl i16 %20, 8
  %22 = load i16, ptr %5, align 2, !tbaa !8
  %23 = xor i16 %22, %21
  store i16 %23, ptr %5, align 2, !tbaa !8
  store i8 0, ptr %7, align 1, !tbaa !7
  br label %24

24:                                               ; preds = %41, %14
  %25 = load i8, ptr %7, align 1, !tbaa !7
  %26 = zext i8 %25 to i16
  %27 = icmp slt i16 %26, 8
  br i1 %27, label %28, label %44

28:                                               ; preds = %24
  %29 = load i16, ptr %5, align 2, !tbaa !8
  %30 = and i16 %29, -32768
  %31 = icmp ne i16 %30, 0
  br i1 %31, label %32, label %36

32:                                               ; preds = %28
  %33 = load i16, ptr %5, align 2, !tbaa !8
  %34 = shl i16 %33, 1
  %35 = xor i16 %34, 4129
  br label %39

36:                                               ; preds = %28
  %37 = load i16, ptr %5, align 2, !tbaa !8
  %38 = shl i16 %37, 1
  br label %39

39:                                               ; preds = %36, %32
  %40 = phi i16 [ %35, %32 ], [ %38, %36 ]
  store i16 %40, ptr %5, align 2, !tbaa !8
  br label %41

41:                                               ; preds = %39
  %42 = load i8, ptr %7, align 1, !tbaa !7
  %43 = add i8 %42, 1
  store i8 %43, ptr %7, align 1, !tbaa !7
  br label %24, !llvm.loop !10

44:                                               ; preds = %24
  br label %45

45:                                               ; preds = %44
  %46 = load i8, ptr %6, align 1, !tbaa !7
  %47 = add i8 %46, 1
  store i8 %47, ptr %6, align 1, !tbaa !7
  br label %8, !llvm.loop !13

48:                                               ; preds = %8
  %49 = load i16, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(i64 1, ptr %7) #2
  call void @llvm.lifetime.end.p0(i64 1, ptr %6) #2
  call void @llvm.lifetime.end.p0(i64 2, ptr %5) #2
  ret i16 %49
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
!8 = !{!9, !9, i64 0}
!9 = !{!"int", !5, i64 0}
!10 = distinct !{!10, !11, !12}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!"llvm.loop.unroll.disable"}
!13 = distinct !{!13, !11, !12}
