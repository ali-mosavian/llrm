target triple = "msp430"

@v = internal global [14 x i32] [i32 0, i32 1, i32 2, i32 32767, i32 32768, i32 65535, i32 65536, i32 65537, i32 2147483647, i32 -2147483648, i32 -2147483647, i32 -65536, i32 -2, i32 -1], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %37, %0
  %6 = load i16, ptr %2, align 2, !tbaa !7
  %7 = icmp slt i16 %6, 14
  br i1 %7, label %8, label %40

8:                                                ; preds = %5
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !8
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %32, %8
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = icmp slt i16 %10, 14
  br i1 %11, label %12, label %35

12:                                               ; preds = %9
  %13 = load i32, ptr %4, align 2, !tbaa !8
  %14 = mul i32 %13, 257
  %15 = load i16, ptr %2, align 2, !tbaa !7
  %16 = getelementptr inbounds [14 x i32], ptr @v, i16 0, i16 %15
  %17 = load volatile i32, ptr %16, align 2, !tbaa !8
  %18 = load i16, ptr %3, align 2, !tbaa !7
  %19 = getelementptr inbounds [14 x i32], ptr @v, i16 0, i16 %18
  %20 = load volatile i32, ptr %19, align 2, !tbaa !8
  %21 = call i32 @branches(i32 noundef %17, i32 noundef %20) #4
  %22 = add i32 %14, %21
  %23 = load i16, ptr %2, align 2, !tbaa !7
  %24 = getelementptr inbounds [14 x i32], ptr @v, i16 0, i16 %23
  %25 = load volatile i32, ptr %24, align 2, !tbaa !8
  %26 = load i16, ptr %3, align 2, !tbaa !7
  %27 = getelementptr inbounds [14 x i32], ptr @v, i16 0, i16 %26
  %28 = load volatile i32, ptr %27, align 2, !tbaa !8
  %29 = call i32 @selects(i32 noundef %25, i32 noundef %28) #4
  %30 = mul i32 %29, 65599
  %31 = add i32 %22, %30
  store i32 %31, ptr %4, align 2, !tbaa !8
  br label %32

32:                                               ; preds = %12
  %33 = load i16, ptr %3, align 2, !tbaa !7
  %34 = add nsw i16 %33, 1
  store i16 %34, ptr %3, align 2, !tbaa !7
  br label %9, !llvm.loop !10

35:                                               ; preds = %9
  %36 = load i32, ptr %4, align 2, !tbaa !8
  call void @report(i32 noundef %36) #4
  call void @llvm.lifetime.end.p0(ptr %4) #3
  br label %37

37:                                               ; preds = %35
  %38 = load i16, ptr %2, align 2, !tbaa !7
  %39 = add nsw i16 %38, 1
  store i16 %39, ptr %2, align 2, !tbaa !7
  br label %5, !llvm.loop !12

40:                                               ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i32 @branches(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %5) #3
  %8 = load i32, ptr %3, align 2, !tbaa !8
  store i32 %8, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %9 = load i32, ptr %4, align 2, !tbaa !8
  store i32 %9, ptr %6, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %7) #3
  store i32 0, ptr %7, align 2, !tbaa !8
  %10 = load i32, ptr %3, align 2, !tbaa !8
  %11 = load i32, ptr %4, align 2, !tbaa !8
  %12 = icmp ult i32 %10, %11
  br i1 %12, label %13, label %16

13:                                               ; preds = %2
  %14 = load i32, ptr %7, align 2, !tbaa !8
  %15 = add i32 %14, 1
  store i32 %15, ptr %7, align 2, !tbaa !8
  br label %16

16:                                               ; preds = %13, %2
  %17 = load i32, ptr %3, align 2, !tbaa !8
  %18 = load i32, ptr %4, align 2, !tbaa !8
  %19 = icmp ule i32 %17, %18
  br i1 %19, label %20, label %23

20:                                               ; preds = %16
  %21 = load i32, ptr %7, align 2, !tbaa !8
  %22 = add i32 %21, 2
  store i32 %22, ptr %7, align 2, !tbaa !8
  br label %23

23:                                               ; preds = %20, %16
  %24 = load i32, ptr %3, align 2, !tbaa !8
  %25 = load i32, ptr %4, align 2, !tbaa !8
  %26 = icmp ugt i32 %24, %25
  br i1 %26, label %27, label %30

