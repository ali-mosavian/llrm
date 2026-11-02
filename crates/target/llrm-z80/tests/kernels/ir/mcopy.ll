source_filename = "kernels.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

define dso_local zeroext i8 @mcopy(ptr noundef %0, ptr noundef %1, i8 noundef zeroext %2) #0 {
  %4 = alloca ptr, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  store ptr %0, ptr %4, align 2, !tbaa !2
  store ptr %1, ptr %5, align 2, !tbaa !2
  store i8 %2, ptr %6, align 1, !tbaa !7
  call void @llvm.lifetime.start.p0(i64 1, ptr %7) #2
  store i8 0, ptr %7, align 1, !tbaa !7
  br label %8

8:                                                ; preds = %24, %3
  %9 = load i8, ptr %7, align 1, !tbaa !7
  %10 = zext i8 %9 to i16
  %11 = load i8, ptr %6, align 1, !tbaa !7
  %12 = zext i8 %11 to i16
  %13 = icmp slt i16 %10, %12
  br i1 %13, label %14, label %27

14:                                               ; preds = %8
  %15 = load ptr, ptr %5, align 2, !tbaa !2
  %16 = load i8, ptr %7, align 1, !tbaa !7
  %17 = zext i8 %16 to i16
  %18 = getelementptr inbounds nuw i8, ptr %15, i16 %17
  %19 = load i8, ptr %18, align 1, !tbaa !7
  %20 = load ptr, ptr %4, align 2, !tbaa !2
  %21 = load i8, ptr %7, align 1, !tbaa !7
  %22 = zext i8 %21 to i16
  %23 = getelementptr inbounds nuw i8, ptr %20, i16 %22
  store i8 %19, ptr %23, align 1, !tbaa !7
  br label %24

24:                                               ; preds = %14
  %25 = load i8, ptr %7, align 1, !tbaa !7
  %26 = add i8 %25, 1
  store i8 %26, ptr %7, align 1, !tbaa !7
  br label %8, !llvm.loop !8

27:                                               ; preds = %8
  %28 = load ptr, ptr %4, align 2, !tbaa !2
  %29 = load i8, ptr %6, align 1, !tbaa !7
  %30 = zext i8 %29 to i16
  %31 = sub nsw i16 %30, 1
  %32 = getelementptr inbounds i8, ptr %28, i16 %31
  %33 = load i8, ptr %32, align 1, !tbaa !7
  call void @llvm.lifetime.end.p0(i64 1, ptr %7) #2
  ret i8 %33
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
