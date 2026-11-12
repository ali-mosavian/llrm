target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

@sum = internal global i32 0, align 2

; Function Attrs: noinline nounwind optsize
define dso_local i32 @c3(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  store ptr %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %5) #2
  store i32 0, ptr %5, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %6) #2
  store i32 0, ptr %6, align 2, !tbaa !10
  br label %10

10:                                               ; preds = %44, %2
  %11 = load i32, ptr %6, align 2, !tbaa !10
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %47

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #2
  %14 = load ptr, ptr %3, align 2, !tbaa !7
  %15 = load i32, ptr %6, align 2, !tbaa !10
  %16 = trunc i32 %15 to i16
  %17 = getelementptr inbounds i32, ptr %14, i16 %16
  %18 = load i32, ptr %17, align 2, !tbaa !10
  store i32 %18, ptr %7, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %8) #2
  %19 = load i32, ptr %7, align 2, !tbaa !10
  %20 = udiv i32 %19, 3
  store i32 %20, ptr %8, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %9) #2
  %21 = load i32, ptr %7, align 2, !tbaa !10
  %22 = urem i32 %21, 3
  store i32 %22, ptr %9, align 2, !tbaa !10
  %23 = load i32, ptr %8, align 2, !tbaa !10
  %24 = load i32, ptr %7, align 2, !tbaa !10
  %25 = load i32, ptr %4, align 2, !tbaa !10
  %26 = udiv i32 %24, %25
  %27 = icmp ne i32 %23, %26
  br i1 %27, label %34, label %28

28:                                               ; preds = %13
  %29 = load i32, ptr %9, align 2, !tbaa !10
  %30 = load i32, ptr %7, align 2, !tbaa !10
  %31 = load i32, ptr %4, align 2, !tbaa !10
  %32 = urem i32 %30, %31
  %33 = icmp ne i32 %29, %32
  br i1 %33, label %34, label %37

34:                                               ; preds = %28, %13
  %35 = load i32, ptr %5, align 2, !tbaa !10
  %36 = add nsw i32 %35, 1
  store i32 %36, ptr %5, align 2, !tbaa !10
  br label %37

37:                                               ; preds = %34, %28
  %38 = load i32, ptr %8, align 2, !tbaa !10
  %39 = mul i32 %38, 31
  %40 = load i32, ptr %9, align 2, !tbaa !10
  %41 = add i32 %39, %40
  %42 = load i32, ptr @sum, align 2, !tbaa !10
  %43 = add i32 %42, %41
  store i32 %43, ptr @sum, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %9) #2
  call void @llvm.lifetime.end.p0(ptr %8) #2
  call void @llvm.lifetime.end.p0(ptr %7) #2
  br label %44

44:                                               ; preds = %37
  %45 = load i32, ptr %6, align 2, !tbaa !10
  %46 = add nsw i32 %45, 1
  store i32 %46, ptr %6, align 2, !tbaa !10
  br label %10, !llvm.loop !12

47:                                               ; preds = %10
  %48 = load i32, ptr %5, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %6) #2
  call void @llvm.lifetime.end.p0(ptr %5) #2
  ret i32 %48
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: noinline nounwind optsize
define dso_local i32 @c5(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  store ptr %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %5) #2
  store i32 0, ptr %5, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %6) #2
  store i32 0, ptr %6, align 2, !tbaa !10
  br label %10

10:                                               ; preds = %44, %2
  %11 = load i32, ptr %6, align 2, !tbaa !10
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %47

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #2
  %14 = load ptr, ptr %3, align 2, !tbaa !7
  %15 = load i32, ptr %6, align 2, !tbaa !10
  %16 = trunc i32 %15 to i16
  %17 = getelementptr inbounds i32, ptr %14, i16 %16
  %18 = load i32, ptr %17, align 2, !tbaa !10
  store i32 %18, ptr %7, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %8) #2
  %19 = load i32, ptr %7, align 2, !tbaa !10
  %20 = udiv i32 %19, 5
  store i32 %20, ptr %8, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %9) #2
  %21 = load i32, ptr %7, align 2, !tbaa !10
  %22 = urem i32 %21, 5
  store i32 %22, ptr %9, align 2, !tbaa !10
  %23 = load i32, ptr %8, align 2, !tbaa !10
  %24 = load i32, ptr %7, align 2, !tbaa !10
  %25 = load i32, ptr %4, align 2, !tbaa !10
  %26 = udiv i32 %24, %25
  %27 = icmp ne i32 %23, %26
  br i1 %27, label %34, label %28

28:                                               ; preds = %13
  %29 = load i32, ptr %9, align 2, !tbaa !10
  %30 = load i32, ptr %7, align 2, !tbaa !10
  %31 = load i32, ptr %4, align 2, !tbaa !10
  %32 = urem i32 %30, %31
  %33 = icmp ne i32 %29, %32
  br i1 %33, label %34, label %37

