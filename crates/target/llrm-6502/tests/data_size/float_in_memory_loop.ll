target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@acc = internal global double 0.000000e+00, align 2
@step = internal global double 0.000000e+00, align 2

; Function Attrs: nounwind optsize
define dso_local i32 @sum_of_steps(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i32, align 2
  store i16 %0, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !8
  store i16 0, ptr %4, align 2, !tbaa !7
  br label %6

6:                                                ; preds = %29, %1
  %7 = load i16, ptr %4, align 2, !tbaa !7
  %8 = load i16, ptr %2, align 2, !tbaa !7
  %9 = icmp slt i16 %7, %8
  br i1 %9, label %10, label %32

10:                                               ; preds = %6
  store double 0.000000e+00, ptr @acc, align 2, !tbaa !10
  store double 5.000000e-01, ptr @step, align 2, !tbaa !10
  store i16 0, ptr %3, align 2, !tbaa !7
  br label %11

11:                                               ; preds = %18, %10
  %12 = load i16, ptr %3, align 2, !tbaa !7
  %13 = icmp slt i16 %12, 16
  br i1 %13, label %14, label %21

14:                                               ; preds = %11
  %15 = load double, ptr @step, align 2, !tbaa !10
  %16 = load double, ptr @acc, align 2, !tbaa !10
  %17 = fadd double %16, %15
  store double %17, ptr @acc, align 2, !tbaa !10
  br label %18

18:                                               ; preds = %14
  %19 = load i16, ptr %3, align 2, !tbaa !7
  %20 = add nsw i16 %19, 1
  store i16 %20, ptr %3, align 2, !tbaa !7
  br label %11, !llvm.loop !12

21:                                               ; preds = %11
  %22 = load double, ptr @acc, align 2, !tbaa !10
  %23 = fptosi double %22 to i32
  %24 = load i16, ptr %4, align 2, !tbaa !7
  %25 = sext i16 %24 to i32
  %26 = add nsw i32 %23, %25
  %27 = load i32, ptr %5, align 2, !tbaa !8
  %28 = add nsw i32 %27, %26
  store i32 %28, ptr %5, align 2, !tbaa !8
  br label %29

29:                                               ; preds = %21
  %30 = load i16, ptr %4, align 2, !tbaa !7
  %31 = add nsw i16 %30, 1
  store i16 %31, ptr %4, align 2, !tbaa !7
  br label %6, !llvm.loop !14

32:                                               ; preds = %6
  %33 = load i32, ptr %5, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %33
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i32 @sum_of_steps(i16 noundef 100) #4
  call void @report(i32 noundef %2) #4
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
!7 = !{!4, !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"long", !5, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"double", !5, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
