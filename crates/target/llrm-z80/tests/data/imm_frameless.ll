target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = '/home/alim/scratch/wt-align/tests/run/c/frameless.c'
source_filename = "/home/alim/scratch/wt-align/tests/run/c/frameless.c"
target triple = "msp430"

; Function Attrs: noinline nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 0, ptr %2, align 2, !tbaa !7
  %3 = call i32 @mix(i32 noundef 1, i32 noundef 2, i32 noundef 3)
  call void @report(i32 noundef %3)
  %4 = call i32 @chain(i32 noundef 5)
  call void @report(i32 noundef %4)
  %5 = call i32 @ring(i32 noundef 2)
  call void @report(i32 noundef %5)
  %6 = call i32 @address_taken(i32 noundef 3)
  call void @report(i32 noundef %6)
  %7 = call i32 @descend(i32 noundef 10, ptr noundef %2)
  call void @report(i32 noundef %7)
  %8 = load i32, ptr %2, align 2, !tbaa !7
  call void @report(i32 noundef %8)
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare dso_local void @report(i32 noundef) #2

; Function Attrs: noinline nounwind
define internal i32 @mix(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca [4 x i32], align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store i32 %1, ptr %5, align 2, !tbaa !7
  store i32 %2, ptr %6, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %8 = load i32, ptr %4, align 2, !tbaa !7
  %9 = getelementptr inbounds [4 x i32], ptr %7, i16 0, i16 0
  store i32 %8, ptr %9, align 2, !tbaa !7
  %10 = load i32, ptr %5, align 2, !tbaa !7
  %11 = getelementptr inbounds [4 x i32], ptr %7, i16 0, i16 1
  store i32 %10, ptr %11, align 2, !tbaa !7
  %12 = load i32, ptr %6, align 2, !tbaa !7
  %13 = getelementptr inbounds [4 x i32], ptr %7, i16 0, i16 2
  store i32 %12, ptr %13, align 2, !tbaa !7
  %14 = load i32, ptr %4, align 2, !tbaa !7
  %15 = load i32, ptr %5, align 2, !tbaa !7
  %16 = load i32, ptr %6, align 2, !tbaa !7
  %17 = load i32, ptr %4, align 2, !tbaa !7
  %18 = load i32, ptr %5, align 2, !tbaa !7
  %19 = add nsw i32 %17, %18
  %20 = call i32 @add4(i32 noundef %14, i32 noundef %15, i32 noundef %16, i32 noundef %19)
  %21 = getelementptr inbounds [4 x i32], ptr %7, i16 0, i16 3
  store i32 %20, ptr %21, align 2, !tbaa !7
  %22 = getelementptr inbounds [4 x i32], ptr %7, i16 0, i16 0
  %23 = load i32, ptr %22, align 2, !tbaa !7
  %24 = mul nsw i32 %23, 3
  %25 = getelementptr inbounds [4 x i32], ptr %7, i16 0, i16 1
  %26 = load i32, ptr %25, align 2, !tbaa !7
  %27 = mul nsw i32 %26, 5
  %28 = add nsw i32 %24, %27
  %29 = getelementptr inbounds [4 x i32], ptr %7, i16 0, i16 2
  %30 = load i32, ptr %29, align 2, !tbaa !7
  %31 = mul nsw i32 %30, 7
  %32 = add nsw i32 %28, %31
  %33 = getelementptr inbounds [4 x i32], ptr %7, i16 0, i16 3
  %34 = load i32, ptr %33, align 2, !tbaa !7
  %35 = add nsw i32 %32, %34
  call void @llvm.lifetime.end.p0(ptr %7) #3
  ret i32 %35
}

