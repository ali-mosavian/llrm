target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i64, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i64 0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 -1, ptr %3, align 2, !tbaa !9
  br label %5

5:                                                ; preds = %42, %0
  %6 = load i16, ptr %3, align 2, !tbaa !9
  %7 = icmp sle i16 %6, 1
  br i1 %7, label %8, label %45

8:                                                ; preds = %5
  store i16 -1, ptr %4, align 2, !tbaa !9
  br label %9

9:                                                ; preds = %38, %8
  %10 = load i16, ptr %4, align 2, !tbaa !9
  %11 = icmp sle i16 %10, 1
  br i1 %11, label %12, label %41

12:                                               ; preds = %9
  %13 = load i64, ptr %2, align 2, !tbaa !7
  %14 = mul nsw i64 %13, 3
  %15 = load i16, ptr %3, align 2, !tbaa !9
  %16 = load i16, ptr %4, align 2, !tbaa !9
  %17 = call i64 @lt(i16 noundef %15, i16 noundef %16) #4
  %18 = add nsw i64 %14, %17
  %19 = load i16, ptr %3, align 2, !tbaa !9
  %20 = sext i16 %19 to i32
  %21 = load i16, ptr %4, align 2, !tbaa !9
  %22 = sext i16 %21 to i32
  %23 = call i64 @ne(i32 noundef %20, i32 noundef %22) #4
  %24 = mul nsw i64 %23, 2
  %25 = add nsw i64 %18, %24
  %26 = load i16, ptr %3, align 2, !tbaa !9
  %27 = load i16, ptr %4, align 2, !tbaa !9
  %28 = call i64 @both(i16 noundef %26, i16 noundef %27) #4
  %29 = lshr i64 %28, 33
  %30 = mul nsw i64 %29, 5
  %31 = add nsw i64 %25, %30
  %32 = load i16, ptr %3, align 2, !tbaa !9
  %33 = load i16, ptr %4, align 2, !tbaa !9
  %34 = icmp eq i16 %32, %33
  %35 = zext i1 %34 to i16
  %36 = sext i16 %35 to i64
  %37 = sub nsw i64 %31, %36
  store i64 %37, ptr %2, align 2, !tbaa !7
  br label %38

38:                                               ; preds = %12
  %39 = load i16, ptr %4, align 2, !tbaa !9
  %40 = add nsw i16 %39, 1
  store i16 %40, ptr %4, align 2, !tbaa !9
  br label %9, !llvm.loop !10

41:                                               ; preds = %9
  br label %42

42:                                               ; preds = %41
  %43 = load i16, ptr %3, align 2, !tbaa !9
  %44 = add nsw i16 %43, 1
  store i16 %44, ptr %3, align 2, !tbaa !9
  br label %5, !llvm.loop !12

45:                                               ; preds = %5
  %46 = load i64, ptr %2, align 2, !tbaa !7
  %47 = trunc i64 %46 to i32
  call void @report(i32 noundef %47) #4
  %48 = load i64, ptr %2, align 2, !tbaa !7
  %49 = ashr i64 %48, 3
  %50 = trunc i64 %49 to i32
  %51 = call i64 @both(i16 noundef 1, i16 noundef 1) #4
  %52 = lshr i64 %51, 32
  %53 = trunc i64 %52 to i32
  %54 = add nsw i32 %50, %53
  call void @report(i32 noundef %54) #4
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i64 @lt(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !9
  store i16 %1, ptr %4, align 2, !tbaa !9
  %5 = load i16, ptr %3, align 2, !tbaa !9
  %6 = load i16, ptr %4, align 2, !tbaa !9
  %7 = icmp slt i16 %5, %6
  %8 = zext i1 %7 to i16
  %9 = sext i16 %8 to i64
  ret i64 %9
}

; Function Attrs: nounwind optsize
define internal i64 @ne(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !13
  store i32 %1, ptr %4, align 2, !tbaa !13
  %5 = load i32, ptr %3, align 2, !tbaa !13
  %6 = load i32, ptr %4, align 2, !tbaa !13
  %7 = icmp ne i32 %5, %6
  %8 = zext i1 %7 to i16
  %9 = sext i16 %8 to i64
  ret i64 %9
}

; Function Attrs: nounwind optsize
define internal i64 @both(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !9
  store i16 %1, ptr %4, align 2, !tbaa !9
  %5 = load i16, ptr %3, align 2, !tbaa !9
  %6 = icmp ne i16 %5, 0
  br i1 %6, label %7, label %10

7:                                                ; preds = %2
  %8 = load i16, ptr %4, align 2, !tbaa !9
  %9 = icmp ne i16 %8, 0
  br label %10

10:                                               ; preds = %7, %2
  %11 = phi i1 [ false, %2 ], [ %9, %7 ]
  %12 = zext i1 %11 to i16
  %13 = sext i16 %12 to i64
  %14 = shl i64 %13, 33
  ret i64 %14
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

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
!8 = !{!"long long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = !{!14, !14, i64 0}
!14 = !{!"long", !5, i64 0}
