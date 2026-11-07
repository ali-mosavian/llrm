target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
target triple = "msp430"

; Function Attrs: noinline nounwind optsize
define hidden void @lc1(ptr noundef %0, ptr noundef %1) #0 {
  %3 = alloca ptr, align 2
  %4 = alloca ptr, align 2
  %5 = alloca [700 x i8], align 1
  %6 = alloca [700 x i8], align 1
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  store ptr %0, ptr %3, align 2, !tbaa !7
  store ptr %1, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  call void @llvm.lifetime.start.p0(ptr %6) #3
  call void @llvm.lifetime.start.p0(ptr %7) #3
  store i16 0, ptr %7, align 2, !tbaa !10
  br label %9

9:                                                ; preds = %22, %2
  %10 = load i16, ptr %7, align 2, !tbaa !10
  %11 = icmp slt i16 %10, 700
  br i1 %11, label %13, label %12

12:                                               ; preds = %9
  call void @llvm.lifetime.end.p0(ptr %7) #3
  br label %25

13:                                               ; preds = %9
  %14 = load ptr, ptr %4, align 2, !tbaa !7
  %15 = load i16, ptr %7, align 2, !tbaa !10
  %16 = getelementptr inbounds i8, ptr %14, i16 %15
  %17 = load i8, ptr %16, align 1, !tbaa !11
  %18 = load i16, ptr %7, align 2, !tbaa !10
  %19 = getelementptr inbounds [700 x i8], ptr %5, i16 0, i16 %18
  store i8 %17, ptr %19, align 1, !tbaa !11
  %20 = load i16, ptr %7, align 2, !tbaa !10
  %21 = getelementptr inbounds [700 x i8], ptr %6, i16 0, i16 %20
  store i8 -18, ptr %21, align 1, !tbaa !11
  br label %22

22:                                               ; preds = %13
  %23 = load i16, ptr %7, align 2, !tbaa !10
  %24 = add nsw i16 %23, 1
  store i16 %24, ptr %7, align 2, !tbaa !10
  br label %9, !llvm.loop !12

25:                                               ; preds = %12
  %26 = getelementptr inbounds [700 x i8], ptr %6, i16 0, i16 0
  %27 = getelementptr inbounds i8, ptr %26, i16 3
  %28 = getelementptr inbounds [700 x i8], ptr %5, i16 0, i16 0
  %29 = getelementptr inbounds i8, ptr %28, i16 1
  call void @llvm.memcpy.p0.p0.i16(ptr align 1 %27, ptr align 1 %29, i16 1, i1 false)
  %30 = getelementptr inbounds [700 x i8], ptr %5, i16 0, i16 0
  %31 = getelementptr inbounds i8, ptr %30, i16 5
  %32 = getelementptr inbounds [700 x i8], ptr %5, i16 0, i16 0
  %33 = getelementptr inbounds i8, ptr %32, i16 2
  call void @llvm.memmove.p0.p0.i16(ptr align 1 %31, ptr align 1 %33, i16 1, i1 false)
  %34 = getelementptr inbounds [700 x i8], ptr %6, i16 0, i16 0
  %35 = getelementptr inbounds i8, ptr %34, i16 650
  call void @llvm.memset.p0.i16(ptr align 1 %35, i8 51, i16 1, i1 false)
  call void @llvm.lifetime.start.p0(ptr %8) #3
  store i16 0, ptr %8, align 2, !tbaa !10
  br label %36

36:                                               ; preds = %54, %25
  %37 = load i16, ptr %8, align 2, !tbaa !10
  %38 = icmp slt i16 %37, 700
  br i1 %38, label %40, label %39

39:                                               ; preds = %36
  call void @llvm.lifetime.end.p0(ptr %8) #3
  br label %57

40:                                               ; preds = %36
  %41 = load i16, ptr %8, align 2, !tbaa !10
  %42 = getelementptr inbounds [700 x i8], ptr %5, i16 0, i16 %41
  %43 = load i8, ptr %42, align 1, !tbaa !11
  %44 = zext i8 %43 to i16
  %45 = load i16, ptr %8, align 2, !tbaa !10
  %46 = getelementptr inbounds [700 x i8], ptr %6, i16 0, i16 %45
  %47 = load i8, ptr %46, align 1, !tbaa !11
  %48 = zext i8 %47 to i16
  %49 = xor i16 %44, %48
  %50 = trunc i16 %49 to i8
  %51 = load ptr, ptr %3, align 2, !tbaa !7
  %52 = load i16, ptr %8, align 2, !tbaa !10
  %53 = getelementptr inbounds i8, ptr %51, i16 %52
  store i8 %50, ptr %53, align 1, !tbaa !11
  br label %54

54:                                               ; preds = %40
  %55 = load i16, ptr %8, align 2, !tbaa !10
  %56 = add nsw i16 %55, 1
  store i16 %56, ptr %8, align 2, !tbaa !10
  br label %36, !llvm.loop !14

57:                                               ; preds = %39
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i16(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i16, i1 immarg) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i16(ptr writeonly captures(none), i8, i16, i1 immarg) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memmove.p0.p0.i16(ptr writeonly captures(none), ptr readonly captures(none), i16, i1 immarg) #1

attributes #0 = { noinline nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
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
!8 = !{!"p1 omnipotent char", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!4, !4, i64 0}
!11 = !{!5, !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
