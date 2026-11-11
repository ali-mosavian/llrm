source_filename = "bench/nbody/nbody.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @bench_nbody(i16 noundef zeroext %0) local_unnamed_addr #0 {
  %2 = alloca [4 x double], align 8
  %3 = alloca [4 x double], align 8
  %4 = alloca [4 x double], align 8
  %5 = alloca [4 x double], align 8
  call void @llvm.lifetime.start.p0(ptr nonnull %2) #6
  call void @llvm.lifetime.start.p0(ptr nonnull %3) #6
  call void @llvm.lifetime.start.p0(ptr nonnull %4) #6
  call void @llvm.lifetime.start.p0(ptr nonnull %5) #6
  store double -1.000000e+00, ptr %2, align 8, !tbaa !8
  %6 = getelementptr inbounds nuw i8, ptr %2, i32 8
  store double 1.000000e+00, ptr %6, align 8, !tbaa !8
  %7 = getelementptr inbounds nuw i8, ptr %2, i32 16
  %8 = getelementptr inbounds nuw i8, ptr %2, i32 24
  call void @llvm.memset.p0.i64(ptr noundef nonnull align 8 dereferenceable(16) %7, i8 0, i64 16, i1 false)
  %9 = getelementptr inbounds nuw i8, ptr %3, i32 8
  %10 = getelementptr inbounds nuw i8, ptr %3, i32 16
  call void @llvm.memset.p0.i64(ptr noundef nonnull align 8 dereferenceable(16) %3, i8 0, i64 16, i1 false)
  store double -1.000000e+00, ptr %10, align 8, !tbaa !8
  %11 = getelementptr inbounds nuw i8, ptr %3, i32 24
  store double 1.000000e+00, ptr %11, align 8, !tbaa !8
  %12 = getelementptr inbounds nuw i8, ptr %4, i32 8
  %13 = getelementptr inbounds nuw i8, ptr %4, i32 16
  call void @llvm.memset.p0.i64(ptr noundef nonnull align 8 dereferenceable(16) %4, i8 0, i64 16, i1 false)
  store double 1.250000e-02, ptr %13, align 8, !tbaa !8
  %14 = getelementptr inbounds nuw i8, ptr %4, i32 24
  store double -1.250000e-02, ptr %14, align 8, !tbaa !8
  store double -1.250000e-02, ptr %5, align 8, !tbaa !8
  %15 = getelementptr inbounds nuw i8, ptr %5, i32 8
  store double 1.250000e-02, ptr %15, align 8, !tbaa !8
  %16 = getelementptr inbounds nuw i8, ptr %5, i32 16
  %17 = getelementptr inbounds nuw i8, ptr %5, i32 24
  %18 = icmp eq i16 %0, 0
  call void @llvm.memset.p0.i64(ptr noundef nonnull align 8 dereferenceable(16) %16, i8 0, i64 16, i1 false)
  br i1 %18, label %95, label %19

19:                                               ; preds = %1, %70
  %20 = phi double [ %84, %70 ], [ 0.000000e+00, %1 ]
  %21 = phi double [ %80, %70 ], [ 0.000000e+00, %1 ]
  %22 = phi double [ %76, %70 ], [ 1.000000e+00, %1 ]
  %23 = phi double [ %72, %70 ], [ -1.000000e+00, %1 ]
  %24 = phi double [ %86, %70 ], [ 1.000000e+00, %1 ]
  %25 = phi double [ %82, %70 ], [ -1.000000e+00, %1 ]
  %26 = phi double [ %78, %70 ], [ 0.000000e+00, %1 ]
  %27 = phi double [ %74, %70 ], [ 0.000000e+00, %1 ]
  %28 = phi i16 [ %87, %70 ], [ 0, %1 ]
  br label %29

29:                                               ; preds = %19, %66
  %30 = phi i32 [ 0, %19 ], [ %67, %66 ]
  %31 = phi i32 [ 1, %19 ], [ %68, %66 ]
  %32 = icmp samesign ult i32 %30, 3
  br i1 %32, label %33, label %66

33:                                               ; preds = %29
  %34 = getelementptr inbounds nuw [8 x i8], ptr %2, i32 %30
  %35 = load double, ptr %34, align 8, !tbaa !8
  %36 = getelementptr inbounds nuw [8 x i8], ptr %3, i32 %30
  %37 = load double, ptr %36, align 8, !tbaa !8
  %38 = getelementptr inbounds nuw [8 x i8], ptr %4, i32 %30
  %39 = getelementptr inbounds nuw [8 x i8], ptr %5, i32 %30
  br label %40

