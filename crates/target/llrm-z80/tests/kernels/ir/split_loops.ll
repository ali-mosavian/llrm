target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
; ModuleID = '/home/alim/scratch/wt-z80rot/tests/run/c/split_loops.c'
source_filename = "/home/alim/scratch/wt-z80rot/tests/run/c/split_loops.c"
target triple = "msp430"

@counts = internal global [64 x i16] zeroinitializer, align 2

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @below(i16 noundef signext 10, i16 noundef signext 4)
  call void @report(i32 noundef %2)
  %3 = call i32 @below(i16 noundef signext 10, i16 noundef signext 0)
  call void @report(i32 noundef %3)
  %4 = call i32 @below(i16 noundef signext 10, i16 noundef signext 20)
  call void @report(i32 noundef %4)
  %5 = call i32 @below(i16 noundef signext 10, i16 noundef signext -3)
  call void @report(i32 noundef %5)
  %6 = call i32 @from(i16 noundef signext 10, i16 noundef signext 4)
  call void @report(i32 noundef %6)
  %7 = call i32 @from(i16 noundef signext 10, i16 noundef signext 0)
  call void @report(i32 noundef %7)
  %8 = call i32 @from(i16 noundef signext 10, i16 noundef signext 20)
  call void @report(i32 noundef %8)
  %9 = call i32 @from(i16 noundef signext 0, i16 noundef signext 3)
  call void @report(i32 noundef %9)
  %10 = call i32 @marks(i16 noundef zeroext 100)
  call void @report(i32 noundef %10)
  %11 = call i32 @marks(i16 noundef zeroext 20)
  call void @report(i32 noundef %11)
  ret i16 0
}

declare dso_local void @report(i32 noundef) #1

; Function Attrs: nounwind
define internal i32 @below(i16 noundef signext %0, i16 noundef signext %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i16 0, ptr %6, align 2, !tbaa !7
  br label %7

7:                                                ; preds = %26, %2
  %8 = load i16, ptr %6, align 2, !tbaa !7
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %11, label %29

11:                                               ; preds = %7
  %12 = load i16, ptr %6, align 2, !tbaa !7
  %13 = load i16, ptr %4, align 2, !tbaa !7
  %14 = icmp slt i16 %12, %13
  br i1 %14, label %15, label %18

15:                                               ; preds = %11
  %16 = load i16, ptr %6, align 2, !tbaa !7
  %17 = mul nsw i16 %16, 3
  br label %21

18:                                               ; preds = %11
  %19 = load i16, ptr %6, align 2, !tbaa !7
  %20 = add nsw i16 %19, 7
  br label %21

21:                                               ; preds = %18, %15
  %22 = phi i16 [ %17, %15 ], [ %20, %18 ]
  %23 = sext i16 %22 to i32
  %24 = load i32, ptr %5, align 2, !tbaa !9
  %25 = add nsw i32 %24, %23
  store i32 %25, ptr %5, align 2, !tbaa !9
  br label %26

26:                                               ; preds = %21
  %27 = load i16, ptr %6, align 2, !tbaa !7
  %28 = add nsw i16 %27, 1
  store i16 %28, ptr %6, align 2, !tbaa !7
  br label %7, !llvm.loop !11

29:                                               ; preds = %7
  %30 = load i32, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %30
}

; Function Attrs: nounwind
define internal i32 @from(i16 noundef signext %0, i16 noundef signext %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  %6 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %6) #3
  store i16 0, ptr %6, align 2, !tbaa !7
  br label %7

7:                                                ; preds = %24, %2
  %8 = load i16, ptr %6, align 2, !tbaa !7
  %9 = load i16, ptr %3, align 2, !tbaa !7
  %10 = icmp slt i16 %8, %9
  br i1 %10, label %11, label %27

11:                                               ; preds = %7
  %12 = load i16, ptr %6, align 2, !tbaa !7
  %13 = load i16, ptr %4, align 2, !tbaa !7
  %14 = icmp sge i16 %12, %13
  br i1 %14, label %15, label %18

