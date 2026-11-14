; ModuleID = 'tests/run/c/x87rotate.c'
source_filename = "tests/run/c/x87rotate.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

@trips = internal global i16 7, align 2

; Function Attrs: nounwind
define dso_local i32 @rot(i16 noundef signext %0, float noundef %1) #0 {
  %3 = alloca i16, align 2
  %4 = alloca float, align 2
  %5 = alloca float, align 2
  %6 = alloca float, align 2
  %7 = alloca float, align 2
  %8 = alloca float, align 2
  %9 = alloca float, align 2
  %10 = alloca float, align 2
  %11 = alloca float, align 2
  %12 = alloca float, align 2
  %13 = alloca float, align 2
  %14 = alloca float, align 2
  %15 = alloca float, align 2
  %16 = alloca i16, align 2
  store i16 %0, ptr %3, align 2, !tbaa !7
  store float %1, ptr %4, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %5) #4
  %17 = load float, ptr %4, align 2, !tbaa !9
  store float %17, ptr %5, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %6) #4
  %18 = load float, ptr %4, align 2, !tbaa !9
  %19 = fadd float %18, 1.000000e+00
  store float %19, ptr %6, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %7) #4
  %20 = load float, ptr %4, align 2, !tbaa !9
  %21 = fadd float %20, 2.000000e+00
  store float %21, ptr %7, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %8) #4
  %22 = load float, ptr %4, align 2, !tbaa !9
  %23 = fadd float %22, 3.000000e+00
  store float %23, ptr %8, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %9) #4
  %24 = load float, ptr %4, align 2, !tbaa !9
  %25 = fadd float %24, 4.000000e+00
  store float %25, ptr %9, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %10) #4
  %26 = load float, ptr %4, align 2, !tbaa !9
  %27 = fadd float %26, 5.000000e+00
  store float %27, ptr %10, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %11) #4
  %28 = load float, ptr %4, align 2, !tbaa !9
  %29 = fadd float %28, 6.000000e+00
  store float %29, ptr %11, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %12) #4
  %30 = load float, ptr %4, align 2, !tbaa !9
  %31 = fadd float %30, 7.000000e+00
  store float %31, ptr %12, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %13) #4
  %32 = load float, ptr %4, align 2, !tbaa !9
  %33 = fadd float %32, 8.000000e+00
  store float %33, ptr %13, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %14) #4
  %34 = load float, ptr %4, align 2, !tbaa !9
  %35 = fadd float %34, 9.000000e+00
  store float %35, ptr %14, align 2, !tbaa !9
  call void @llvm.lifetime.start.p0(ptr %15) #4
  call void @llvm.lifetime.start.p0(ptr %16) #4
  store i16 0, ptr %16, align 2, !tbaa !7
  br label %36

36:                                               ; preds = %55, %2
  %37 = load i16, ptr %16, align 2, !tbaa !7
  %38 = load i16, ptr %3, align 2, !tbaa !7
  %39 = icmp slt i16 %37, %38
  br i1 %39, label %40, label %58

