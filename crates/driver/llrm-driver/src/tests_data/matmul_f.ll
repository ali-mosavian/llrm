target datalayout = "@m32-datalayout@"

@A = internal global [12 x [12 x double]] zeroinitializer, align 8
@B = internal global [12 x [12 x double]] zeroinitializer, align 8
@C = internal global [12 x [12 x double]] zeroinitializer, align 8
@bench_x_matmul_f_via = dso_local global ptr @bench_x_matmul_f, align 4

; Function Attrs: nounwind uwtable
define dso_local i32 @bench_x_matmul_f(i32 noundef %0) #0 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca double, align 8
  %7 = alloca double, align 8
  store i32 %0, ptr %2, align 4, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  call void @llvm.lifetime.start.p0(ptr %5) #4
  call void @llvm.lifetime.start.p0(ptr %6) #4
  store double 0.000000e+00, ptr %6, align 8, !tbaa !11
  store i32 0, ptr %3, align 4, !tbaa !10
  br label %8

8:                                                ; preds = %40, %1
  %9 = load i32, ptr %3, align 4, !tbaa !10
  %10 = load i32, ptr %2, align 4, !tbaa !10
  %11 = icmp slt i32 %9, %10
  br i1 %11, label %12, label %43

12:                                               ; preds = %8
  store i32 0, ptr %4, align 4, !tbaa !10
  br label %13

13:                                               ; preds = %36, %12
  %14 = load i32, ptr %4, align 4, !tbaa !10
  %15 = load i32, ptr %2, align 4, !tbaa !10
  %16 = icmp slt i32 %14, %15
  br i1 %16, label %17, label %39

17:                                               ; preds = %13
  %18 = load i32, ptr %3, align 4, !tbaa !10
  %19 = load i32, ptr %4, align 4, !tbaa !10
  %20 = add nsw i32 %18, %19
  %21 = sitofp i32 %20 to double
  %22 = fmul double %21, 5.000000e-01
  %23 = load i32, ptr %3, align 4, !tbaa !10
  %24 = getelementptr inbounds [12 x [12 x double]], ptr @A, i32 0, i32 %23
  %25 = load i32, ptr %4, align 4, !tbaa !10
  %26 = getelementptr inbounds [12 x double], ptr %24, i32 0, i32 %25
  store double %22, ptr %26, align 8, !tbaa !11
  %27 = load i32, ptr %3, align 4, !tbaa !10
  %28 = load i32, ptr %4, align 4, !tbaa !10
  %29 = sub nsw i32 %27, %28
  %30 = sitofp i32 %29 to double
  %31 = call double @llvm.fmuladd.f64(double %30, double 2.500000e-01, double 1.000000e+00)
  %32 = load i32, ptr %3, align 4, !tbaa !10
  %33 = getelementptr inbounds [12 x [12 x double]], ptr @B, i32 0, i32 %32
  %34 = load i32, ptr %4, align 4, !tbaa !10
  %35 = getelementptr inbounds [12 x double], ptr %33, i32 0, i32 %34
  store double %31, ptr %35, align 8, !tbaa !11
  br label %36

36:                                               ; preds = %17
  %37 = load i32, ptr %4, align 4, !tbaa !10
  %38 = add nsw i32 %37, 1
  store i32 %38, ptr %4, align 4, !tbaa !10
  br label %13, !llvm.loop !13

39:                                               ; preds = %13
  br label %40

40:                                               ; preds = %39
  %41 = load i32, ptr %3, align 4, !tbaa !10
  %42 = add nsw i32 %41, 1
  store i32 %42, ptr %3, align 4, !tbaa !10
  br label %8, !llvm.loop !15

43:                                               ; preds = %8
  store i32 0, ptr %3, align 4, !tbaa !10
  br label %44

44:                                               ; preds = %87, %43
  %45 = load i32, ptr %3, align 4, !tbaa !10
  %46 = load i32, ptr %2, align 4, !tbaa !10
  %47 = icmp slt i32 %45, %46
  br i1 %47, label %48, label %90

48:                                               ; preds = %44
  store i32 0, ptr %4, align 4, !tbaa !10
  br label %49

49:                                               ; preds = %83, %48
  %50 = load i32, ptr %4, align 4, !tbaa !10
  %51 = load i32, ptr %2, align 4, !tbaa !10
  %52 = icmp slt i32 %50, %51
  br i1 %52, label %53, label %86