40:                                               ; preds = %33, %40
  %41 = phi i32 [ %31, %33 ], [ %64, %40 ]
  %42 = getelementptr inbounds nuw [8 x i8], ptr %2, i32 %41
  %43 = load double, ptr %42, align 8, !tbaa !8
  %44 = fsub double %43, %35
  %45 = getelementptr inbounds nuw [8 x i8], ptr %3, i32 %41
  %46 = load double, ptr %45, align 8, !tbaa !8
  %47 = fsub double %46, %37
  %48 = fmul double %47, %47
  %49 = tail call double @llvm.fmuladd.f64(double %44, double %44, double %48)
  %50 = fadd double %49, 1.250000e-01
  %51 = fdiv double 1.000000e-05, %50
  %52 = load double, ptr %38, align 8, !tbaa !8
  %53 = tail call double @llvm.fmuladd.f64(double %44, double %51, double %52)
  store double %53, ptr %38, align 8, !tbaa !8
  %54 = load double, ptr %39, align 8, !tbaa !8
  %55 = tail call double @llvm.fmuladd.f64(double %47, double %51, double %54)
  store double %55, ptr %39, align 8, !tbaa !8
  %56 = getelementptr inbounds nuw [8 x i8], ptr %4, i32 %41
  %57 = load double, ptr %56, align 8, !tbaa !8
  %58 = fneg double %44
  %59 = tail call double @llvm.fmuladd.f64(double %58, double %51, double %57)
  store double %59, ptr %56, align 8, !tbaa !8
  %60 = getelementptr inbounds nuw [8 x i8], ptr %5, i32 %41
  %61 = load double, ptr %60, align 8, !tbaa !8
  %62 = fneg double %47
  %63 = tail call double @llvm.fmuladd.f64(double %62, double %51, double %61)
  store double %63, ptr %60, align 8, !tbaa !8
  %64 = add nuw nsw i32 %41, 1
  %65 = icmp eq i32 %64, 4
  br i1 %65, label %66, label %40, !llvm.loop !10

66:                                               ; preds = %40, %29
  %67 = add nuw nsw i32 %30, 1
  %68 = add nuw nsw i32 %31, 1
  %69 = icmp eq i32 %67, 4
  br i1 %69, label %70, label %29, !llvm.loop !12

70:                                               ; preds = %66
  %71 = load double, ptr %4, align 8, !tbaa !8
  %72 = fadd double %71, %23
  store double %72, ptr %2, align 8, !tbaa !8
  %73 = load double, ptr %5, align 8, !tbaa !8
  %74 = fadd double %73, %27
  store double %74, ptr %3, align 8, !tbaa !8
  %75 = load double, ptr %12, align 8, !tbaa !8
  %76 = fadd double %75, %22
  store double %76, ptr %6, align 8, !tbaa !8
  %77 = load double, ptr %15, align 8, !tbaa !8
  %78 = fadd double %77, %26
  store double %78, ptr %9, align 8, !tbaa !8
  %79 = load double, ptr %13, align 8, !tbaa !8
  %80 = fadd double %79, %21
  store double %80, ptr %7, align 8, !tbaa !8
  %81 = load double, ptr %16, align 8, !tbaa !8
  %82 = fadd double %81, %25
  store double %82, ptr %10, align 8, !tbaa !8
  %83 = load double, ptr %14, align 8, !tbaa !8
  %84 = fadd double %83, %20
  store double %84, ptr %8, align 8, !tbaa !8
  %85 = load double, ptr %17, align 8, !tbaa !8
  %86 = fadd double %85, %24
  store double %86, ptr %11, align 8, !tbaa !8
  %87 = add nuw i16 %28, 1
  %88 = icmp eq i16 %87, %0
  br i1 %88, label %89, label %19, !llvm.loop !13

89:                                               ; preds = %70
  %90 = fadd double %72, %78
  %91 = fadd double %90, %80
  %92 = fadd double %91, %86
  %93 = fmul double %92, 1.000000e+06
  %94 = fptosi double %93 to i32
  br label %95

95:                                               ; preds = %89, %1
  %96 = phi i32 [ %94, %89 ], [ 0, %1 ]
  call void @llvm.lifetime.end.p0(ptr nonnull %5) #6
  call void @llvm.lifetime.end.p0(ptr nonnull %4) #6
  call void @llvm.lifetime.end.p0(ptr nonnull %3) #6
  call void @llvm.lifetime.end.p0(ptr nonnull %2) #6
  ret i32 %96
}

declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare double @llvm.fmuladd.f64(double, double, double) #2

declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

define noundef i32 @main() local_unnamed_addr #3 {
  %1 = tail call i32 @bench_nbody(i16 noundef zeroext 200) #7
  tail call void @report(i32 noundef %1) #8
  ret i32 0
}

declare void @report(i32 noundef) local_unnamed_addr #4

declare void @llvm.memset.p0.i64(ptr writeonly captures(none), i8, i64, i1 immarg) #5

attributes #0 = { nofree norecurse nosync nounwind memory(none) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { mustprogress nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #4 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #5 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }
attributes #6 = { nounwind }
attributes #7 = { nobuiltin "no-builtins" }
attributes #8 = { nobuiltin nounwind "no-builtins" }

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
!9 = !{!"double", !6, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = distinct !{!13, !11}