27:                                               ; preds = %23
  %28 = load i32, ptr %7, align 2, !tbaa !8
  %29 = add i32 %28, 4
  store i32 %29, ptr %7, align 2, !tbaa !8
  br label %30

30:                                               ; preds = %27, %23
  %31 = load i32, ptr %3, align 2, !tbaa !8
  %32 = load i32, ptr %4, align 2, !tbaa !8
  %33 = icmp uge i32 %31, %32
  br i1 %33, label %34, label %37

34:                                               ; preds = %30
  %35 = load i32, ptr %7, align 2, !tbaa !8
  %36 = add i32 %35, 8
  store i32 %36, ptr %7, align 2, !tbaa !8
  br label %37

37:                                               ; preds = %34, %30
  %38 = load i32, ptr %5, align 2, !tbaa !8
  %39 = load i32, ptr %6, align 2, !tbaa !8
  %40 = icmp slt i32 %38, %39
  br i1 %40, label %41, label %44

41:                                               ; preds = %37
  %42 = load i32, ptr %7, align 2, !tbaa !8
  %43 = add i32 %42, 16
  store i32 %43, ptr %7, align 2, !tbaa !8
  br label %44

44:                                               ; preds = %41, %37
  %45 = load i32, ptr %5, align 2, !tbaa !8
  %46 = load i32, ptr %6, align 2, !tbaa !8
  %47 = icmp sle i32 %45, %46
  br i1 %47, label %48, label %51

48:                                               ; preds = %44
  %49 = load i32, ptr %7, align 2, !tbaa !8
  %50 = add i32 %49, 32
  store i32 %50, ptr %7, align 2, !tbaa !8
  br label %51

51:                                               ; preds = %48, %44
  %52 = load i32, ptr %5, align 2, !tbaa !8
  %53 = load i32, ptr %6, align 2, !tbaa !8
  %54 = icmp sgt i32 %52, %53
  br i1 %54, label %55, label %58

55:                                               ; preds = %51
  %56 = load i32, ptr %7, align 2, !tbaa !8
  %57 = add i32 %56, 64
  store i32 %57, ptr %7, align 2, !tbaa !8
  br label %58

58:                                               ; preds = %55, %51
  %59 = load i32, ptr %5, align 2, !tbaa !8
  %60 = load i32, ptr %6, align 2, !tbaa !8
  %61 = icmp sge i32 %59, %60
  br i1 %61, label %62, label %65

62:                                               ; preds = %58
  %63 = load i32, ptr %7, align 2, !tbaa !8
  %64 = add i32 %63, 128
  store i32 %64, ptr %7, align 2, !tbaa !8
  br label %65

65:                                               ; preds = %62, %58
  %66 = load i32, ptr %7, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %66
}

; Function Attrs: nounwind optsize
define internal i32 @selects(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %5) #3
  %7 = load i32, ptr %3, align 2, !tbaa !8
  store i32 %7, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %8 = load i32, ptr %4, align 2, !tbaa !8
  store i32 %8, ptr %6, align 2, !tbaa !8
  %9 = load i32, ptr %3, align 2, !tbaa !8
  %10 = load i32, ptr %4, align 2, !tbaa !8
  %11 = icmp ult i32 %9, %10
  %12 = zext i1 %11 to i64
  %13 = select i1 %11, i32 3, i32 5
  %14 = load i32, ptr %3, align 2, !tbaa !8
  %15 = load i32, ptr %4, align 2, !tbaa !8
  %16 = icmp uge i32 %14, %15
  %17 = zext i1 %16 to i64
  %18 = select i1 %16, i32 7, i32 11
  %19 = mul i32 %18, 16
  %20 = add i32 %13, %19
  %21 = load i32, ptr %5, align 2, !tbaa !8
  %22 = load i32, ptr %6, align 2, !tbaa !8
  %23 = icmp slt i32 %21, %22
  %24 = zext i1 %23 to i64
  %25 = select i1 %23, i32 13, i32 17
  %26 = mul i32 %25, 256
  %27 = add i32 %20, %26
  %28 = load i32, ptr %5, align 2, !tbaa !8
  %29 = load i32, ptr %6, align 2, !tbaa !8
  %30 = icmp sge i32 %28, %29
  %31 = zext i1 %30 to i64
  %32 = select i1 %30, i32 19, i32 23
  %33 = mul i32 %32, 4096
  %34 = add i32 %27, %33
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %34
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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