53:                                               ; preds = %49
  call void @llvm.lifetime.start.p0(ptr %7) #4
  store double 0.000000e+00, ptr %7, align 8, !tbaa !11
  store i32 0, ptr %5, align 4, !tbaa !10
  br label %54

54:                                               ; preds = %71, %53
  %55 = load i32, ptr %5, align 4, !tbaa !10
  %56 = load i32, ptr %2, align 4, !tbaa !10
  %57 = icmp slt i32 %55, %56
  br i1 %57, label %58, label %74

58:                                               ; preds = %54
  %59 = load i32, ptr %3, align 4, !tbaa !10
  %60 = getelementptr inbounds [12 x [12 x double]], ptr @A, i32 0, i32 %59
  %61 = load i32, ptr %5, align 4, !tbaa !10
  %62 = getelementptr inbounds [12 x double], ptr %60, i32 0, i32 %61
  %63 = load double, ptr %62, align 8, !tbaa !11
  %64 = load i32, ptr %5, align 4, !tbaa !10
  %65 = getelementptr inbounds [12 x [12 x double]], ptr @B, i32 0, i32 %64
  %66 = load i32, ptr %4, align 4, !tbaa !10
  %67 = getelementptr inbounds [12 x double], ptr %65, i32 0, i32 %66
  %68 = load double, ptr %67, align 8, !tbaa !11
  %69 = load double, ptr %7, align 8, !tbaa !11
  %70 = call double @llvm.fmuladd.f64(double %63, double %68, double %69)
  store double %70, ptr %7, align 8, !tbaa !11
  br label %71

71:                                               ; preds = %58
  %72 = load i32, ptr %5, align 4, !tbaa !10
  %73 = add nsw i32 %72, 1
  store i32 %73, ptr %5, align 4, !tbaa !10
  br label %54, !llvm.loop !16

74:                                               ; preds = %54
  %75 = load double, ptr %7, align 8, !tbaa !11
  %76 = load i32, ptr %3, align 4, !tbaa !10
  %77 = getelementptr inbounds [12 x [12 x double]], ptr @C, i32 0, i32 %76
  %78 = load i32, ptr %4, align 4, !tbaa !10
  %79 = getelementptr inbounds [12 x double], ptr %77, i32 0, i32 %78
  store double %75, ptr %79, align 8, !tbaa !11
  %80 = load double, ptr %7, align 8, !tbaa !11
  %81 = load double, ptr %6, align 8, !tbaa !11
  %82 = fadd double %81, %80
  store double %82, ptr %6, align 8, !tbaa !11
  call void @llvm.lifetime.end.p0(ptr %7) #4
  br label %83

83:                                               ; preds = %74
  %84 = load i32, ptr %4, align 4, !tbaa !10
  %85 = add nsw i32 %84, 1
  store i32 %85, ptr %4, align 4, !tbaa !10
  br label %49, !llvm.loop !17

86:                                               ; preds = %49
  br label %87

87:                                               ; preds = %86
  %88 = load i32, ptr %3, align 4, !tbaa !10
  %89 = add nsw i32 %88, 1
  store i32 %89, ptr %3, align 4, !tbaa !10
  br label %44, !llvm.loop !18

90:                                               ; preds = %44
  %91 = load double, ptr %6, align 8, !tbaa !11
  %92 = fptosi double %91 to i32
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  ret i32 %92
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.fmuladd.f64(double, double, double) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = load volatile ptr, ptr @bench_x_matmul_f_via, align 4, !tbaa !19
  %3 = call i32 %2(i32 noundef 12)
  call void @report(i32 noundef %3)
  ret i32 0
}

declare void @report(i32 noundef) #3

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #4 = { nounwind }

!llvm.module.flags = !{!0, !1, !2, !3}
!llvm.ident = !{!4}
!llvm.errno.tbaa = !{!5}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!5 = !{!6, !7, i64 0}
!6 = !{!"__libc_errno", !7, i64 0}
!7 = !{!"int", !8, i64 0}
!8 = !{!"omnipotent char", !9, i64 0}
!9 = !{!"Simple C/C++ TBAA"}
!10 = !{!7, !7, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"double", !8, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = distinct !{!15, !14}
!16 = distinct !{!16, !14}
!17 = distinct !{!17, !14}
!18 = distinct !{!18, !14}
!19 = !{!20, !20, i64 0}
!20 = !{!"any pointer", !8, i64 0}
