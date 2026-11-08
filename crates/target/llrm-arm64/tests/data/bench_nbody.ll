; ModuleID = '/home/alim/scratch/w15-bar/out/src/nbody.c'
source_filename = "/home/alim/scratch/w15-bar/out/src/nbody.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@opq0 = internal global i32 0, align 4

; Function Attrs: noinline nounwind uwtable
define dso_local i64 @bench_nbody(i16 noundef %0) #0 {
  %2 = alloca i16, align 2
  %3 = alloca [4 x double], align 8
  %4 = alloca [4 x double], align 8
  %5 = alloca [4 x double], align 8
  %6 = alloca [4 x double], align 8
  %7 = alloca i16, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca double, align 8
  %11 = alloca double, align 8
  %12 = alloca double, align 8
  store i16 %0, ptr %2, align 2
  %13 = getelementptr inbounds [4 x double], ptr %3, i64 0, i64 0
  store double -1.000000e+00, ptr %13, align 8
  %14 = getelementptr inbounds [4 x double], ptr %3, i64 0, i64 1
  store double 1.000000e+00, ptr %14, align 8
  %15 = getelementptr inbounds [4 x double], ptr %3, i64 0, i64 2
  store double 0.000000e+00, ptr %15, align 8
  %16 = getelementptr inbounds [4 x double], ptr %3, i64 0, i64 3
  store double 0.000000e+00, ptr %16, align 8
  %17 = getelementptr inbounds [4 x double], ptr %4, i64 0, i64 0
  store double 0.000000e+00, ptr %17, align 8
  %18 = getelementptr inbounds [4 x double], ptr %4, i64 0, i64 1
  store double 0.000000e+00, ptr %18, align 8
  %19 = getelementptr inbounds [4 x double], ptr %4, i64 0, i64 2
  store double -1.000000e+00, ptr %19, align 8
  %20 = getelementptr inbounds [4 x double], ptr %4, i64 0, i64 3
  store double 1.000000e+00, ptr %20, align 8
  %21 = getelementptr inbounds [4 x double], ptr %5, i64 0, i64 0
  store double 0.000000e+00, ptr %21, align 8
  %22 = getelementptr inbounds [4 x double], ptr %5, i64 0, i64 1
  store double 0.000000e+00, ptr %22, align 8
  %23 = getelementptr inbounds [4 x double], ptr %5, i64 0, i64 2
  store double 1.250000e-02, ptr %23, align 8
  %24 = getelementptr inbounds [4 x double], ptr %5, i64 0, i64 3
  store double -1.250000e-02, ptr %24, align 8
  %25 = getelementptr inbounds [4 x double], ptr %6, i64 0, i64 0
  store double -1.250000e-02, ptr %25, align 8
  %26 = getelementptr inbounds [4 x double], ptr %6, i64 0, i64 1
  store double 1.250000e-02, ptr %26, align 8
  %27 = getelementptr inbounds [4 x double], ptr %6, i64 0, i64 2
  store double 0.000000e+00, ptr %27, align 8
  %28 = getelementptr inbounds [4 x double], ptr %6, i64 0, i64 3
  store double 0.000000e+00, ptr %28, align 8
  store i16 0, ptr %7, align 2
  br label %29

29:                                               ; preds = %141, %1
  %30 = load i16, ptr %7, align 2
  %31 = zext i16 %30 to i32
  %32 = load i16, ptr %2, align 2
  %33 = zext i16 %32 to i32
  %34 = icmp slt i32 %31, %33
  br i1 %34, label %35, label %144

35:                                               ; preds = %29
  store i16 0, ptr %8, align 2
  br label %36

36:                                               ; preds = %110, %35
  %37 = load i16, ptr %8, align 2
  %38 = zext i16 %37 to i32
  %39 = icmp slt i32 %38, 4
  br i1 %39, label %40, label %113

