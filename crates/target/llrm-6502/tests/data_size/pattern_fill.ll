target triple = "msp430"

@a = dso_local global [64 x i16] zeroinitializer, align 2
@b = dso_local global [64 x i32] zeroinitializer, align 2
@main.counts = internal constant [6 x i16] [i16 0, i16 1, i16 2, i16 17, i16 63, i16 64], align 2

; Function Attrs: nounwind optsize
define dso_local void @fillw(i16 noundef signext %0, i16 noundef signext %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %14, %2
  %7 = load i16, ptr %5, align 2, !tbaa !7
  %8 = load i16, ptr %3, align 2, !tbaa !7
  %9 = icmp slt i16 %7, %8
  br i1 %9, label %10, label %17

10:                                               ; preds = %6
  %11 = load i16, ptr %4, align 2, !tbaa !7
  %12 = load i16, ptr %5, align 2, !tbaa !7
  %13 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %12
  store i16 %11, ptr %13, align 2, !tbaa !7
  br label %14

14:                                               ; preds = %10
  %15 = load i16, ptr %5, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %5, align 2, !tbaa !7
  br label %6, !llvm.loop !9

17:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local void @filll(i16 noundef signext %0, i32 noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca i32, align 2
  %5 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store i32 %1, ptr %4, align 2, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %5, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %14, %2
  %7 = load i16, ptr %5, align 2, !tbaa !7
  %8 = load i16, ptr %3, align 2, !tbaa !7
  %9 = icmp slt i16 %7, %8
  br i1 %9, label %10, label %17

10:                                               ; preds = %6
  %11 = load i32, ptr %4, align 2, !tbaa !11
  %12 = load i16, ptr %5, align 2, !tbaa !7
  %13 = getelementptr inbounds [64 x i32], ptr @b, i16 0, i16 %12
  store i32 %11, ptr %13, align 2, !tbaa !11
  br label %14

14:                                               ; preds = %10
  %15 = load i16, ptr %5, align 2, !tbaa !7
  %16 = add nsw i16 %15, 1
  store i16 %16, ptr %5, align 2, !tbaa !7
  br label %6, !llvm.loop !13

17:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i16 0, ptr %2, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %58, %0
  %7 = load i16, ptr %2, align 2, !tbaa !7
  %8 = icmp slt i16 %7, 6
  br i1 %8, label %9, label %61

9:                                                ; preds = %6
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %10

10:                                               ; preds = %18, %9
  %11 = load i16, ptr %3, align 2, !tbaa !7
  %12 = icmp slt i16 %11, 64
  br i1 %12, label %13, label %21

13:                                               ; preds = %10
  %14 = load i16, ptr %3, align 2, !tbaa !7
  %15 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %14
  store i16 0, ptr %15, align 2, !tbaa !7
  %16 = load i16, ptr %3, align 2, !tbaa !7
  %17 = getelementptr inbounds [64 x i32], ptr @b, i16 0, i16 %16
  store i32 0, ptr %17, align 2, !tbaa !11
  br label %18

18:                                               ; preds = %13
  %19 = load i16, ptr %3, align 2, !tbaa !7
  %20 = add nsw i16 %19, 1
  store i16 %20, ptr %3, align 2, !tbaa !7
  br label %10, !llvm.loop !14

21:                                               ; preds = %10
  %22 = load i16, ptr %2, align 2, !tbaa !7
  %23 = getelementptr inbounds [6 x i16], ptr @main.counts, i16 0, i16 %22
  %24 = load i16, ptr %23, align 2, !tbaa !7
  call void @fillw(i16 noundef signext %24, i16 noundef signext 4660) #4
  %25 = load i16, ptr %2, align 2, !tbaa !7
  %26 = getelementptr inbounds [6 x i16], ptr @main.counts, i16 0, i16 %25
  %27 = load i16, ptr %26, align 2, !tbaa !7
  call void @filll(i16 noundef signext %27, i32 noundef 305419896) #4
  store i32 0, ptr %5, align 2, !tbaa !11
  store i16 0, ptr %4, align 2, !tbaa !7
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %28

28:                                               ; preds = %46, %21
  %29 = load i16, ptr %3, align 2, !tbaa !7
  %30 = icmp slt i16 %29, 64
  br i1 %30, label %31, label %49

31:                                               ; preds = %28
  %32 = load i16, ptr %3, align 2, !tbaa !7
  %33 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %32
  %34 = load i16, ptr %33, align 2, !tbaa !7
  %35 = sext i16 %34 to i32
  %36 = load i32, ptr %5, align 2, !tbaa !11
  %37 = add nsw i32 %36, %35
  store i32 %37, ptr %5, align 2, !tbaa !11
  %38 = load i16, ptr %3, align 2, !tbaa !7
  %39 = getelementptr inbounds [64 x i32], ptr @b, i16 0, i16 %38
  %40 = load i32, ptr %39, align 2, !tbaa !11
  %41 = icmp ne i32 %40, 0
  br i1 %41, label %42, label %45

42:                                               ; preds = %31
  %43 = load i16, ptr %4, align 2, !tbaa !7
  %44 = add nsw i16 %43, 1
  store i16 %44, ptr %4, align 2, !tbaa !7
  br label %45

45:                                               ; preds = %42, %31
  br label %46

46:                                               ; preds = %45
  %47 = load i16, ptr %3, align 2, !tbaa !7
  %48 = add nsw i16 %47, 1
  store i16 %48, ptr %3, align 2, !tbaa !7
  br label %28, !llvm.loop !15

49:                                               ; preds = %28
  %50 = load i16, ptr %2, align 2, !tbaa !7
  %51 = getelementptr inbounds [6 x i16], ptr @main.counts, i16 0, i16 %50
  %52 = load i16, ptr %51, align 2, !tbaa !7
  %53 = sext i16 %52 to i32
  call void @report(i32 noundef %53) #4
  %54 = load i32, ptr %5, align 2, !tbaa !11
  call void @report(i32 noundef %54) #4
  %55 = load i16, ptr %4, align 2, !tbaa !7
  %56 = sext i16 %55 to i32
  call void @report(i32 noundef %56) #4
  %57 = load i32, ptr getelementptr inbounds nuw (i8, ptr @b, i16 252), align 2, !tbaa !11
  call void @report(i32 noundef %57) #4
  br label %58

58:                                               ; preds = %49
  %59 = load i16, ptr %2, align 2, !tbaa !7
  %60 = add nsw i16 %59, 1
  store i16 %60, ptr %2, align 2, !tbaa !7
  br label %6, !llvm.loop !16

61:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
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
!8 = !{!"short", !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = !{!12, !12, i64 0}
!12 = !{!"long", !5, i64 0}
!13 = distinct !{!13, !10}
!14 = distinct !{!14, !10}
!15 = distinct !{!15, !10}
!16 = distinct !{!16, !10}
