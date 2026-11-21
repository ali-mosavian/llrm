target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
; ModuleID = 'unswitch_buys_clocks.c'
source_filename = "unswitch_buys_clocks.c"
target triple = "msp430"

@a = dso_local global [64 x i16] zeroinitializer, align 2

; Function Attrs: nounwind
define dso_local i16 @f(ptr noundef %0, i16 noundef %1, i16 noundef %2) #0 {
  %4 = alloca ptr, align 2
  %5 = alloca i16, align 2
  %6 = alloca i16, align 2
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  store ptr %0, ptr %4, align 2, !tbaa !7
  store i16 %1, ptr %5, align 2, !tbaa !10
  store i16 %2, ptr %6, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %7) #2
  store i16 0, ptr %7, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %8) #2
  store i16 0, ptr %8, align 2, !tbaa !10
  br label %9

9:                                                ; preds = %38, %3
  %10 = load i16, ptr %8, align 2, !tbaa !10
  %11 = load i16, ptr %5, align 2, !tbaa !10
  %12 = icmp slt i16 %10, %11
  br i1 %12, label %14, label %13

13:                                               ; preds = %9
  call void @llvm.lifetime.end.p0(ptr %8) #2
  br label %41

14:                                               ; preds = %9
  %15 = load i16, ptr %6, align 2, !tbaa !10
  %16 = icmp ne i16 %15, 0
  br i1 %16, label %17, label %27

17:                                               ; preds = %14
  %18 = load ptr, ptr %4, align 2, !tbaa !7
  %19 = load i16, ptr %8, align 2, !tbaa !10
  %20 = getelementptr inbounds i16, ptr %18, i16 %19
  %21 = load i16, ptr %20, align 2, !tbaa !10
  %22 = mul nsw i16 %21, 3
  %23 = load i16, ptr %8, align 2, !tbaa !10
  %24 = add nsw i16 %22, %23
  %25 = load i16, ptr %7, align 2, !tbaa !10
  %26 = add nsw i16 %25, %24
  store i16 %26, ptr %7, align 2, !tbaa !10
  br label %37

27:                                               ; preds = %14
  %28 = load ptr, ptr %4, align 2, !tbaa !7
  %29 = load i16, ptr %8, align 2, !tbaa !10
  %30 = getelementptr inbounds i16, ptr %28, i16 %29
  %31 = load i16, ptr %30, align 2, !tbaa !10
  %32 = xor i16 %31, 5
  %33 = load i16, ptr %8, align 2, !tbaa !10
  %34 = sub nsw i16 %32, %33
  %35 = load i16, ptr %7, align 2, !tbaa !10
  %36 = add nsw i16 %35, %34
  store i16 %36, ptr %7, align 2, !tbaa !10
  br label %37

37:                                               ; preds = %27, %17
  br label %38

38:                                               ; preds = %37
  %39 = load i16, ptr %8, align 2, !tbaa !10
  %40 = add nsw i16 %39, 1
  store i16 %40, ptr %8, align 2, !tbaa !10
  br label %9, !llvm.loop !11

41:                                               ; preds = %13
  %42 = load i16, ptr %7, align 2, !tbaa !10
  call void @llvm.lifetime.end.p0(ptr %7) #2
  ret i16 %42
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #2
  store i16 0, ptr %2, align 2, !tbaa !10
  br label %3

3:                                                ; preds = %12, %0
  %4 = load i16, ptr %2, align 2, !tbaa !10
  %5 = icmp slt i16 %4, 64
  br i1 %5, label %7, label %6

6:                                                ; preds = %3
  call void @llvm.lifetime.end.p0(ptr %2) #2
  br label %15

7:                                                ; preds = %3
  %8 = load i16, ptr %2, align 2, !tbaa !10
  %9 = mul nsw i16 %8, 7
  %10 = load i16, ptr %2, align 2, !tbaa !10
  %11 = getelementptr inbounds [64 x i16], ptr @a, i16 0, i16 %10
  store i16 %9, ptr %11, align 2, !tbaa !10
  br label %12

12:                                               ; preds = %7
  %13 = load i16, ptr %2, align 2, !tbaa !10
  %14 = add nsw i16 %13, 1
  store i16 %14, ptr %2, align 2, !tbaa !10
  br label %3, !llvm.loop !13

15:                                               ; preds = %6
  %16 = load i16, ptr getelementptr inbounds nuw (i8, ptr @a, i16 2), align 2, !tbaa !10
  %17 = icmp sgt i16 %16, 3
  %18 = zext i1 %17 to i16
  %19 = call i16 @f(ptr noundef @a, i16 noundef 64, i16 noundef %18)
  %20 = load i16, ptr getelementptr inbounds nuw (i8, ptr @a, i16 4), align 2, !tbaa !10
  %21 = icmp sgt i16 %20, 90
  %22 = zext i1 %21 to i16
  %23 = call i16 @f(ptr noundef @a, i16 noundef 60, i16 noundef %22)
  %24 = add nsw i16 %19, %23
  ret i16 %24
}

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind }

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
!8 = !{!"p1 int", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!4, !4, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