40:                                               ; preds = %36
  %41 = load float, ptr %5, align 2, !tbaa !9
  store float %41, ptr %15, align 2, !tbaa !9
  %42 = load float, ptr %6, align 2, !tbaa !9
  store float %42, ptr %5, align 2, !tbaa !9
  %43 = load float, ptr %7, align 2, !tbaa !9
  store float %43, ptr %6, align 2, !tbaa !9
  %44 = load float, ptr %8, align 2, !tbaa !9
  store float %44, ptr %7, align 2, !tbaa !9
  %45 = load float, ptr %9, align 2, !tbaa !9
  store float %45, ptr %8, align 2, !tbaa !9
  %46 = load float, ptr %10, align 2, !tbaa !9
  store float %46, ptr %9, align 2, !tbaa !9
  %47 = load float, ptr %11, align 2, !tbaa !9
  store float %47, ptr %10, align 2, !tbaa !9
  %48 = load float, ptr %12, align 2, !tbaa !9
  store float %48, ptr %11, align 2, !tbaa !9
  %49 = load float, ptr %13, align 2, !tbaa !9
  store float %49, ptr %12, align 2, !tbaa !9
  %50 = load float, ptr %14, align 2, !tbaa !9
  store float %50, ptr %13, align 2, !tbaa !9
  %51 = load float, ptr %15, align 2, !tbaa !9
  store float %51, ptr %14, align 2, !tbaa !9
  %52 = load float, ptr %5, align 2, !tbaa !9
  %53 = load float, ptr %14, align 2, !tbaa !9
  %54 = call float @llvm.fmuladd.f32(float %53, float 5.000000e-01, float %52)
  store float %54, ptr %5, align 2, !tbaa !9
  br label %55

55:                                               ; preds = %40
  %56 = load i16, ptr %16, align 2, !tbaa !7
  %57 = add nsw i16 %56, 1
  store i16 %57, ptr %16, align 2, !tbaa !7
  br label %36, !llvm.loop !11

58:                                               ; preds = %36
  %59 = load float, ptr %5, align 2, !tbaa !9
  %60 = load float, ptr %6, align 2, !tbaa !9
  %61 = call float @llvm.fmuladd.f32(float 2.000000e+00, float %60, float %59)
  %62 = load float, ptr %7, align 2, !tbaa !9
  %63 = call float @llvm.fmuladd.f32(float 3.000000e+00, float %62, float %61)
  %64 = load float, ptr %8, align 2, !tbaa !9
  %65 = call float @llvm.fmuladd.f32(float 4.000000e+00, float %64, float %63)
  %66 = load float, ptr %9, align 2, !tbaa !9
  %67 = call float @llvm.fmuladd.f32(float 5.000000e+00, float %66, float %65)
  %68 = load float, ptr %10, align 2, !tbaa !9
  %69 = call float @llvm.fmuladd.f32(float 6.000000e+00, float %68, float %67)
  %70 = load float, ptr %11, align 2, !tbaa !9
  %71 = call float @llvm.fmuladd.f32(float 7.000000e+00, float %70, float %69)
  %72 = load float, ptr %12, align 2, !tbaa !9
  %73 = call float @llvm.fmuladd.f32(float 8.000000e+00, float %72, float %71)
  %74 = load float, ptr %13, align 2, !tbaa !9
  %75 = call float @llvm.fmuladd.f32(float 9.000000e+00, float %74, float %73)
  %76 = load float, ptr %14, align 2, !tbaa !9
  %77 = call float @llvm.fmuladd.f32(float 1.000000e+01, float %76, float %75)
  %78 = fptosi float %77 to i32
  call void @llvm.lifetime.end.p0(ptr %16) #4
  call void @llvm.lifetime.end.p0(ptr %15) #4
  call void @llvm.lifetime.end.p0(ptr %14) #4
  call void @llvm.lifetime.end.p0(ptr %13) #4
  call void @llvm.lifetime.end.p0(ptr %12) #4
  call void @llvm.lifetime.end.p0(ptr %11) #4
  call void @llvm.lifetime.end.p0(ptr %10) #4
  call void @llvm.lifetime.end.p0(ptr %9) #4
  call void @llvm.lifetime.end.p0(ptr %8) #4
  call void @llvm.lifetime.end.p0(ptr %7) #4
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  ret i32 %78
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare float @llvm.fmuladd.f32(float, float, float) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  %2 = load volatile i16, ptr @trips, align 2, !tbaa !7
  %3 = call i32 @rot(i16 noundef signext %2, float noundef 1.000000e+00)
  call void @report(i32 noundef %3)
  ret i16 0
}

declare dso_local void @report(i32 noundef) #3

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
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
!8 = !{!"short", !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"float", !5, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
