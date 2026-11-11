target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@sink = dso_local global [4 x i32] zeroinitializer, align 2
@chosen = dso_local global ptr @forty_two, align 2
@in = dso_local global [10 x i32] [i32 1, i32 2, i32 3, i32 4, i32 5, i32 6, i32 7, i32 41, i32 40, i32 20], align 2

; Function Attrs: nounwind optsize
define dso_local i32 @add3(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  store i32 %2, ptr %6, align 2, !tbaa !7
  %7 = load i32, ptr %4, align 2, !tbaa !7
  %8 = load i32, ptr %5, align 2, !tbaa !7
  %9 = mul nsw i32 %8, 2
  %10 = add nsw i32 %7, %9
  %11 = load i32, ptr %6, align 2, !tbaa !7
  %12 = mul nsw i32 %11, 3
  %13 = add nsw i32 %10, %12
  ret i32 %13
}

; Function Attrs: nounwind optsize
define dso_local i32 @same(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  store i32 %2, ptr %6, align 2, !tbaa !7
  %7 = load i32, ptr %4, align 2, !tbaa !7
  %8 = load i32, ptr %5, align 2, !tbaa !7
  %9 = load i32, ptr %6, align 2, !tbaa !7
  %10 = call i32 @add3(i32 noundef %7, i32 noundef %8, i32 noundef %9) #3
  ret i32 %10
}

; Function Attrs: nounwind optsize
define dso_local i32 @extra(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = call i32 @add3(i32 noundef %5, i32 noundef %6, i32 noundef 7) #3
  ret i32 %7
}

; Function Attrs: nounwind optsize
define dso_local i32 @swapped(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  store i32 %2, ptr %6, align 2, !tbaa !7
  %7 = load i32, ptr %6, align 2, !tbaa !7
  %8 = load i32, ptr %5, align 2, !tbaa !7
  %9 = load i32, ptr %4, align 2, !tbaa !7
  %10 = call i32 @add3(i32 noundef %7, i32 noundef %8, i32 noundef %9) #3
  ret i32 %10
}

; Function Attrs: nounwind optsize
define dso_local i32 @mixed(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  store i32 %2, ptr %6, align 2, !tbaa !7
  %7 = load i32, ptr %4, align 2, !tbaa !7
  %8 = load i32, ptr %6, align 2, !tbaa !7
  %9 = load i32, ptr %5, align 2, !tbaa !7
  %10 = call i32 @add3(i32 noundef %7, i32 noundef %8, i32 noundef %9) #3
  %11 = xor i32 %10, 85
  ret i32 %11
}

; Function Attrs: nounwind optsize
define dso_local i32 @framed(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca [6 x i32], align 2
  %8 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  store i32 %2, ptr %6, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %7) #4
  call void @llvm.lifetime.start.p0(ptr %8) #4
  store i32 0, ptr %8, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %21, %3
  %10 = load i32, ptr %8, align 2, !tbaa !7
  %11 = icmp slt i32 %10, 6
  br i1 %11, label %12, label %24

12:                                               ; preds = %9
  %13 = load i32, ptr %4, align 2, !tbaa !7
  %14 = load i32, ptr %8, align 2, !tbaa !7
  %15 = mul nsw i32 %13, %14
  %16 = load i32, ptr %5, align 2, !tbaa !7
  %17 = add nsw i32 %15, %16
  %18 = load i32, ptr %8, align 2, !tbaa !7
  %19 = trunc i32 %18 to i16
  %20 = getelementptr inbounds [6 x i32], ptr %7, i16 0, i16 %19
  store i32 %17, ptr %20, align 2, !tbaa !7
  br label %21

21:                                               ; preds = %12
  %22 = load i32, ptr %8, align 2, !tbaa !7
  %23 = add nsw i32 %22, 1
  store i32 %23, ptr %8, align 2, !tbaa !7
  br label %9, !llvm.loop !9

24:                                               ; preds = %9
  %25 = getelementptr inbounds [6 x i32], ptr %7, i16 0, i16 3
  %26 = load i32, ptr %25, align 2, !tbaa !7
  %27 = getelementptr inbounds [6 x i32], ptr %7, i16 0, i16 5
  %28 = load i32, ptr %27, align 2, !tbaa !7
  %29 = add nsw i32 %26, %28
  store volatile i32 %29, ptr @sink, align 2, !tbaa !7
  %30 = load i32, ptr %4, align 2, !tbaa !7
  %31 = load i32, ptr %5, align 2, !tbaa !7
  %32 = load i32, ptr %6, align 2, !tbaa !7
  %33 = call i32 @add3(i32 noundef %30, i32 noundef %31, i32 noundef %32) #3
  call void @llvm.lifetime.end.p0(ptr %8) #4
  call void @llvm.lifetime.end.p0(ptr %7) #4
  ret i32 %33
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i32 @odd(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = icmp eq i32 %5, 0
  br i1 %6, label %7, label %9

7:                                                ; preds = %2
  %8 = load i32, ptr %4, align 2, !tbaa !7
  br label %15

9:                                                ; preds = %2
  %10 = load i32, ptr %3, align 2, !tbaa !7
  %11 = sub nsw i32 %10, 1
  %12 = load i32, ptr %4, align 2, !tbaa !7
  %13 = add nsw i32 %12, 3
  %14 = call i32 @even(i32 noundef %11, i32 noundef %13) #3
  br label %15

15:                                               ; preds = %9, %7
  %16 = phi i32 [ %8, %7 ], [ %14, %9 ]
  ret i32 %16
}