34:                                               ; preds = %28, %13
  %35 = load i32, ptr %5, align 2, !tbaa !10
  %36 = add nsw i32 %35, 1
  store i32 %36, ptr %5, align 2, !tbaa !10
  br label %37

37:                                               ; preds = %34, %28
  %38 = load i32, ptr %8, align 2, !tbaa !10
  %39 = mul i32 %38, 31
  %40 = load i32, ptr %9, align 2, !tbaa !10
  %41 = add i32 %39, %40
  %42 = load i32, ptr @sum, align 2, !tbaa !10
  %43 = add i32 %42, %41
  store i32 %43, ptr @sum, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %9) #2
  call void @llvm.lifetime.end.p0(ptr %8) #2
  call void @llvm.lifetime.end.p0(ptr %7) #2
  br label %44

44:                                               ; preds = %37
  %45 = load i32, ptr %6, align 2, !tbaa !10
  %46 = add nsw i32 %45, 1
  store i32 %46, ptr %6, align 2, !tbaa !10
  br label %10, !llvm.loop !14

47:                                               ; preds = %10
  %48 = load i32, ptr %5, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %6) #2
  call void @llvm.lifetime.end.p0(ptr %5) #2
  ret i32 %48
}

; Function Attrs: noinline nounwind optsize
define dso_local i32 @c7(ptr noundef %0, i32 noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  store ptr %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %5) #2
  store i32 0, ptr %5, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %6) #2
  store i32 0, ptr %6, align 2, !tbaa !10
  br label %10

10:                                               ; preds = %44, %2
  %11 = load i32, ptr %6, align 2, !tbaa !10
  %12 = icmp slt i32 %11, 1500
  br i1 %12, label %13, label %47

13:                                               ; preds = %10
  call void @llvm.lifetime.start.p0(ptr %7) #2
  %14 = load ptr, ptr %3, align 2, !tbaa !7
  %15 = load i32, ptr %6, align 2, !tbaa !10
  %16 = trunc i32 %15 to i16
  %17 = getelementptr inbounds i32, ptr %14, i16 %16
  %18 = load i32, ptr %17, align 2, !tbaa !10
  store i32 %18, ptr %7, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %8) #2
  %19 = load i32, ptr %7, align 2, !tbaa !10
  %20 = udiv i32 %19, 7
  store i32 %20, ptr %8, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %9) #2
  %21 = load i32, ptr %7, align 2, !tbaa !10
  %22 = urem i32 %21, 7
  store i32 %22, ptr %9, align 2, !tbaa !10
  %23 = load i32, ptr %8, align 2, !tbaa !10
  %24 = load i32, ptr %7, align 2, !tbaa !10
  %25 = load i32, ptr %4, align 2, !tbaa !10
  %26 = udiv i32 %24, %25
  %27 = icmp ne i32 %23, %26
  br i1 %27, label %34, label %28

28:                                               ; preds = %13
  %29 = load i32, ptr %9, align 2, !tbaa !10
  %30 = load i32, ptr %7, align 2, !tbaa !10
  %31 = load i32, ptr %4, align 2, !tbaa !10
  %32 = urem i32 %30, %31
  %33 = icmp ne i32 %29, %32
  br i1 %33, label %34, label %37

34:                                               ; preds = %28, %13
  %35 = load i32, ptr %5, align 2, !tbaa !10
  %36 = add nsw i32 %35, 1
  store i32 %36, ptr %5, align 2, !tbaa !10
  br label %37

37:                                               ; preds = %34, %28
  %38 = load i32, ptr %8, align 2, !tbaa !10
  %39 = mul i32 %38, 31
  %40 = load i32, ptr %9, align 2, !tbaa !10
  %41 = add i32 %39, %40
  %42 = load i32, ptr @sum, align 2, !tbaa !10
  %43 = add i32 %42, %41
  store i32 %43, ptr @sum, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %9) #2
  call void @llvm.lifetime.end.p0(ptr %8) #2
  call void @llvm.lifetime.end.p0(ptr %7) #2
  br label %44

44:                                               ; preds = %37
  %45 = load i32, ptr %6, align 2, !tbaa !10
  %46 = add nsw i32 %45, 1
  store i32 %46, ptr %6, align 2, !tbaa !10
  br label %10, !llvm.loop !15

47:                                               ; preds = %10
  %48 = load i32, ptr %5, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %6) #2
  call void @llvm.lifetime.end.p0(ptr %5) #2
  ret i32 %48
}

attributes #0 = { noinline nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"p1 long", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
!15 = distinct !{!15, !13}
