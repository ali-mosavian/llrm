target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i32 @words() #0 {
  %1 = alloca [64 x [64 x i16]], align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !7
  store i16 0, ptr %2, align 2, !tbaa !9
  br label %5

5:                                                ; preds = %21, %0
  %6 = load i16, ptr %2, align 2, !tbaa !9
  %7 = icmp slt i16 %6, 64
  br i1 %7, label %8, label %24

8:                                                ; preds = %5
  store i16 0, ptr %3, align 2, !tbaa !9
  br label %9

9:                                                ; preds = %17, %8
  %10 = load i16, ptr %3, align 2, !tbaa !9
  %11 = icmp slt i16 %10, 64
  br i1 %11, label %12, label %20

12:                                               ; preds = %9
  %13 = load i16, ptr %2, align 2, !tbaa !9
  %14 = getelementptr inbounds [64 x [64 x i16]], ptr %1, i16 0, i16 %13
  %15 = load i16, ptr %3, align 2, !tbaa !9
  %16 = getelementptr inbounds [64 x i16], ptr %14, i16 0, i16 %15
  store i16 4660, ptr %16, align 2, !tbaa !9
  br label %17

17:                                               ; preds = %12
  %18 = load i16, ptr %3, align 2, !tbaa !9
  %19 = add nsw i16 %18, 1
  store i16 %19, ptr %3, align 2, !tbaa !9
  br label %9, !llvm.loop !11

20:                                               ; preds = %9
  br label %21

21:                                               ; preds = %20
  %22 = load i16, ptr %2, align 2, !tbaa !9
  %23 = add nsw i16 %22, 1
  store i16 %23, ptr %2, align 2, !tbaa !9
  br label %5, !llvm.loop !13

24:                                               ; preds = %5
  store i16 0, ptr %2, align 2, !tbaa !9
  br label %25

25:                                               ; preds = %41, %24
  %26 = load i16, ptr %2, align 2, !tbaa !9
  %27 = icmp slt i16 %26, 64
  br i1 %27, label %28, label %44

28:                                               ; preds = %25
  %29 = load i16, ptr %2, align 2, !tbaa !9
  %30 = getelementptr inbounds [64 x [64 x i16]], ptr %1, i16 0, i16 %29
  %31 = getelementptr inbounds [64 x i16], ptr %30, i16 0, i16 0
  %32 = load i16, ptr %31, align 2, !tbaa !9
  %33 = load i16, ptr %2, align 2, !tbaa !9
  %34 = getelementptr inbounds [64 x [64 x i16]], ptr %1, i16 0, i16 %33
  %35 = getelementptr inbounds [64 x i16], ptr %34, i16 0, i16 63
  %36 = load i16, ptr %35, align 2, !tbaa !9
  %37 = add nsw i16 %32, %36
  %38 = sext i16 %37 to i32
  %39 = load i32, ptr %4, align 2, !tbaa !7
  %40 = add nsw i32 %39, %38
  store i32 %40, ptr %4, align 2, !tbaa !7
  br label %41

41:                                               ; preds = %28
  %42 = load i16, ptr %2, align 2, !tbaa !9
  %43 = add nsw i16 %42, 1
  store i16 %43, ptr %2, align 2, !tbaa !9
  br label %25, !llvm.loop !14

44:                                               ; preds = %25
  %45 = load i32, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %45
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i32 @bytes() #0 {
  %1 = alloca [64 x [64 x i8]], align 1
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store i32 0, ptr %4, align 2, !tbaa !7
  store i16 0, ptr %2, align 2, !tbaa !9
  br label %5

5:                                                ; preds = %21, %0
  %6 = load i16, ptr %2, align 2, !tbaa !9
  %7 = icmp slt i16 %6, 64
  br i1 %7, label %8, label %24

8:                                                ; preds = %5
  store i16 0, ptr %3, align 2, !tbaa !9
  br label %9

9:                                                ; preds = %17, %8
  %10 = load i16, ptr %3, align 2, !tbaa !9
  %11 = icmp slt i16 %10, 64
  br i1 %11, label %12, label %20

12:                                               ; preds = %9
  %13 = load i16, ptr %2, align 2, !tbaa !9
  %14 = getelementptr inbounds [64 x [64 x i8]], ptr %1, i16 0, i16 %13
  %15 = load i16, ptr %3, align 2, !tbaa !9
  %16 = getelementptr inbounds [64 x i8], ptr %14, i16 0, i16 %15
  store i8 65, ptr %16, align 1, !tbaa !15
  br label %17

17:                                               ; preds = %12
  %18 = load i16, ptr %3, align 2, !tbaa !9
  %19 = add nsw i16 %18, 1
  store i16 %19, ptr %3, align 2, !tbaa !9
  br label %9, !llvm.loop !16

20:                                               ; preds = %9
  br label %21

21:                                               ; preds = %20
  %22 = load i16, ptr %2, align 2, !tbaa !9
  %23 = add nsw i16 %22, 1
  store i16 %23, ptr %2, align 2, !tbaa !9
  br label %5, !llvm.loop !17

24:                                               ; preds = %5
  store i16 0, ptr %2, align 2, !tbaa !9
  br label %25

25:                                               ; preds = %43, %24
  %26 = load i16, ptr %2, align 2, !tbaa !9
  %27 = icmp slt i16 %26, 64
  br i1 %27, label %28, label %46

28:                                               ; preds = %25
  %29 = load i16, ptr %2, align 2, !tbaa !9
  %30 = getelementptr inbounds [64 x [64 x i8]], ptr %1, i16 0, i16 %29
  %31 = getelementptr inbounds [64 x i8], ptr %30, i16 0, i16 0
  %32 = load i8, ptr %31, align 1, !tbaa !15
  %33 = zext i8 %32 to i16
  %34 = load i16, ptr %2, align 2, !tbaa !9
  %35 = getelementptr inbounds [64 x [64 x i8]], ptr %1, i16 0, i16 %34
  %36 = getelementptr inbounds [64 x i8], ptr %35, i16 0, i16 63
  %37 = load i8, ptr %36, align 1, !tbaa !15
  %38 = zext i8 %37 to i16
  %39 = add nsw i16 %33, %38
  %40 = sext i16 %39 to i32
  %41 = load i32, ptr %4, align 2, !tbaa !7
  %42 = add nsw i32 %41, %40
  store i32 %42, ptr %4, align 2, !tbaa !7
  br label %43

43:                                               ; preds = %28
  %44 = load i16, ptr %2, align 2, !tbaa !9
  %45 = add nsw i16 %44, 1
  store i16 %45, ptr %2, align 2, !tbaa !9
  br label %25, !llvm.loop !18

46:                                               ; preds = %25
  %47 = load i32, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %47
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @words() #4
  call void @report(i32 noundef %2) #4
  %3 = call i32 @bytes() #4
  call void @report(i32 noundef %3) #4
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

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
!10 = !{!"short", !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = distinct !{!14, !12}
!15 = !{!5, !5, i64 0}
!16 = distinct !{!16, !12}
!17 = distinct !{!17, !12}
!18 = distinct !{!18, !12}
