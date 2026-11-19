target triple = "msp430"

@buf = internal global [256 x i8] zeroinitializer, align 1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !7
  store i16 0, ptr %2, align 2, !tbaa !9
  br label %4

4:                                                ; preds = %16, %0
  %5 = load i16, ptr %2, align 2, !tbaa !9
  %6 = icmp slt i16 %5, 24
  br i1 %6, label %7, label %19

7:                                                ; preds = %4
  %8 = load i32, ptr %3, align 2, !tbaa !7
  %9 = mul nsw i32 %8, 31
  %10 = load i16, ptr %2, align 2, !tbaa !9
  %11 = load i16, ptr %2, align 2, !tbaa !9
  %12 = add nsw i16 %11, 1
  %13 = call i16 @bump(i16 noundef %10, i16 noundef %12) #4
  %14 = sext i16 %13 to i32
  %15 = add nsw i32 %9, %14
  store i32 %15, ptr %3, align 2, !tbaa !7
  br label %16

16:                                               ; preds = %7
  %17 = load i16, ptr %2, align 2, !tbaa !9
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %2, align 2, !tbaa !9
  br label %4, !llvm.loop !10

19:                                               ; preds = %4
  store i16 0, ptr %2, align 2, !tbaa !9
  br label %20

20:                                               ; preds = %31, %19
  %21 = load i16, ptr %2, align 2, !tbaa !9
  %22 = icmp slt i16 %21, 256
  br i1 %22, label %23, label %34

23:                                               ; preds = %20
  %24 = load i32, ptr %3, align 2, !tbaa !7
  %25 = mul nsw i32 %24, 7
  %26 = load i16, ptr %2, align 2, !tbaa !9
  %27 = getelementptr inbounds [256 x i8], ptr @buf, i16 0, i16 %26
  %28 = load i8, ptr %27, align 1, !tbaa !12
  %29 = zext i8 %28 to i32
  %30 = add nsw i32 %25, %29
  store i32 %30, ptr %3, align 2, !tbaa !7
  br label %31

31:                                               ; preds = %23
  %32 = load i16, ptr %2, align 2, !tbaa !9
  %33 = add nsw i16 %32, 1
  store i16 %33, ptr %2, align 2, !tbaa !9
  br label %20, !llvm.loop !13

34:                                               ; preds = %20
  %35 = load i32, ptr %3, align 2, !tbaa !7
  call void @report(i32 noundef %35) #4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal i16 @bump(i16 noundef %0, i16 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  %7 = alloca i32, align 2
  store i16 %0, ptr %4, align 2, !tbaa !9
  store i16 %1, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %8 = load i16, ptr %4, align 2, !tbaa !9
  switch i16 %8, label %13 [
    i16 7, label %9
    i16 8, label %10
    i16 16, label %11
    i16 20, label %12
  ]

9:                                                ; preds = %2
  store i16 120, ptr %6, align 2, !tbaa !9
  br label %14

10:                                               ; preds = %2
  store i16 124, ptr %6, align 2, !tbaa !9
  br label %14

11:                                               ; preds = %2
  store i16 128, ptr %6, align 2, !tbaa !9
  br label %14

12:                                               ; preds = %2
  store i16 116, ptr %6, align 2, !tbaa !9
  br label %14

13:                                               ; preds = %2
  store i16 0, ptr %3, align 2
  store i32 1, ptr %7, align 2
  br label %22

14:                                               ; preds = %12, %11, %10, %9
  %15 = load i16, ptr %5, align 2, !tbaa !9
  %16 = load i16, ptr %6, align 2, !tbaa !9
  %17 = getelementptr inbounds [256 x i8], ptr @buf, i16 0, i16 %16
  %18 = load i8, ptr %17, align 1, !tbaa !12
  %19 = zext i8 %18 to i16
  %20 = add nsw i16 %19, %15
  %21 = trunc i16 %20 to i8
  store i8 %21, ptr %17, align 1, !tbaa !12
  store i16 1, ptr %3, align 2
  store i32 1, ptr %7, align 2
  br label %22

22:                                               ; preds = %14, %13
  call void @llvm.lifetime.end.p0(ptr %6) #3
  %23 = load i16, ptr %3, align 2
  ret i16 %23
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
!8 = !{!"long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!5, !5, i64 0}
!13 = distinct !{!13, !11}
