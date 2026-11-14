target triple = "msp430"

@ops = dso_local global [2 x ptr] [ptr @add, ptr @mul], align 2
@one = dso_local global ptr @mul, align 2
@cells = dso_local global [4 x i16] [i16 10, i16 20, i16 30, i16 40], align 2
@inner = dso_local global ptr getelementptr (i8, ptr @cells, i64 4), align 2
@.str = private unnamed_addr constant [3 x i8] c"ab\00", align 1
@name = dso_local global ptr @.str, align 2

; Function Attrs: nounwind optsize
define internal i16 @add(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !7
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = add nsw i16 %5, %6
  ret i16 %7
}

; Function Attrs: nounwind optsize
define internal i16 @mul(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !7
  %5 = load i16, ptr %3, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = mul nsw i16 %5, %6
  ret i16 %7
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca ptr, align 2
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  %5 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @ops, i16 2), align 2, !tbaa !8
  store ptr %5, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !10
  %6 = load ptr, ptr @ops, align 2, !tbaa !8
  %7 = call i16 %6(i16 noundef 3, i16 noundef 4) #4
  %8 = sext i16 %7 to i32
  call void @report(i32 noundef %8) #4
  %9 = load ptr, ptr getelementptr inbounds nuw (i8, ptr @ops, i16 2), align 2, !tbaa !8
  %10 = call i16 %9(i16 noundef 3, i16 noundef 4) #4
  %11 = sext i16 %10 to i32
  call void @report(i32 noundef %11) #4
  %12 = load ptr, ptr @one, align 2, !tbaa !8
  %13 = call i16 %12(i16 noundef 5, i16 noundef 6) #4
  %14 = sext i16 %13 to i32
  call void @report(i32 noundef %14) #4
  %15 = load ptr, ptr @inner, align 2, !tbaa !12
  %16 = load i16, ptr %15, align 2, !tbaa !7
  %17 = sext i16 %16 to i32
  call void @report(i32 noundef %17) #4
  %18 = load ptr, ptr @name, align 2, !tbaa !14
  %19 = getelementptr inbounds i8, ptr %18, i16 1
  %20 = load i8, ptr %19, align 1, !tbaa !16
  %21 = zext i8 %20 to i32
  call void @report(i32 noundef %21) #4
  %22 = load ptr, ptr %2, align 2, !tbaa !8
  %23 = call i16 %22(i16 noundef 7, i16 noundef 8) #4
  %24 = sext i16 %23 to i32
  call void @report(i32 noundef %24) #4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %25

25:                                               ; preds = %38, %0
  %26 = load i16, ptr %3, align 2, !tbaa !7
  %27 = icmp slt i16 %26, 2
  br i1 %27, label %28, label %41

28:                                               ; preds = %25
  %29 = load i16, ptr %3, align 2, !tbaa !7
  %30 = getelementptr inbounds [2 x ptr], ptr @ops, i16 0, i16 %29
  %31 = load ptr, ptr %30, align 2, !tbaa !8
  %32 = load i16, ptr %3, align 2, !tbaa !7
  %33 = add nsw i16 %32, 2
  %34 = call i16 %31(i16 noundef %33, i16 noundef 5) #4
  %35 = sext i16 %34 to i32
  %36 = load i32, ptr %4, align 2, !tbaa !10
  %37 = add nsw i32 %36, %35
  store i32 %37, ptr %4, align 2, !tbaa !10
  br label %38

38:                                               ; preds = %28
  %39 = load i16, ptr %3, align 2, !tbaa !7
  %40 = add nsw i16 %39, 1
  store i16 %40, ptr %3, align 2, !tbaa !7
  br label %25, !llvm.loop !17

41:                                               ; preds = %25
  %42 = load i32, ptr %4, align 2, !tbaa !10
  call void @report(i32 noundef %42) #4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %43

43:                                               ; preds = %50, %41
  %44 = load i16, ptr %3, align 2, !tbaa !7
  %45 = icmp slt i16 %44, 7
  br i1 %45, label %46, label %53

46:                                               ; preds = %43
  %47 = load i16, ptr %3, align 2, !tbaa !7
  %48 = call i16 @pick(i16 noundef %47, i16 noundef 10) #4
  %49 = sext i16 %48 to i32
  call void @report(i32 noundef %49) #4
  br label %50

50:                                               ; preds = %46
  %51 = load i16, ptr %3, align 2, !tbaa !7
  %52 = add nsw i16 %51, 1
  store i16 %52, ptr %3, align 2, !tbaa !7
  br label %43, !llvm.loop !19

53:                                               ; preds = %43
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i16 @pick(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %4, align 2, !tbaa !7
  store i16 %1, ptr %5, align 2, !tbaa !7
  %6 = load i16, ptr %4, align 2, !tbaa !7
  switch i16 %6, label %25 [
    i16 0, label %7
    i16 1, label %10
    i16 2, label %13
    i16 3, label %16
    i16 4, label %19
    i16 5, label %22
  ]

7:                                                ; preds = %2
  %8 = load i16, ptr %5, align 2, !tbaa !7
  %9 = add nsw i16 %8, 1
  store i16 %9, ptr %3, align 2
  br label %26

10:                                               ; preds = %2
  %11 = load i16, ptr %5, align 2, !tbaa !7
  %12 = add nsw i16 %11, 2
  store i16 %12, ptr %3, align 2
  br label %26

13:                                               ; preds = %2
  %14 = load i16, ptr %5, align 2, !tbaa !7
  %15 = mul nsw i16 %14, 3
  store i16 %15, ptr %3, align 2
  br label %26

16:                                               ; preds = %2
  %17 = load i16, ptr %5, align 2, !tbaa !7
  %18 = sub nsw i16 %17, 4
  store i16 %18, ptr %3, align 2
  br label %26

19:                                               ; preds = %2
  %20 = load i16, ptr %5, align 2, !tbaa !7
  %21 = shl i16 %20, 2
  store i16 %21, ptr %3, align 2
  br label %26

22:                                               ; preds = %2
  %23 = load i16, ptr %5, align 2, !tbaa !7
  %24 = xor i16 %23, 7
  store i16 %24, ptr %3, align 2
  br label %26

25:                                               ; preds = %2
  store i16 -1, ptr %3, align 2
  br label %26

26:                                               ; preds = %25, %22, %19, %16, %13, %10, %7
  %27 = load i16, ptr %3, align 2
  ret i16 %27
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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !5, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"p1 int", !9, i64 0}
!14 = !{!15, !15, i64 0}
!15 = !{!"p1 omnipotent char", !9, i64 0}
!16 = !{!5, !5, i64 0}
!17 = distinct !{!17, !18}
!18 = !{!"llvm.loop.mustprogress"}
!19 = distinct !{!19, !18}