40:                                               ; preds = %36
  %41 = load i16, ptr %8, align 2
  %42 = zext i16 %41 to i32
  %43 = add nsw i32 %42, 1
  %44 = trunc i32 %43 to i16
  store i16 %44, ptr %9, align 2
  br label %45

45:                                               ; preds = %106, %40
  %46 = load i16, ptr %9, align 2
  %47 = zext i16 %46 to i32
  %48 = icmp slt i32 %47, 4
  br i1 %48, label %49, label %109

49:                                               ; preds = %45
  %50 = load i16, ptr %9, align 2
  %51 = zext i16 %50 to i64
  %52 = getelementptr inbounds nuw [4 x double], ptr %3, i64 0, i64 %51
  %53 = load double, ptr %52, align 8
  %54 = load i16, ptr %8, align 2
  %55 = zext i16 %54 to i64
  %56 = getelementptr inbounds nuw [4 x double], ptr %3, i64 0, i64 %55
  %57 = load double, ptr %56, align 8
  %58 = fsub double %53, %57
  store double %58, ptr %10, align 8
  %59 = load i16, ptr %9, align 2
  %60 = zext i16 %59 to i64
  %61 = getelementptr inbounds nuw [4 x double], ptr %4, i64 0, i64 %60
  %62 = load double, ptr %61, align 8
  %63 = load i16, ptr %8, align 2
  %64 = zext i16 %63 to i64
  %65 = getelementptr inbounds nuw [4 x double], ptr %4, i64 0, i64 %64
  %66 = load double, ptr %65, align 8
  %67 = fsub double %62, %66
  store double %67, ptr %11, align 8
  %68 = load double, ptr %10, align 8
  %69 = load double, ptr %10, align 8
  %70 = load double, ptr %11, align 8
  %71 = load double, ptr %11, align 8
  %72 = fmul double %70, %71
  %73 = call double @llvm.fmuladd.f64(double %68, double %69, double %72)
  %74 = fadd double %73, 1.250000e-01
  %75 = fdiv double 1.000000e-05, %74
  store double %75, ptr %12, align 8
  %76 = load double, ptr %10, align 8
  %77 = load double, ptr %12, align 8
  %78 = load i16, ptr %8, align 2
  %79 = zext i16 %78 to i64
  %80 = getelementptr inbounds nuw [4 x double], ptr %5, i64 0, i64 %79
  %81 = load double, ptr %80, align 8
  %82 = call double @llvm.fmuladd.f64(double %76, double %77, double %81)
  store double %82, ptr %80, align 8
  %83 = load double, ptr %11, align 8
  %84 = load double, ptr %12, align 8
  %85 = load i16, ptr %8, align 2
  %86 = zext i16 %85 to i64
  %87 = getelementptr inbounds nuw [4 x double], ptr %6, i64 0, i64 %86
  %88 = load double, ptr %87, align 8
  %89 = call double @llvm.fmuladd.f64(double %83, double %84, double %88)
  store double %89, ptr %87, align 8
  %90 = load double, ptr %10, align 8
  %91 = load double, ptr %12, align 8
  %92 = load i16, ptr %9, align 2
  %93 = zext i16 %92 to i64
  %94 = getelementptr inbounds nuw [4 x double], ptr %5, i64 0, i64 %93
  %95 = load double, ptr %94, align 8
  %96 = fneg double %90
  %97 = call double @llvm.fmuladd.f64(double %96, double %91, double %95)
  store double %97, ptr %94, align 8
  %98 = load double, ptr %11, align 8
  %99 = load double, ptr %12, align 8
  %100 = load i16, ptr %9, align 2
  %101 = zext i16 %100 to i64
  %102 = getelementptr inbounds nuw [4 x double], ptr %6, i64 0, i64 %101
  %103 = load double, ptr %102, align 8
  %104 = fneg double %98
  %105 = call double @llvm.fmuladd.f64(double %104, double %99, double %103)
  store double %105, ptr %102, align 8
  br label %106

