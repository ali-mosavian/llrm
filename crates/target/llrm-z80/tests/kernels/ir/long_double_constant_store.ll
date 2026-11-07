target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
; ModuleID = 'tests/run/c/long_double_constant_store.c'
source_filename = "tests/run/c/long_double_constant_store.c"
target triple = "msp430"

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @dirty() #4
  %2 = call i32 @show() #4
  call void @report(i32 noundef %2) #4
  call void @dirty() #4
  %3 = call i32 @show() #4
  %4 = add nsw i32 %3, 1
  call void @report(i32 noundef %4) #4
  ret i16 0
}

; Function Attrs: nounwind optsize
define internal void @dirty() #0 {
  %1 = alloca [96 x i8], align 1
  %2 = alloca i16, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #5
  call void @llvm.lifetime.start.p0(ptr %2) #5
  store i16 0, ptr %2, align 2, !tbaa !8
  br label %3

3:                                                ; preds = %9, %0
  %4 = load i16, ptr %2, align 2, !tbaa !8
  %5 = icmp slt i16 %4, 96
  br i1 %5, label %6, label %12

6:                                                ; preds = %3
  %7 = load i16, ptr %2, align 2, !tbaa !8
  %8 = getelementptr inbounds [96 x i8], ptr %1, i16 0, i16 %7
  store volatile i8 -1, ptr %8, align 1, !tbaa !9
  br label %9

9:                                                ; preds = %6
  %10 = load i16, ptr %2, align 2, !tbaa !8
  %11 = add nsw i16 %10, 1
  store i16 %11, ptr %2, align 2, !tbaa !8
  br label %3, !llvm.loop !10

12:                                               ; preds = %3
  call void @llvm.lifetime.end.p0(ptr %2) #5
  call void @llvm.lifetime.end.p0(ptr %1) #5
  ret void
}

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #1

; Function Attrs: nounwind optsize
define internal i32 @show() #0 {
  %1 = alloca double, align 2
  %2 = alloca double, align 2
  %3 = alloca double, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #5
  store double 1.000000e+00, ptr %1, align 2, !tbaa !12
  call void @llvm.lifetime.start.p0(ptr %2) #5
  store double 3.500000e+00, ptr %2, align 2, !tbaa !12
  call void @llvm.lifetime.start.p0(ptr %3) #5
  store double -2.250000e+00, ptr %3, align 2, !tbaa !12
  %4 = load double, ptr %1, align 2, !tbaa !12
  %5 = load double, ptr %2, align 2, !tbaa !12
  %6 = fmul double %5, 1.000000e+01
  %7 = call double @llvm.fmuladd.f64(double %4, double 1.000000e+02, double %6)
  %8 = load double, ptr %3, align 2, !tbaa !12
  %9 = call double @llvm.fmuladd.f64(double %8, double 4.000000e+00, double %7)
  %10 = fptosi double %9 to i32
  call void @llvm.lifetime.end.p0(ptr %3) #5
  call void @llvm.lifetime.end.p0(ptr %2) #5
  call void @llvm.lifetime.end.p0(ptr %1) #5
  ret i32 %10
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.fmuladd.f64(double, double, double) #3

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #2 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #3 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #4 = { optsize }
attributes #5 = { nounwind }

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{i32 1, !"long-double-type", !"double"}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!5, !5, i64 0}
!9 = !{!6, !6, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = !{!13, !13, i64 0}
!13 = !{!"long double", !6, i64 0}