15:                                               ; preds = %11
  %16 = load i16, ptr %6, align 2, !tbaa !7
  %17 = mul nsw i16 %16, 5
  br label %19

18:                                               ; preds = %11
  br label %19

19:                                               ; preds = %18, %15
  %20 = phi i16 [ %17, %15 ], [ 1, %18 ]
  %21 = sext i16 %20 to i32
  %22 = load i32, ptr %5, align 2, !tbaa !9
  %23 = add nsw i32 %22, %21
  store i32 %23, ptr %5, align 2, !tbaa !9
  br label %24

24:                                               ; preds = %19
  %25 = load i16, ptr %6, align 2, !tbaa !7
  %26 = add nsw i16 %25, 1
  store i16 %26, ptr %6, align 2, !tbaa !7
  br label %7, !llvm.loop !14

27:                                               ; preds = %7
  %28 = load i32, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret i32 %28
}

; Function Attrs: nounwind
define internal i32 @marks(i16 noundef zeroext %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !7
  store i16 2, ptr %3, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %36, %1
  %7 = load i16, ptr %3, align 2, !tbaa !7
  %8 = load i16, ptr %2, align 2, !tbaa !7
  %9 = icmp ult i16 %7, %8
  br i1 %9, label %10, label %39

10:                                               ; preds = %6
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = icmp ule i16 %11, 31
  br i1 %12, label %13, label %32

13:                                               ; preds = %10
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = load i16, ptr %3, align 2, !tbaa !7
  %16 = mul i16 %14, %15
  store i16 %16, ptr %4, align 2, !tbaa !7
  br label %17

17:                                               ; preds = %27, %13
  %18 = load i16, ptr %4, align 2, !tbaa !7
  %19 = load i16, ptr %2, align 2, !tbaa !7
  %20 = icmp ult i16 %18, %19
  br i1 %20, label %21, label %31

21:                                               ; preds = %17
  %22 = load i16, ptr %4, align 2, !tbaa !7
  %23 = and i16 %22, 63
  %24 = getelementptr inbounds nuw [64 x i16], ptr @counts, i16 0, i16 %23
  %25 = load i16, ptr %24, align 2, !tbaa !7
  %26 = add i16 %25, 1
  store i16 %26, ptr %24, align 2, !tbaa !7
  br label %27

27:                                               ; preds = %21
  %28 = load i16, ptr %3, align 2, !tbaa !7
  %29 = load i16, ptr %4, align 2, !tbaa !7
  %30 = add i16 %29, %28
  store i16 %30, ptr %4, align 2, !tbaa !7
  br label %17, !llvm.loop !15

31:                                               ; preds = %17
  br label %35

32:                                               ; preds = %10
  %33 = load i16, ptr %5, align 2, !tbaa !7
  %34 = add i16 %33, 1
  store i16 %34, ptr %5, align 2, !tbaa !7
  br label %35

35:                                               ; preds = %32, %31
  br label %36

36:                                               ; preds = %35
  %37 = load i16, ptr %3, align 2, !tbaa !7
  %38 = add i16 %37, 1
  store i16 %38, ptr %3, align 2, !tbaa !7
  br label %6, !llvm.loop !16

39:                                               ; preds = %6
  %40 = load i16, ptr %5, align 2, !tbaa !7
  %41 = load i16, ptr getelementptr inbounds nuw (i8, ptr @counts, i16 10), align 2, !tbaa !7
  %42 = add i16 %40, %41
  %43 = load i16, ptr getelementptr inbounds nuw (i8, ptr @counts, i16 34), align 2, !tbaa !7
  %44 = add i16 %42, %43
  %45 = zext i16 %44 to i32
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %45
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
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
!8 = !{!"short", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"long", !5, i64 0}
!11 = distinct !{!11, !12, !13}
!12 = !{!"llvm.loop.mustprogress"}
!13 = !{!"llvm.loop.unroll.disable"}
!14 = distinct !{!14, !12, !13}
!15 = distinct !{!15, !12, !13}
!16 = distinct !{!16, !12, !13}
