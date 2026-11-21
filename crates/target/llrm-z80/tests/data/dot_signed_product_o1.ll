target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = 'bench/dot/dot.c'
source_filename = "bench/dot/dot.c"
target triple = "msp430"

; Function Attrs: nounwind
define dso_local i32 @bench_dot() #0 {
  %1 = alloca [64 x i16], align 2
  %2 = alloca [48 x i16], align 2
  %3 = alloca i32, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %4, align 2, !tbaa !9
  br label %6

6:                                                ; preds = %15, %0
  %7 = load i16, ptr %4, align 2, !tbaa !9
  %8 = icmp slt i16 %7, 64
  br i1 %8, label %9, label %18

9:                                                ; preds = %6
  %10 = load i16, ptr %4, align 2, !tbaa !9
  %11 = mul nsw i16 %10, 3
  %12 = sub nsw i16 %11, 90
  %13 = load i16, ptr %4, align 2, !tbaa !9
  %14 = getelementptr inbounds [64 x i16], ptr %1, i16 0, i16 %13
  store i16 %12, ptr %14, align 2, !tbaa !9
  br label %15

15:                                               ; preds = %9
  %16 = load i16, ptr %4, align 2, !tbaa !9
  %17 = add nsw i16 %16, 1
  store i16 %17, ptr %4, align 2, !tbaa !9
  br label %6, !llvm.loop !11

18:                                               ; preds = %6
  store i16 0, ptr %4, align 2, !tbaa !9
  br label %19

19:                                               ; preds = %28, %18
  %20 = load i16, ptr %4, align 2, !tbaa !9
  %21 = icmp slt i16 %20, 48
  br i1 %21, label %22, label %31

22:                                               ; preds = %19
  %23 = load i16, ptr %4, align 2, !tbaa !9
  %24 = mul nsw i16 %23, 2
  %25 = sub nsw i16 70, %24
  %26 = load i16, ptr %4, align 2, !tbaa !9
  %27 = getelementptr inbounds [48 x i16], ptr %2, i16 0, i16 %26
  store i16 %25, ptr %27, align 2, !tbaa !9
  br label %28

28:                                               ; preds = %22
  %29 = load i16, ptr %4, align 2, !tbaa !9
  %30 = add nsw i16 %29, 1
  store i16 %30, ptr %4, align 2, !tbaa !9
  br label %19, !llvm.loop !14

31:                                               ; preds = %19
  store i16 0, ptr %5, align 2, !tbaa !9
  br label %32

32:                                               ; preds = %43, %31
  %33 = load i16, ptr %5, align 2, !tbaa !9
  %34 = icmp slt i16 %33, 40
  br i1 %34, label %35, label %46

35:                                               ; preds = %32
  %36 = load i16, ptr %5, align 2, !tbaa !9
  %37 = getelementptr inbounds [64 x i16], ptr %1, i16 0, i16 0
  store i16 %36, ptr %37, align 2, !tbaa !9
  %38 = getelementptr inbounds [64 x i16], ptr %1, i16 0, i16 0
  %39 = getelementptr inbounds [48 x i16], ptr %2, i16 0, i16 0
  %40 = call i32 @dot(ptr noundef %38, i16 noundef signext 64, ptr noundef %39, i16 noundef signext 48)
  %41 = load i32, ptr %3, align 2, !tbaa !7
  %42 = add nsw i32 %41, %40
  store i32 %42, ptr %3, align 2, !tbaa !7
  br label %43

43:                                               ; preds = %35
  %44 = load i16, ptr %5, align 2, !tbaa !9
  %45 = add nsw i16 %44, 1
  store i16 %45, ptr %5, align 2, !tbaa !9
  br label %32, !llvm.loop !15

46:                                               ; preds = %32
  %47 = load i32, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %47
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal i32 @dot(ptr noundef %0, i16 noundef signext %1, ptr noundef %2, i16 noundef signext %3) #0 {
  %5 = alloca ptr, align 2
  %6 = alloca i16, align 2
  %7 = alloca ptr, align 2
  %8 = alloca i16, align 2
  %9 = alloca i32, align 2
  %10 = alloca i16, align 2
  %11 = alloca i16, align 2
  store ptr %0, ptr %5, align 2, !tbaa !16
  store i16 %1, ptr %6, align 2, !tbaa !9
  store ptr %2, ptr %7, align 2, !tbaa !16
  store i16 %3, ptr %8, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 0, ptr %9, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %10) #3
  %12 = load i16, ptr %6, align 2, !tbaa !9
  %13 = load i16, ptr %8, align 2, !tbaa !9
  %14 = icmp slt i16 %12, %13
  br i1 %14, label %15, label %17

15:                                               ; preds = %4
  %16 = load i16, ptr %6, align 2, !tbaa !9
  br label %19

17:                                               ; preds = %4
  %18 = load i16, ptr %8, align 2, !tbaa !9
  br label %19

19:                                               ; preds = %17, %15
  %20 = phi i16 [ %16, %15 ], [ %18, %17 ]
  store i16 %20, ptr %10, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %11) #3
  store i16 0, ptr %11, align 2, !tbaa !9
  br label %21

21:                                               ; preds = %39, %19
  %22 = load i16, ptr %11, align 2, !tbaa !9
  %23 = load i16, ptr %10, align 2, !tbaa !9
  %24 = icmp slt i16 %22, %23
  br i1 %24, label %25, label %42

25:                                               ; preds = %21
  %26 = load ptr, ptr %5, align 2, !tbaa !16
  %27 = load i16, ptr %11, align 2, !tbaa !9
  %28 = getelementptr inbounds i16, ptr %26, i16 %27
  %29 = load i16, ptr %28, align 2, !tbaa !9
  %30 = sext i16 %29 to i32
  %31 = load ptr, ptr %7, align 2, !tbaa !16
  %32 = load i16, ptr %11, align 2, !tbaa !9
  %33 = getelementptr inbounds i16, ptr %31, i16 %32
  %34 = load i16, ptr %33, align 2, !tbaa !9
  %35 = sext i16 %34 to i32
  %36 = mul nsw i32 %30, %35
  %37 = load i32, ptr %9, align 2, !tbaa !7
  %38 = add nsw i32 %37, %36
  store i32 %38, ptr %9, align 2, !tbaa !7
  br label %39

39:                                               ; preds = %25
  %40 = load i16, ptr %11, align 2, !tbaa !9
  %41 = add nsw i16 %40, 1
  store i16 %41, ptr %11, align 2, !tbaa !9
  br label %21, !llvm.loop !19

42:                                               ; preds = %21
  %43 = load i32, ptr %9, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %11) #3
  call void @llvm.lifetime.end.p0(ptr %10) #3
  call void @llvm.lifetime.end.p0(ptr %9) #3
  ret i32 %43
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @bench_dot()
  call void @report(i32 noundef %2)
  ret i16 0
}

declare dso_local void @report(i32 noundef) #2

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
!7 = !{!8, !8, i64 0}
!8 = !{!"long", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"short", !5, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
!14 = distinct !{!14, !12, !13}
!15 = distinct !{!15, !12, !13}
!16 = !{!17, !17, i64 0}
!17 = !{!"p1 short", !18, i64 0}
!18 = !{!"any pointer", !5, i64 0}
!19 = distinct !{!19, !12, !13}