; Function Attrs: noinline nounwind
define internal i32 @chain(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  %3 = alloca [12 x i32], align 2
  %4 = alloca i32, align 2
  %5 = alloca i16, align 2
  store i32 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %6

6:                                                ; preds = %16, %1
  %7 = load i16, ptr %5, align 2, !tbaa !9
  %8 = icmp slt i16 %7, 12
  br i1 %8, label %9, label %19

9:                                                ; preds = %6
  %10 = load i32, ptr %2, align 2, !tbaa !7
  %11 = load i16, ptr %5, align 2, !tbaa !9
  %12 = sext i16 %11 to i32
  %13 = add nsw i32 %10, %12
  %14 = load i16, ptr %5, align 2, !tbaa !9
  %15 = getelementptr inbounds [12 x i32], ptr %3, i16 0, i16 %14
  store i32 %13, ptr %15, align 2, !tbaa !7
  br label %16

16:                                               ; preds = %9
  %17 = load i16, ptr %5, align 2, !tbaa !9
  %18 = add nsw i16 %17, 1
  store i16 %18, ptr %5, align 2, !tbaa !9
  br label %6, !llvm.loop !10

19:                                               ; preds = %6
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %20

20:                                               ; preds = %37, %19
  %21 = load i16, ptr %5, align 2, !tbaa !9
  %22 = icmp slt i16 %21, 12
  br i1 %22, label %23, label %40

23:                                               ; preds = %20
  %24 = load i16, ptr %5, align 2, !tbaa !9
  %25 = getelementptr inbounds [12 x i32], ptr %3, i16 0, i16 %24
  %26 = load i32, ptr %25, align 2, !tbaa !7
  %27 = load i16, ptr %5, align 2, !tbaa !9
  %28 = sub nsw i16 11, %27
  %29 = getelementptr inbounds [12 x i32], ptr %3, i16 0, i16 %28
  %30 = load i32, ptr %29, align 2, !tbaa !7
  %31 = load i32, ptr %2, align 2, !tbaa !7
  %32 = load i16, ptr %5, align 2, !tbaa !9
  %33 = sext i16 %32 to i32
  %34 = call i32 @add4(i32 noundef %26, i32 noundef %30, i32 noundef %31, i32 noundef %33)
  %35 = load i32, ptr %4, align 2, !tbaa !7
  %36 = add nsw i32 %35, %34
  store i32 %36, ptr %4, align 2, !tbaa !7
  br label %37

37:                                               ; preds = %23
  %38 = load i16, ptr %5, align 2, !tbaa !9
  %39 = add nsw i16 %38, 1
  store i16 %39, ptr %5, align 2, !tbaa !9
  br label %20, !llvm.loop !12

40:                                               ; preds = %20
  %41 = load i32, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %41
}

; Function Attrs: noinline nounwind
define internal i32 @ring(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  %3 = alloca [64 x i32], align 2
  %4 = alloca i32, align 2
  %5 = alloca i16, align 2
  store i32 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %6

6:                                                ; preds = %19, %1
  %7 = load i16, ptr %5, align 2, !tbaa !9
  %8 = icmp slt i16 %7, 64
  br i1 %8, label %9, label %22

9:                                                ; preds = %6
  %10 = load i16, ptr %5, align 2, !tbaa !9
  %11 = add nsw i16 %10, 1
  %12 = sext i16 %11 to i32
  %13 = mul nsw i32 %12, 3
  %14 = sub nsw i32 %13, 7
  %15 = load i32, ptr %2, align 2, !tbaa !7
  %16 = add nsw i32 %14, %15
  %17 = load i16, ptr %5, align 2, !tbaa !9
  %18 = getelementptr inbounds [64 x i32], ptr %3, i16 0, i16 %17
  store i32 %16, ptr %18, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %9
  %20 = load i16, ptr %5, align 2, !tbaa !9
  %21 = add nsw i16 %20, 1
  store i16 %21, ptr %5, align 2, !tbaa !9
  br label %6, !llvm.loop !13

22:                                               ; preds = %6
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %23

23:                                               ; preds = %35, %22
  %24 = load i16, ptr %5, align 2, !tbaa !9
  %25 = icmp slt i16 %24, 300
  br i1 %25, label %26, label %38

26:                                               ; preds = %23
  %27 = load i16, ptr %5, align 2, !tbaa !9
  %28 = mul nsw i16 %27, 5
  %29 = add nsw i16 %28, 3
  %30 = and i16 %29, 63
  %31 = getelementptr inbounds [64 x i32], ptr %3, i16 0, i16 %30
  %32 = load i32, ptr %31, align 2, !tbaa !7
  %33 = load i32, ptr %4, align 2, !tbaa !7
  %34 = add nsw i32 %33, %32
  store i32 %34, ptr %4, align 2, !tbaa !7
  br label %35

35:                                               ; preds = %26
  %36 = load i16, ptr %5, align 2, !tbaa !9
  %37 = add nsw i16 %36, 1
  store i16 %37, ptr %5, align 2, !tbaa !9
  br label %23, !llvm.loop !14

38:                                               ; preds = %23
  %39 = load i32, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %39
}

; Function Attrs: noinline nounwind
define internal i32 @address_taken(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  %3 = alloca [8 x i32], align 2
  %4 = alloca i32, align 2
  %5 = alloca i16, align 2
  store i32 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  %6 = getelementptr inbounds [8 x i32], ptr %3, i16 0, i16 0
  call void @fill(ptr noundef %6, i16 noundef 8)
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %7

7:                                                ; preds = %18, %1
  %8 = load i16, ptr %5, align 2, !tbaa !9
  %9 = icmp slt i16 %8, 8
  br i1 %9, label %10, label %21

10:                                               ; preds = %7
  %11 = load i16, ptr %5, align 2, !tbaa !9
  %12 = getelementptr inbounds [8 x i32], ptr %3, i16 0, i16 %11
  %13 = load i32, ptr %12, align 2, !tbaa !7
  %14 = load i32, ptr %2, align 2, !tbaa !7
  %15 = mul nsw i32 %13, %14
  %16 = load i32, ptr %4, align 2, !tbaa !7
  %17 = add nsw i32 %16, %15
  store i32 %17, ptr %4, align 2, !tbaa !7
  br label %18

18:                                               ; preds = %10
  %19 = load i16, ptr %5, align 2, !tbaa !9
  %20 = add nsw i16 %19, 1
  store i16 %20, ptr %5, align 2, !tbaa !9
  br label %7, !llvm.loop !15

21:                                               ; preds = %7
  %22 = load i32, ptr %4, align 2, !tbaa !7
  %23 = load i32, ptr %2, align 2, !tbaa !7
  %24 = call i32 @mix(i32 noundef %23, i32 noundef 1, i32 noundef 2)
  %25 = add nsw i32 %22, %24
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %25
}

