target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = 'queens.c'
source_filename = "queens.c"
target triple = "msp430"

; Function Attrs: nounwind
define dso_local i32 @bench_queens(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [12 x i16], align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  %4 = getelementptr inbounds [12 x i16], ptr %3, i16 0, i16 0
  %5 = load i16, ptr %2, align 2, !tbaa !7
  %6 = call i16 @place(ptr noundef %4, i16 noundef 0, i16 noundef %5)
  %7 = sext i16 %6 to i32
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %7
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal i16 @place(ptr noundef %0, i16 noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca i32, align 2
  store ptr %0, ptr %5, align 2, !tbaa !8
  store i16 %1, ptr %6, align 2, !tbaa !7
  store i16 %2, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i16 0, ptr %9, align 2, !tbaa !7
  %11 = load i16, ptr %6, align 2, !tbaa !7
  %12 = load i16, ptr %7, align 2, !tbaa !7
  %13 = icmp eq i16 %11, %12
  br i1 %13, label %14, label %15

14:                                               ; preds = %3
  store i16 1, ptr %4, align 2
  store i32 1, ptr %10, align 2
  br label %44

15:                                               ; preds = %3
  store i16 0, ptr %8, align 2, !tbaa !7
  br label %16

16:                                               ; preds = %39, %15
  %17 = load i16, ptr %8, align 2, !tbaa !7
  %18 = load i16, ptr %7, align 2, !tbaa !7
  %19 = icmp slt i16 %17, %18
  br i1 %19, label %20, label %42

20:                                               ; preds = %16
  %21 = load ptr, ptr %5, align 2, !tbaa !8
  %22 = load i16, ptr %6, align 2, !tbaa !7
  %23 = load i16, ptr %8, align 2, !tbaa !7
  %24 = call i16 @safe(ptr noundef %21, i16 noundef %22, i16 noundef %23)
  %25 = icmp ne i16 %24, 0
  br i1 %25, label %26, label %38

26:                                               ; preds = %20
  %27 = load i16, ptr %8, align 2, !tbaa !7
  %28 = load ptr, ptr %5, align 2, !tbaa !8
  %29 = load i16, ptr %6, align 2, !tbaa !7
  %30 = getelementptr inbounds i16, ptr %28, i16 %29
  store i16 %27, ptr %30, align 2, !tbaa !7
  %31 = load ptr, ptr %5, align 2, !tbaa !8
  %32 = load i16, ptr %6, align 2, !tbaa !7
  %33 = add nsw i16 %32, 1
  %34 = load i16, ptr %7, align 2, !tbaa !7
  %35 = call i16 @place(ptr noundef %31, i16 noundef %33, i16 noundef %34)
  %36 = load i16, ptr %9, align 2, !tbaa !7
  %37 = add nsw i16 %36, %35
  store i16 %37, ptr %9, align 2, !tbaa !7
  br label %38

38:                                               ; preds = %26, %20
  br label %39

39:                                               ; preds = %38
  %40 = load i16, ptr %8, align 2, !tbaa !7
  %41 = add nsw i16 %40, 1
  store i16 %41, ptr %8, align 2, !tbaa !7
  br label %16, !llvm.loop !11

42:                                               ; preds = %16
  %43 = load i16, ptr %9, align 2, !tbaa !7
  store i16 %43, ptr %4, align 2
  store i32 1, ptr %10, align 2
  br label %44

44:                                               ; preds = %42, %14
  call void @llvm.lifetime.end.p0(ptr %9) #3
  call void @llvm.lifetime.end.p0(ptr %8) #3
  %45 = load i16, ptr %4, align 2
  ret i16 %45
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @bench_queens(i16 noundef 7)
  call void @report(i32 noundef %2)
  ret i16 0
}

declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind
define internal i16 @safe(ptr noundef %0, i16 noundef %1, i16 noundef %2) #0 {
  %4 = alloca i16, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca i32, align 2
  store ptr %0, ptr %5, align 2, !tbaa !8
  store i16 %1, ptr %6, align 2, !tbaa !7
  store i16 %2, ptr %7, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %8) #3
  store i16 0, ptr %8, align 2, !tbaa !7
  br label %11

11:                                               ; preds = %41, %3
  %12 = load i16, ptr %8, align 2, !tbaa !7
  %13 = load i16, ptr %6, align 2, !tbaa !7
  %14 = icmp slt i16 %12, %13
  br i1 %14, label %15, label %44

15:                                               ; preds = %11
  call void @llvm.lifetime.start.p0(ptr %9) #3
  %16 = load ptr, ptr %5, align 2, !tbaa !8
  %17 = load i16, ptr %8, align 2, !tbaa !7
  %18 = getelementptr inbounds i16, ptr %16, i16 %17
  %19 = load i16, ptr %18, align 2, !tbaa !7
  %20 = load i16, ptr %7, align 2, !tbaa !7
  %21 = sub nsw i16 %19, %20
  store i16 %21, ptr %9, align 2, !tbaa !7
  %22 = load i16, ptr %9, align 2, !tbaa !7
  %23 = icmp eq i16 %22, 0
  br i1 %23, label %36, label %24

24:                                               ; preds = %15
  %25 = load i16, ptr %9, align 2, !tbaa !7
  %26 = load i16, ptr %6, align 2, !tbaa !7
  %27 = load i16, ptr %8, align 2, !tbaa !7
  %28 = sub nsw i16 %26, %27
  %29 = icmp eq i16 %25, %28
  br i1 %29, label %36, label %30

30:                                               ; preds = %24
  %31 = load i16, ptr %9, align 2, !tbaa !7
  %32 = load i16, ptr %8, align 2, !tbaa !7
  %33 = load i16, ptr %6, align 2, !tbaa !7
  %34 = sub nsw i16 %32, %33
  %35 = icmp eq i16 %31, %34
  br i1 %35, label %36, label %37

36:                                               ; preds = %30, %24, %15
  store i16 0, ptr %4, align 2
  store i32 1, ptr %10, align 2
  br label %38

37:                                               ; preds = %30
  store i32 0, ptr %10, align 2
  br label %38

38:                                               ; preds = %37, %36
  call void @llvm.lifetime.end.p0(ptr %9) #3
  %39 = load i32, ptr %10, align 2
  switch i32 %39, label %45 [
    i32 0, label %40
  ]

40:                                               ; preds = %38
  br label %41

41:                                               ; preds = %40
  %42 = load i16, ptr %8, align 2, !tbaa !7
  %43 = add nsw i16 %42, 1
  store i16 %43, ptr %8, align 2, !tbaa !7
  br label %11, !llvm.loop !13

44:                                               ; preds = %11
  store i16 1, ptr %4, align 2
  store i32 1, ptr %10, align 2
  br label %45

45:                                               ; preds = %44, %38
  call void @llvm.lifetime.end.p0(ptr %8) #3
  %46 = load i16, ptr %4, align 2
  ret i16 %46
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { nounwind }

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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"p1 int", !10, i64 0}
!10 = !{!"any pointer", !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
