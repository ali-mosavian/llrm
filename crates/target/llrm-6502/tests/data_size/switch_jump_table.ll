target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  %3 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 5, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 -3, ptr %3, align 2, !tbaa !9
  br label %4

4:                                                ; preds = %12, %0
  %5 = load i16, ptr %3, align 2, !tbaa !9
  %6 = icmp slt i16 %5, 14
  br i1 %6, label %7, label %15

7:                                                ; preds = %4
  %8 = load i16, ptr %3, align 2, !tbaa !9
  %9 = load i32, ptr %2, align 2, !tbaa !7
  %10 = call i32 @step(i16 noundef %8, i32 noundef %9) #4
  store i32 %10, ptr %2, align 2, !tbaa !7
  %11 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %11) #4
  br label %12

12:                                               ; preds = %7
  %13 = load i16, ptr %3, align 2, !tbaa !9
  %14 = add nsw i16 %13, 1
  store i16 %14, ptr %3, align 2, !tbaa !9
  br label %4, !llvm.loop !10

15:                                               ; preds = %4
  store i16 248, ptr %3, align 2, !tbaa !9
  br label %16

16:                                               ; preds = %26, %15
  %17 = load i16, ptr %3, align 2, !tbaa !9
  %18 = icmp slt i16 %17, 258
  br i1 %18, label %19, label %29

19:                                               ; preds = %16
  %20 = load i16, ptr %3, align 2, !tbaa !9
  %21 = trunc i16 %20 to i8
  %22 = load i16, ptr %3, align 2, !tbaa !9
  %23 = add nsw i16 100, %22
  %24 = sext i16 %23 to i32
  %25 = call i32 @byte(i8 noundef zeroext %21, i32 noundef %24) #4
  call void @report(i32 noundef %25) #4
  br label %26

26:                                               ; preds = %19
  %27 = load i16, ptr %3, align 2, !tbaa !9
  %28 = add nsw i16 %27, 1
  store i16 %28, ptr %3, align 2, !tbaa !9
  br label %16, !llvm.loop !12

29:                                               ; preds = %16
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i32 @step(i16 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  store i16 %0, ptr %3, align 2, !tbaa !9
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i16, ptr %3, align 2, !tbaa !9
  switch i16 %5, label %36 [
    i16 0, label %6
    i16 1, label %9
    i16 2, label %12
    i16 3, label %15
    i16 4, label %18
    i16 5, label %22
    i16 7, label %25
    i16 8, label %30
    i16 9, label %33
  ]

6:                                                ; preds = %2
  %7 = load i32, ptr %4, align 2, !tbaa !7
  %8 = add nsw i32 %7, 3
  store i32 %8, ptr %4, align 2, !tbaa !7
  br label %39

9:                                                ; preds = %2
  %10 = load i32, ptr %4, align 2, !tbaa !7
  %11 = mul nsw i32 %10, 2
  store i32 %11, ptr %4, align 2, !tbaa !7
  br label %39

12:                                               ; preds = %2
  %13 = load i32, ptr %4, align 2, !tbaa !7
  %14 = sub nsw i32 %13, 7
  store i32 %14, ptr %4, align 2, !tbaa !7
  br label %39

15:                                               ; preds = %2
  %16 = load i32, ptr %4, align 2, !tbaa !7
  %17 = xor i32 %16, 85
  store i32 %17, ptr %4, align 2, !tbaa !7
  br label %39

18:                                               ; preds = %2
  %19 = load i32, ptr %4, align 2, !tbaa !7
  %20 = shl i32 %19, 1
  %21 = or i32 %20, 1
  store i32 %21, ptr %4, align 2, !tbaa !7
  br label %39

22:                                               ; preds = %2
  %23 = load i32, ptr %4, align 2, !tbaa !7
  %24 = sdiv i32 %23, 3
  store i32 %24, ptr %4, align 2, !tbaa !7
  br label %39

25:                                               ; preds = %2
  %26 = load i32, ptr %4, align 2, !tbaa !7
  %27 = ashr i32 %26, 2
  %28 = load i32, ptr %4, align 2, !tbaa !7
  %29 = add nsw i32 %28, %27
  store i32 %29, ptr %4, align 2, !tbaa !7
  br label %39

30:                                               ; preds = %2
  %31 = load i32, ptr %4, align 2, !tbaa !7
  %32 = sub nsw i32 0, %31
  store i32 %32, ptr %4, align 2, !tbaa !7
  br label %39

33:                                               ; preds = %2
  %34 = load i32, ptr %4, align 2, !tbaa !7
  %35 = and i32 %34, 255
  store i32 %35, ptr %4, align 2, !tbaa !7
  br label %39

36:                                               ; preds = %2
  %37 = load i32, ptr %4, align 2, !tbaa !7
  %38 = add nsw i32 %37, 1000
  store i32 %38, ptr %4, align 2, !tbaa !7
  br label %39

39:                                               ; preds = %36, %33, %30, %25, %22, %18, %15, %12, %9, %6
  %40 = load i32, ptr %4, align 2, !tbaa !7
  ret i32 %40
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @byte(i8 noundef zeroext %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i8, align 1
  %5 = alloca i32, align 2
  store i8 %0, ptr %4, align 1, !tbaa !13
  store i32 %1, ptr %5, align 2, !tbaa !7
  %6 = load i8, ptr %4, align 1, !tbaa !13
  %7 = zext i8 %6 to i16
  switch i16 %7, label %26 [
    i16 250, label %8
    i16 251, label %11
    i16 252, label %14
    i16 253, label %17
    i16 254, label %20
    i16 255, label %23
  ]

8:                                                ; preds = %2
  %9 = load i32, ptr %5, align 2, !tbaa !7
  %10 = add nsw i32 %9, 1
  store i32 %10, ptr %3, align 2
  br label %29

11:                                               ; preds = %2
  %12 = load i32, ptr %5, align 2, !tbaa !7
  %13 = mul nsw i32 %12, 3
  store i32 %13, ptr %3, align 2
  br label %29

14:                                               ; preds = %2
  %15 = load i32, ptr %5, align 2, !tbaa !7
  %16 = sub nsw i32 %15, 4
  store i32 %16, ptr %3, align 2
  br label %29

17:                                               ; preds = %2
  %18 = load i32, ptr %5, align 2, !tbaa !7
  %19 = xor i32 %18, 9
  store i32 %19, ptr %3, align 2
  br label %29

20:                                               ; preds = %2
  %21 = load i32, ptr %5, align 2, !tbaa !7
  %22 = or i32 %21, 16
  store i32 %22, ptr %3, align 2
  br label %29

23:                                               ; preds = %2
  %24 = load i32, ptr %5, align 2, !tbaa !7
  %25 = ashr i32 %24, 1
  store i32 %25, ptr %3, align 2
  br label %29

26:                                               ; preds = %2
  %27 = load i32, ptr %5, align 2, !tbaa !7
  %28 = sub nsw i32 0, %27
  store i32 %28, ptr %3, align 2
  br label %29

29:                                               ; preds = %26, %23, %20, %17, %14, %11, %8
  %30 = load i32, ptr %3, align 2
  ret i32 %30
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }
attributes #4 = { optsize }

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
!8 = !{!"long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = !{!5, !5, i64 0}
