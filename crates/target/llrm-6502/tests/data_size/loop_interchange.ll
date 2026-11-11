target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@b = internal global [32 x [32 x i16]] zeroinitializer, align 2
@a = internal global [32 x [32 x i16]] zeroinitializer, align 2
@c = internal global [32 x [32 x i16]] zeroinitializer, align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %4

4:                                                ; preds = %24, %0
  %5 = load i16, ptr %2, align 2, !tbaa !7
  %6 = icmp slt i16 %5, 32
  br i1 %6, label %7, label %27

7:                                                ; preds = %4
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %8

8:                                                ; preds = %20, %7
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = icmp slt i16 %9, 32
  br i1 %10, label %11, label %23

11:                                               ; preds = %8
  %12 = load i16, ptr %2, align 2, !tbaa !7
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = mul nsw i16 2, %13
  %15 = add nsw i16 %12, %14
  %16 = load i16, ptr %2, align 2, !tbaa !7
  %17 = getelementptr inbounds [32 x [32 x i16]], ptr @b, i16 0, i16 %16
  %18 = load i16, ptr %3, align 2, !tbaa !7
  %19 = getelementptr inbounds [32 x i16], ptr %17, i16 0, i16 %18
  store i16 %15, ptr %19, align 2, !tbaa !8
  br label %20

20:                                               ; preds = %11
  %21 = load i16, ptr %3, align 2, !tbaa !7
  %22 = add nsw i16 %21, 1
  store i16 %22, ptr %3, align 2, !tbaa !7
  br label %8, !llvm.loop !10

23:                                               ; preds = %8
  br label %24

24:                                               ; preds = %23
  %25 = load i16, ptr %2, align 2, !tbaa !7
  %26 = add nsw i16 %25, 1
  store i16 %26, ptr %2, align 2, !tbaa !7
  br label %4, !llvm.loop !12

27:                                               ; preds = %4
  call void @columns(i16 noundef signext 5) #4
  call void @reads_back(i16 noundef signext 3) #4
  call void @triangle(i16 noundef signext 2) #4
  %28 = call i32 @checksum(ptr noundef @a) #4
  call void @report(i32 noundef %28) #4
  %29 = call i32 @checksum(ptr noundef @b) #4
  call void @report(i32 noundef %29) #4
  %30 = call i32 @checksum(ptr noundef @c) #4
  call void @report(i32 noundef %30) #4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @columns(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %28, %1
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = icmp slt i16 %6, 32
  br i1 %7, label %8, label %31

8:                                                ; preds = %5
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %24, %8
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = icmp slt i16 %10, 32
  br i1 %11, label %12, label %27

12:                                               ; preds = %9
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = mul nsw i16 %13, 7
  %15 = load i16, ptr %4, align 2, !tbaa !7
  %16 = mul nsw i16 %15, 3
  %17 = add nsw i16 %14, %16
  %18 = load i16, ptr %2, align 2, !tbaa !8
  %19 = add nsw i16 %17, %18
  %20 = load i16, ptr %3, align 2, !tbaa !7
  %21 = getelementptr inbounds [32 x [32 x i16]], ptr @a, i16 0, i16 %20
  %22 = load i16, ptr %4, align 2, !tbaa !7
  %23 = getelementptr inbounds [32 x i16], ptr %21, i16 0, i16 %22
  store i16 %19, ptr %23, align 2, !tbaa !8
  br label %24

24:                                               ; preds = %12
  %25 = load i16, ptr %3, align 2, !tbaa !7
  %26 = add nsw i16 %25, 1
  store i16 %26, ptr %3, align 2, !tbaa !7
  br label %9, !llvm.loop !13

27:                                               ; preds = %9
  br label %28

28:                                               ; preds = %27
  %29 = load i16, ptr %4, align 2, !tbaa !7
  %30 = add nsw i16 %29, 1
  store i16 %30, ptr %4, align 2, !tbaa !7
  br label %5, !llvm.loop !14

