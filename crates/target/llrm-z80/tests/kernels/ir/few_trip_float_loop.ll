target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca i16, align 2
  %3 = alloca double, align 2
  %4 = alloca double, align 2
  %5 = alloca double, align 2
  %6 = alloca double, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  store double 0.000000e+00, ptr %4, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store double 1.000000e+00, ptr %5, align 2, !tbaa !7
  store i16 1, ptr %2, align 2, !tbaa !9
  br label %7

7:                                                ; preds = %28, %0
  %8 = load i16, ptr %2, align 2, !tbaa !9
  %9 = icmp sle i16 %8, 2
  br i1 %9, label %10, label %31

10:                                               ; preds = %7
  call void @llvm.lifetime.start.p0(ptr %6) #3
  %11 = load double, ptr %5, align 2, !tbaa !7
  %12 = load double, ptr %5, align 2, !tbaa !7
  %13 = fmul double %11, %12
  store double %13, ptr %6, align 2, !tbaa !7
  %14 = load i16, ptr %2, align 2, !tbaa !9
  %15 = icmp ne i16 %14, 1
  br i1 %15, label %16, label %20

16:                                               ; preds = %10
  %17 = load double, ptr %6, align 2, !tbaa !7
  %18 = load double, ptr %3, align 2, !tbaa !7
  %19 = fdiv double %17, %18
  store double %19, ptr %4, align 2, !tbaa !7
  br label %20

20:                                               ; preds = %16, %10
  %21 = load double, ptr %4, align 2, !tbaa !7
  %22 = fcmp oeq double %21, 1.000000e+00
  br i1 %22, label %23, label %24

23:                                               ; preds = %20
  call void @report(i32 noundef -1) #4
  br label %24

24:                                               ; preds = %23, %20
  %25 = load double, ptr %5, align 2, !tbaa !7
  %26 = fdiv double %25, 2.000000e+00
  store double %26, ptr %5, align 2, !tbaa !7
  %27 = load double, ptr %6, align 2, !tbaa !7
  store double %27, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.end.p0(ptr %6) #3
  br label %28

28:                                               ; preds = %24
  %29 = load i16, ptr %2, align 2, !tbaa !9
  %30 = add nsw i16 %29, 1
  store i16 %30, ptr %2, align 2, !tbaa !9
  br label %7, !llvm.loop !10

31:                                               ; preds = %7
  call void @report(i32 noundef 7) #4
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #2

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
!7 = !{!8, !8, i64 0}
!8 = !{!"double", !5, i64 0}
!9 = !{!4, !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
