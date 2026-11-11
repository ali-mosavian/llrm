target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = call i16 @classify(double noundef 3.300000e+01, i16 noundef 11) #3
  %3 = sext i16 %2 to i32
  call void @report(i32 noundef %3) #3
  %4 = call i16 @classify(double noundef 1.100000e+01, i16 noundef 11) #3
  %5 = sext i16 %4 to i32
  call void @report(i32 noundef %5) #3
  %6 = call i16 @classify(double noundef -5.500000e+00, i16 noundef 11) #3
  %7 = sext i16 %6 to i32
  call void @report(i32 noundef %7) #3
  %8 = call i16 @classify(double noundef 1.200000e+01, i16 noundef -7) #3
  %9 = sext i16 %8 to i32
  call void @report(i32 noundef %9) #3
  %10 = call i16 @classify(double noundef 3.300000e+01, i16 noundef 33) #3
  %11 = sext i16 %10 to i32
  call void @report(i32 noundef %11) #3
  ret i16 0
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

; Function Attrs: nounwind optsize
define internal i16 @classify(double noundef %0, i16 noundef %1) #0 {
  %3 = alloca double, align 2
  %4 = alloca i16, align 2
  %5 = alloca i16, align 2
  store double %0, ptr %3, align 2, !tbaa !7
  store i16 %1, ptr %4, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %5) #4
  store i16 0, ptr %5, align 2, !tbaa !9
  %6 = load double, ptr %3, align 2, !tbaa !7
  %7 = fcmp une double %6, 3.300000e+01
  br i1 %7, label %8, label %11

8:                                                ; preds = %2
  %9 = load i16, ptr %5, align 2, !tbaa !9
  %10 = or i16 %9, 1
  store i16 %10, ptr %5, align 2, !tbaa !9
  br label %11

11:                                               ; preds = %8, %2
  %12 = load double, ptr %3, align 2, !tbaa !7
  %13 = load i16, ptr %4, align 2, !tbaa !9
  %14 = sitofp i16 %13 to double
  %15 = fcmp olt double %12, %14
  br i1 %15, label %16, label %19

16:                                               ; preds = %11
  %17 = load i16, ptr %5, align 2, !tbaa !9
  %18 = or i16 %17, 2
  store i16 %18, ptr %5, align 2, !tbaa !9
  br label %19

19:                                               ; preds = %16, %11
  %20 = load double, ptr %3, align 2, !tbaa !7
  %21 = load i16, ptr %4, align 2, !tbaa !9
  %22 = sitofp i16 %21 to double
  %23 = fcmp ogt double %20, %22
  br i1 %23, label %24, label %27

24:                                               ; preds = %19
  %25 = load i16, ptr %5, align 2, !tbaa !9
  %26 = or i16 %25, 4
  store i16 %26, ptr %5, align 2, !tbaa !9
  br label %27

27:                                               ; preds = %24, %19
  %28 = load i16, ptr %4, align 2, !tbaa !9
  %29 = icmp eq i16 %28, 11
  br i1 %29, label %30, label %33

30:                                               ; preds = %27
  %31 = load i16, ptr %5, align 2, !tbaa !9
  %32 = or i16 %31, 8
  store i16 %32, ptr %5, align 2, !tbaa !9
  br label %33

33:                                               ; preds = %30, %27
  %34 = load double, ptr %3, align 2, !tbaa !7
  %35 = load i16, ptr %4, align 2, !tbaa !9
  %36 = sitofp i16 %35 to double
  %37 = fcmp oeq double %34, %36
  br i1 %37, label %38, label %41

38:                                               ; preds = %33
  %39 = load i16, ptr %5, align 2, !tbaa !9
  %40 = or i16 %39, 16
  store i16 %40, ptr %5, align 2, !tbaa !9
  br label %41

41:                                               ; preds = %38, %33
  %42 = load i16, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.end.p0(ptr %5) #4
  ret i16 %42
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { optsize }
attributes #4 = { nounwind }

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