31:                                               ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @reads_back(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %32, %1
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = icmp slt i16 %6, 31
  br i1 %7, label %8, label %35

8:                                                ; preds = %5
  store i16 1, ptr %3, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %28, %8
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = icmp slt i16 %10, 32
  br i1 %11, label %12, label %31

12:                                               ; preds = %9
  %13 = load i16, ptr %3, align 2, !tbaa !7
  %14 = sub nsw i16 %13, 1
  %15 = getelementptr inbounds [32 x [32 x i16]], ptr @b, i16 0, i16 %14
  %16 = load i16, ptr %4, align 2, !tbaa !7
  %17 = add nsw i16 %16, 1
  %18 = getelementptr inbounds [32 x i16], ptr %15, i16 0, i16 %17
  %19 = load i16, ptr %18, align 2, !tbaa !8
  %20 = load i16, ptr %3, align 2, !tbaa !7
  %21 = add nsw i16 %19, %20
  %22 = load i16, ptr %2, align 2, !tbaa !8
  %23 = add nsw i16 %21, %22
  %24 = load i16, ptr %3, align 2, !tbaa !7
  %25 = getelementptr inbounds [32 x [32 x i16]], ptr @b, i16 0, i16 %24
  %26 = load i16, ptr %4, align 2, !tbaa !7
  %27 = getelementptr inbounds [32 x i16], ptr %25, i16 0, i16 %26
  store i16 %23, ptr %27, align 2, !tbaa !8
  br label %28

28:                                               ; preds = %12
  %29 = load i16, ptr %3, align 2, !tbaa !7
  %30 = add nsw i16 %29, 1
  store i16 %30, ptr %3, align 2, !tbaa !7
  br label %9, !llvm.loop !15

31:                                               ; preds = %9
  br label %32

32:                                               ; preds = %31
  %33 = load i16, ptr %4, align 2, !tbaa !7
  %34 = add nsw i16 %33, 1
  store i16 %34, ptr %4, align 2, !tbaa !7
  br label %5, !llvm.loop !16

35:                                               ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind optsize
define internal void @triangle(i16 noundef signext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %5

5:                                                ; preds = %33, %1
  %6 = load i16, ptr %4, align 2, !tbaa !7
  %7 = icmp slt i16 %6, 32
  br i1 %7, label %8, label %36

8:                                                ; preds = %5
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %9

9:                                                ; preds = %29, %8
  %10 = load i16, ptr %3, align 2, !tbaa !7
  %11 = load i16, ptr %4, align 2, !tbaa !7
  %12 = icmp slt i16 %10, %11
  br i1 %12, label %13, label %32

13:                                               ; preds = %9
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = getelementptr inbounds [32 x [32 x i16]], ptr @c, i16 0, i16 %14
  %16 = load i16, ptr %4, align 2, !tbaa !7
  %17 = getelementptr inbounds [32 x i16], ptr %15, i16 0, i16 %16
  %18 = load i16, ptr %17, align 2, !tbaa !8
  %19 = load i16, ptr %3, align 2, !tbaa !7
  %20 = load i16, ptr %4, align 2, !tbaa !7
  %21 = mul nsw i16 %19, %20
  %22 = add nsw i16 %18, %21
  %23 = load i16, ptr %2, align 2, !tbaa !8
  %24 = add nsw i16 %22, %23
  %25 = load i16, ptr %3, align 2, !tbaa !7
  %26 = getelementptr inbounds [32 x [32 x i16]], ptr @c, i16 0, i16 %25
  %27 = load i16, ptr %4, align 2, !tbaa !7
  %28 = getelementptr inbounds [32 x i16], ptr %26, i16 0, i16 %27
  store i16 %24, ptr %28, align 2, !tbaa !8
  br label %29

29:                                               ; preds = %13
  %30 = load i16, ptr %3, align 2, !tbaa !7
  %31 = add nsw i16 %30, 1
  store i16 %31, ptr %3, align 2, !tbaa !7
  br label %9, !llvm.loop !17

32:                                               ; preds = %9
  br label %33

33:                                               ; preds = %32
  %34 = load i16, ptr %4, align 2, !tbaa !7
  %35 = add nsw i16 %34, 1
  store i16 %35, ptr %4, align 2, !tbaa !7
  br label %5, !llvm.loop !18

36:                                               ; preds = %5
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @checksum(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store ptr %0, ptr %2, align 2, !tbaa !19
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !22
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %29, %1
  %7 = load i16, ptr %4, align 2, !tbaa !7
  %8 = icmp slt i16 %7, 32
  br i1 %8, label %9, label %32

9:                                                ; preds = %6
  store i16 0, ptr %5, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %25, %9
  %11 = load i16, ptr %5, align 2, !tbaa !7
  %12 = icmp slt i16 %11, 32
  br i1 %12, label %13, label %28

13:                                               ; preds = %10
  %14 = load i32, ptr %3, align 2, !tbaa !22
  %15 = mul nsw i32 %14, 31
  %16 = load ptr, ptr %2, align 2, !tbaa !19
  %17 = load i16, ptr %4, align 2, !tbaa !7
  %18 = getelementptr inbounds [32 x i16], ptr %16, i16 %17
  %19 = load i16, ptr %5, align 2, !tbaa !7
  %20 = getelementptr inbounds [32 x i16], ptr %18, i16 0, i16 %19
  %21 = load i16, ptr %20, align 2, !tbaa !8
  %22 = sext i16 %21 to i32
  %23 = add nsw i32 %15, %22
  %24 = srem i32 %23, 100003
  store i32 %24, ptr %3, align 2, !tbaa !22
  br label %25

25:                                               ; preds = %13
  %26 = load i16, ptr %5, align 2, !tbaa !7
  %27 = add nsw i16 %26, 1
  store i16 %27, ptr %5, align 2, !tbaa !7
  br label %10, !llvm.loop !24

28:                                               ; preds = %10
  br label %29

29:                                               ; preds = %28
  %30 = load i16, ptr %4, align 2, !tbaa !7
  %31 = add nsw i16 %30, 1
  store i16 %31, ptr %4, align 2, !tbaa !7
  br label %6, !llvm.loop !25

32:                                               ; preds = %6
  %33 = load i32, ptr %3, align 2, !tbaa !22
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %33
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
!9 = !{!"short", !5, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = distinct !{!13, !11}
!14 = distinct !{!14, !11}
!15 = distinct !{!15, !11}
!16 = distinct !{!16, !11}
!17 = distinct !{!17, !11}
!18 = distinct !{!18, !11}
!19 = !{!20, !20, i64 0}
!20 = !{!"p1 short", !21, i64 0}
!21 = !{!"any pointer", !5, i64 0}
!22 = !{!23, !23, i64 0}
!23 = !{!"long", !5, i64 0}
!24 = distinct !{!24, !11}
!25 = distinct !{!25, !11}
