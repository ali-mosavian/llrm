target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@trips = internal constant i16 7, align 2

define i32 @rot(i16 noundef signext %0, float noundef %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" memory(none) nofree nosync norecurse willreturn {
  %3 = fadd float %1, 1.000000e+00
  %4 = fadd float %1, 2.000000e+00
  %5 = fadd float %1, 3.000000e+00
  %6 = fadd float %1, 4.000000e+00
  %7 = fadd float %1, 5.000000e+00
  %8 = fadd float %1, 6.000000e+00
  %9 = fadd float %1, 7.000000e+00
  %10 = fadd float %1, 8.000000e+00
  %11 = fadd float %1, 9.000000e+00
  %12 = icmp sgt i16 %0, 0
  br i1 %12, label %13, label %28

13:
  br label %14

14:
  %15 = phi i16 [ %0, %13 ], [ %27, %14 ]
  %16 = phi float [ %25, %14 ], [ %11, %13 ]
  %17 = phi float [ %16, %14 ], [ %10, %13 ]
  %18 = phi float [ %17, %14 ], [ %9, %13 ]
  %19 = phi float [ %18, %14 ], [ %8, %13 ]
  %20 = phi float [ %19, %14 ], [ %7, %13 ]
  %21 = phi float [ %20, %14 ], [ %6, %13 ]
  %22 = phi float [ %21, %14 ], [ %5, %13 ]
  %23 = phi float [ %22, %14 ], [ %4, %13 ]
  %24 = phi float [ %23, %14 ], [ %3, %13 ]
  %25 = phi float [ %26, %14 ], [ %1, %13 ]
  %26 = tail call float @llvm.fmuladd.f32(float %25, float 5.000000e-01, float %24)
  %27 = add i16 %15, -1
  %exitcond = icmp eq i16 %27, 0
  br i1 %exitcond, label %28, label %14, !llvm.loop !12

28:
  %29 = phi float [ %11, %2 ], [ %25, %14 ]
  %30 = phi float [ %10, %2 ], [ %16, %14 ]
  %31 = phi float [ %9, %2 ], [ %17, %14 ]
  %32 = phi float [ %8, %2 ], [ %18, %14 ]
  %33 = phi float [ %7, %2 ], [ %19, %14 ]
  %34 = phi float [ %6, %2 ], [ %20, %14 ]
  %35 = phi float [ %5, %2 ], [ %21, %14 ]
  %36 = phi float [ %4, %2 ], [ %22, %14 ]
  %37 = phi float [ %3, %2 ], [ %23, %14 ]
  %38 = phi float [ %1, %2 ], [ %26, %14 ]
  %39 = tail call float @llvm.fmuladd.f32(float 2.000000e+00, float %37, float %38)
  %40 = tail call float @llvm.fmuladd.f32(float 3.000000e+00, float %36, float %39)
  %41 = tail call float @llvm.fmuladd.f32(float 4.000000e+00, float %35, float %40)
  %42 = tail call float @llvm.fmuladd.f32(float 5.000000e+00, float %34, float %41)
  %43 = tail call float @llvm.fmuladd.f32(float 6.000000e+00, float %33, float %42)
  %44 = tail call float @llvm.fmuladd.f32(float 7.000000e+00, float %32, float %43)
  %45 = tail call float @llvm.fmuladd.f32(float 8.000000e+00, float %31, float %44)
  %46 = tail call float @llvm.fmuladd.f32(float 9.000000e+00, float %30, float %45)
  %47 = tail call float @llvm.fmuladd.f32(float 1.000000e+01, float %29, float %46)
  %48 = fptosi float %47 to i32
  ret i32 %48
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare float @llvm.fmuladd.f32(float, float, float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" {
  %1 = load volatile i16, ptr @trips, align 2, !tbaa !8
  %2 = icmp sgt i16 %1, 0
  br i1 %2, label %3, label %29

3:
  br label %4

4:
  %5 = phi i16 [ %1, %3 ], [ %17, %4 ]
  %6 = phi float [ %15, %4 ], [ 1.000000e+01, %3 ]
  %7 = phi float [ %6, %4 ], [ 9.000000e+00, %3 ]
  %8 = phi float [ %7, %4 ], [ 8.000000e+00, %3 ]
  %9 = phi float [ %8, %4 ], [ 7.000000e+00, %3 ]
  %10 = phi float [ %9, %4 ], [ 6.000000e+00, %3 ]
  %11 = phi float [ %10, %4 ], [ 5.000000e+00, %3 ]
  %12 = phi float [ %11, %4 ], [ 4.000000e+00, %3 ]
  %13 = phi float [ %12, %4 ], [ 3.000000e+00, %3 ]
  %14 = phi float [ %13, %4 ], [ 2.000000e+00, %3 ]
  %15 = phi float [ %16, %4 ], [ 1.000000e+00, %3 ]
  %16 = tail call float @llvm.fmuladd.f32(float %15, float 5.000000e-01, float %14)
  %17 = add i16 %5, -1
  %exitcond = icmp eq i16 %17, 0
  br i1 %exitcond, label %18, label %4, !llvm.loop !12

18:
  %19 = tail call float @llvm.fmuladd.f32(float 2.000000e+00, float %13, float %16)
  %20 = tail call float @llvm.fmuladd.f32(float 3.000000e+00, float %12, float %19)
  %21 = tail call float @llvm.fmuladd.f32(float 4.000000e+00, float %11, float %20)
  %22 = tail call float @llvm.fmuladd.f32(float 5.000000e+00, float %10, float %21)
  %23 = tail call float @llvm.fmuladd.f32(float 6.000000e+00, float %9, float %22)
  %24 = tail call float @llvm.fmuladd.f32(float 7.000000e+00, float %8, float %23)
  %25 = tail call float @llvm.fmuladd.f32(float 8.000000e+00, float %7, float %24)
  %26 = tail call float @llvm.fmuladd.f32(float 9.000000e+00, float %6, float %25)
  %27 = tail call float @llvm.fmuladd.f32(float 1.000000e+01, float %15, float %26)
  %28 = fptosi float %27 to i32
  br label %29

29:
  %30 = phi i32 [ 385, %0 ], [ %28, %18 ]
  tail call void @report(i32 noundef %30) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000"

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!9, !9, i64 0}
!9 = !{!"short", !6, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"float", !6, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
