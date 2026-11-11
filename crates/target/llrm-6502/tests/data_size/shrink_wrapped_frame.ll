target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 5, ptr %2, align 2, !tbaa !7
  %3 = call i32 @f(i32 noundef 0, ptr noundef %2) #4
  call void @report(i32 noundef %3) #4
  %4 = call i32 @f(i32 noundef 2, ptr noundef %2) #4
  call void @report(i32 noundef %4) #4
  %5 = call i32 @g(i32 noundef 0, ptr noundef %2) #4
  call void @report(i32 noundef %5) #4
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @f(i32 noundef %0, ptr noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca ptr, align 2
  %6 = alloca [16 x i32], align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store ptr %1, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %6) #3
  call void @llvm.lifetime.start.p0(ptr %7) #3
  call void @llvm.lifetime.start.p0(ptr %8) #3
  store i32 0, ptr %8, align 2, !tbaa !7
  %10 = load i32, ptr %4, align 2, !tbaa !7
  %11 = icmp eq i32 %10, 0
  br i1 %11, label %12, label %13

12:                                               ; preds = %2
  store i32 7, ptr %3, align 2
  store i32 1, ptr %9, align 2
  br label %48

13:                                               ; preds = %2
  store i32 0, ptr %7, align 2, !tbaa !7
  br label %14

14:                                               ; preds = %24, %13
  %15 = load i32, ptr %7, align 2, !tbaa !7
  %16 = icmp slt i32 %15, 16
  br i1 %16, label %17, label %27

17:                                               ; preds = %14
  %18 = load i32, ptr %4, align 2, !tbaa !7
  %19 = load i32, ptr %7, align 2, !tbaa !7
  %20 = mul nsw i32 %18, %19
  %21 = load i32, ptr %7, align 2, !tbaa !7
  %22 = trunc i32 %21 to i16
  %23 = getelementptr inbounds [16 x i32], ptr %6, i16 0, i16 %22
  store i32 %20, ptr %23, align 2, !tbaa !7
  br label %24

24:                                               ; preds = %17
  %25 = load i32, ptr %7, align 2, !tbaa !7
  %26 = add nsw i32 %25, 1
  store i32 %26, ptr %7, align 2, !tbaa !7
  br label %14, !llvm.loop !12

27:                                               ; preds = %14
  %28 = load ptr, ptr %5, align 2, !tbaa !9
  %29 = call i32 @depth(ptr noundef %28) #4
  %30 = load i32, ptr %4, align 2, !tbaa !7
  %31 = add nsw i32 %29, %30
  %32 = getelementptr inbounds [16 x i32], ptr %6, i16 0, i16 3
  store i32 %31, ptr %32, align 2, !tbaa !7
  store i32 0, ptr %7, align 2, !tbaa !7
  br label %33

33:                                               ; preds = %43, %27
  %34 = load i32, ptr %7, align 2, !tbaa !7
  %35 = icmp slt i32 %34, 16
  br i1 %35, label %36, label %46

36:                                               ; preds = %33
  %37 = load i32, ptr %7, align 2, !tbaa !7
  %38 = trunc i32 %37 to i16
  %39 = getelementptr inbounds [16 x i32], ptr %6, i16 0, i16 %38
  %40 = load i32, ptr %39, align 2, !tbaa !7
  %41 = load i32, ptr %8, align 2, !tbaa !7
  %42 = add nsw i32 %41, %40
  store i32 %42, ptr %8, align 2, !tbaa !7
  br label %43

43:                                               ; preds = %36
  %44 = load i32, ptr %7, align 2, !tbaa !7
  %45 = add nsw i32 %44, 1
  store i32 %45, ptr %7, align 2, !tbaa !7
  br label %33, !llvm.loop !14

46:                                               ; preds = %33
  %47 = load i32, ptr %8, align 2, !tbaa !7
  store i32 %47, ptr %3, align 2
  store i32 1, ptr %9, align 2
  br label %48

48:                                               ; preds = %46, %12
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  %49 = load i32, ptr %3, align 2
  ret i32 %49
}

; Function Attrs: nounwind optsize
define internal i32 @g(i32 noundef %0, ptr noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca ptr, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store ptr %1, ptr %5, align 2, !tbaa !9
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = icmp sgt i32 %6, 3
  br i1 %7, label %8, label %10

8:                                                ; preds = %2
  %9 = load i32, ptr %4, align 2, !tbaa !7
  store i32 %9, ptr %3, align 2
  br label %19

10:                                               ; preds = %2
  %11 = load i32, ptr %4, align 2, !tbaa !7
  %12 = load ptr, ptr %5, align 2, !tbaa !9
  %13 = call i32 @f(i32 noundef %11, ptr noundef %12) #4
  %14 = load i32, ptr %4, align 2, !tbaa !7
  %15 = add nsw i32 %14, 1
  %16 = load ptr, ptr %5, align 2, !tbaa !9
  %17 = call i32 @g(i32 noundef %15, ptr noundef %16) #4
  %18 = add nsw i32 %13, %17
  store i32 %18, ptr %3, align 2
  br label %19

19:                                               ; preds = %10, %8
  %20 = load i32, ptr %3, align 2
  ret i32 %20
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i32 @depth(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  store ptr %0, ptr %2, align 2, !tbaa !9
  %3 = load ptr, ptr %2, align 2, !tbaa !9
  %4 = load i32, ptr %3, align 2, !tbaa !7
  ret i32 %4
}

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
!9 = !{!10, !10, i64 0}
!10 = !{!"p1 long", !11, i64 0}
!11 = !{!"any pointer", !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