; Function Attrs: nounwind optsize
define dso_local i32 @even(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !7
  %5 = load i32, ptr %3, align 2, !tbaa !7
  %6 = icmp eq i32 %5, 0
  br i1 %6, label %7, label %10

7:                                                ; preds = %2
  %8 = load i32, ptr %4, align 2, !tbaa !7
  %9 = add nsw i32 %8, 1
  br label %17

10:                                               ; preds = %2
  %11 = load i32, ptr %3, align 2, !tbaa !7
  %12 = sub nsw i32 %11, 1
  %13 = load i32, ptr %4, align 2, !tbaa !7
  %14 = mul nsw i32 %13, 2
  %15 = add nsw i32 %14, 1
  %16 = call i32 @odd(i32 noundef %12, i32 noundef %15) #3
  br label %17

17:                                               ; preds = %10, %7
  %18 = phi i32 [ %9, %7 ], [ %16, %10 ]
  ret i32 %18
}

; Function Attrs: nounwind optsize
define dso_local i32 @countdown(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  %6 = load i32, ptr %4, align 2, !tbaa !7
  %7 = icmp eq i32 %6, 0
  br i1 %7, label %8, label %10

8:                                                ; preds = %2
  %9 = load i32, ptr %5, align 2, !tbaa !7
  store i32 %9, ptr %3, align 2
  br label %18

10:                                               ; preds = %2
  %11 = load i32, ptr %4, align 2, !tbaa !7
  %12 = sub nsw i32 %11, 1
  %13 = load i32, ptr %5, align 2, !tbaa !7
  %14 = mul nsw i32 %13, 3
  %15 = load i32, ptr %4, align 2, !tbaa !7
  %16 = add nsw i32 %14, %15
  %17 = call i32 @countdown(i32 noundef %12, i32 noundef %16) #3
  store i32 %17, ptr %3, align 2
  br label %18

18:                                               ; preds = %10, %8
  %19 = load i32, ptr %3, align 2
  ret i32 %19
}

; Function Attrs: nounwind optsize
define dso_local i32 @forty_two() #0 {
  ret i32 42
}

; Function Attrs: nounwind optsize
define dso_local i32 @through_pointer() #0 {
  %1 = load volatile ptr, ptr @chosen, align 2, !tbaa !11
  %2 = call i32 %1() #3
  ret i32 %2
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = load volatile i32, ptr @in, align 2, !tbaa !7
  %3 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 4), align 2, !tbaa !7
  %4 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 8), align 2, !tbaa !7
  %5 = call i32 @same(i32 noundef %2, i32 noundef %3, i32 noundef %4) #3
  call void @report(i32 noundef %5) #3
  %6 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 12), align 2, !tbaa !7
  %7 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 16), align 2, !tbaa !7
  %8 = call i32 @extra(i32 noundef %6, i32 noundef %7) #3
  call void @report(i32 noundef %8) #3
  %9 = load volatile i32, ptr @in, align 2, !tbaa !7
  %10 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 4), align 2, !tbaa !7
  %11 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 8), align 2, !tbaa !7
  %12 = call i32 @swapped(i32 noundef %9, i32 noundef %10, i32 noundef %11) #3
  call void @report(i32 noundef %12) #3
  %13 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 4), align 2, !tbaa !7
  %14 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 8), align 2, !tbaa !7
  %15 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 12), align 2, !tbaa !7
  %16 = call i32 @mixed(i32 noundef %13, i32 noundef %14, i32 noundef %15) #3
  call void @report(i32 noundef %16) #3
  %17 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 16), align 2, !tbaa !7
  %18 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 20), align 2, !tbaa !7
  %19 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 24), align 2, !tbaa !7
  %20 = call i32 @framed(i32 noundef %17, i32 noundef %18, i32 noundef %19) #3
  call void @report(i32 noundef %20) #3
  %21 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 28), align 2, !tbaa !7
  %22 = call i32 @even(i32 noundef %21, i32 noundef 0) #3
  call void @report(i32 noundef %22) #3
  %23 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 32), align 2, !tbaa !7
  %24 = call i32 @odd(i32 noundef %23, i32 noundef 1) #3
  call void @report(i32 noundef %24) #3
  %25 = load volatile i32, ptr getelementptr inbounds nuw (i8, ptr @in, i16 36), align 2, !tbaa !7
  %26 = call i32 @countdown(i32 noundef %25, i32 noundef 1) #3
  %27 = and i32 %26, 16777215
  call void @report(i32 noundef %27) #3
  %28 = call i32 @through_pointer() #3
  call void @report(i32 noundef %28) #3
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #3 = { optsize }
attributes #4 = { nounwind }

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
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!12, !12, i64 0}
!12 = !{!"any pointer", !5, i64 0}