; Function Attrs: noinline nounwind
define internal i32 @descend(i32 noundef %0, ptr noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !7
  store ptr %1, ptr %5, align 2, !tbaa !16
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %8 = load i32, ptr %4, align 2, !tbaa !7
  %9 = mul nsw i32 %8, 2
  store i32 %9, ptr %6, align 2, !tbaa !7
  %10 = load i32, ptr %4, align 2, !tbaa !7
  %11 = icmp eq i32 %10, 0
  br i1 %11, label %12, label %15

12:                                               ; preds = %2
  %13 = load ptr, ptr %5, align 2, !tbaa !16
  %14 = load i32, ptr %13, align 2, !tbaa !7
  store i32 %14, ptr %3, align 2
  store i32 1, ptr %7, align 2
  br label %28

15:                                               ; preds = %2
  %16 = load i32, ptr %6, align 2, !tbaa !7
  %17 = load ptr, ptr %5, align 2, !tbaa !16
  %18 = load i32, ptr %17, align 2, !tbaa !7
  %19 = add nsw i32 %18, %16
  store i32 %19, ptr %17, align 2, !tbaa !7
  %20 = load i32, ptr %4, align 2, !tbaa !7
  %21 = load i32, ptr %4, align 2, !tbaa !7
  %22 = sub nsw i32 %21, 1
  %23 = load ptr, ptr %5, align 2, !tbaa !16
  %24 = call i32 @descend(i32 noundef %22, ptr noundef %23)
  %25 = sub nsw i32 %20, %24
  %26 = load i32, ptr %6, align 2, !tbaa !7
  %27 = add nsw i32 %25, %26
  store i32 %27, ptr %3, align 2
  store i32 1, ptr %7, align 2
  br label %28

28:                                               ; preds = %15, %12
  call void @llvm.lifetime.end.p0(ptr %6) #3
  %29 = load i32, ptr %3, align 2
  ret i32 %29
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: noinline nounwind
define internal i32 @add4(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3) #0 {
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  store i32 %0, ptr %5, align 2, !tbaa !7
  store i32 %1, ptr %6, align 2, !tbaa !7
  store i32 %2, ptr %7, align 2, !tbaa !7
  store i32 %3, ptr %8, align 2, !tbaa !7
  %9 = load i32, ptr %5, align 2, !tbaa !7
  %10 = load i32, ptr %6, align 2, !tbaa !7
  %11 = mul nsw i32 %10, 2
  %12 = add nsw i32 %9, %11
  %13 = load i32, ptr %7, align 2, !tbaa !7
  %14 = mul nsw i32 %13, 3
  %15 = add nsw i32 %12, %14
  %16 = load i32, ptr %8, align 2, !tbaa !7
  %17 = mul nsw i32 %16, 4
  %18 = add nsw i32 %15, %17
  ret i32 %18
}

; Function Attrs: noinline nounwind
define internal void @fill(ptr noundef %0, i16 noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !16
  store i16 %1, ptr %4, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %6

6:                                                ; preds = %19, %2
  %7 = load i16, ptr %5, align 2, !tbaa !9
  %8 = load i16, ptr %4, align 2, !tbaa !9
  %9 = icmp slt i16 %7, %8
  br i1 %9, label %10, label %22

10:                                               ; preds = %6
  %11 = load i16, ptr %5, align 2, !tbaa !9
  %12 = load i16, ptr %5, align 2, !tbaa !9
  %13 = mul nsw i16 %11, %12
  %14 = add nsw i16 %13, 1
  %15 = sext i16 %14 to i32
  %16 = load ptr, ptr %3, align 2, !tbaa !16
  %17 = load i16, ptr %5, align 2, !tbaa !9
  %18 = getelementptr inbounds i32, ptr %16, i16 %17
  store i32 %15, ptr %18, align 2, !tbaa !7
  br label %19

19:                                               ; preds = %10
  %20 = load i16, ptr %5, align 2, !tbaa !9
  %21 = add nsw i16 %20, 1
  store i16 %21, ptr %5, align 2, !tbaa !9
  br label %6, !llvm.loop !19

22:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

attributes #0 = { noinline nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
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
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = distinct !{!13, !11}
!14 = distinct !{!14, !11}
!15 = distinct !{!15, !11}
!16 = !{!17, !17, i64 0}
!17 = !{!"p1 long", !18, i64 0}
!18 = !{!"any pointer", !5, i64 0}
!19 = distinct !{!19, !11}