106:                                              ; preds = %49
  %107 = load i16, ptr %9, align 2
  %108 = add i16 %107, 1
  store i16 %108, ptr %9, align 2
  br label %45, !llvm.loop !11

109:                                              ; preds = %45
  br label %110

110:                                              ; preds = %109
  %111 = load i16, ptr %8, align 2
  %112 = add i16 %111, 1
  store i16 %112, ptr %8, align 2
  br label %36, !llvm.loop !13

113:                                              ; preds = %36
  store i16 0, ptr %8, align 2
  br label %114

114:                                              ; preds = %137, %113
  %115 = load i16, ptr %8, align 2
  %116 = zext i16 %115 to i32
  %117 = icmp slt i32 %116, 4
  br i1 %117, label %118, label %140

118:                                              ; preds = %114
  %119 = load i16, ptr %8, align 2
  %120 = zext i16 %119 to i64
  %121 = getelementptr inbounds nuw [4 x double], ptr %5, i64 0, i64 %120
  %122 = load double, ptr %121, align 8
  %123 = load i16, ptr %8, align 2
  %124 = zext i16 %123 to i64
  %125 = getelementptr inbounds nuw [4 x double], ptr %3, i64 0, i64 %124
  %126 = load double, ptr %125, align 8
  %127 = fadd double %126, %122
  store double %127, ptr %125, align 8
  %128 = load i16, ptr %8, align 2
  %129 = zext i16 %128 to i64
  %130 = getelementptr inbounds nuw [4 x double], ptr %6, i64 0, i64 %129
  %131 = load double, ptr %130, align 8
  %132 = load i16, ptr %8, align 2
  %133 = zext i16 %132 to i64
  %134 = getelementptr inbounds nuw [4 x double], ptr %4, i64 0, i64 %133
  %135 = load double, ptr %134, align 8
  %136 = fadd double %135, %131
  store double %136, ptr %134, align 8
  br label %137

137:                                              ; preds = %118
  %138 = load i16, ptr %8, align 2
  %139 = add i16 %138, 1
  store i16 %139, ptr %8, align 2
  br label %114, !llvm.loop !14

140:                                              ; preds = %114
  br label %141

141:                                              ; preds = %140
  %142 = load i16, ptr %7, align 2
  %143 = add i16 %142, 1
  store i16 %143, ptr %7, align 2
  br label %29, !llvm.loop !15

144:                                              ; preds = %29
  %145 = getelementptr inbounds [4 x double], ptr %3, i64 0, i64 0
  %146 = load double, ptr %145, align 8
  %147 = getelementptr inbounds [4 x double], ptr %4, i64 0, i64 1
  %148 = load double, ptr %147, align 8
  %149 = fadd double %146, %148
  %150 = getelementptr inbounds [4 x double], ptr %3, i64 0, i64 2
  %151 = load double, ptr %150, align 8
  %152 = fadd double %149, %151
  %153 = getelementptr inbounds [4 x double], ptr %4, i64 0, i64 3
  %154 = load double, ptr %153, align 8
  %155 = fadd double %152, %154
  %156 = fmul double %155, 1.000000e+06
  %157 = fptosi double %156 to i64
  ret i64 %157
}

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.fmuladd.f64(double, double, double) #1

; Function Attrs: noinline nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = load volatile i32, ptr @opq0, align 4
  %3 = add nsw i32 200, %2
  %4 = trunc i32 %3 to i16
  %5 = call i64 @bench_nbody(i16 noundef %4)
  call void @report(i64 noundef %5)
  ret i32 0
}

declare void @report(i64 noundef) #2

attributes #0 = { noinline nounwind uwtable "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8, !9}
!llvm.ident = !{!10}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"uwtable", i32 2}
!9 = !{i32 7, !"frame-pointer", i32 4}
!10 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = distinct !{!14, !12}
!15 = distinct !{!15, !12}
