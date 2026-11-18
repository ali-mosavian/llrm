; ModuleID = 'bench/nbody_single/nbody_single.c'
source_filename = "bench/nbody_single/nbody_single.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@pos_x = internal global [6 x float] zeroinitializer, align 4
@pos_y = internal global [6 x float] zeroinitializer, align 4
@vel_x = internal global [6 x float] zeroinitializer, align 4
@vel_y = internal global [6 x float] zeroinitializer, align 4

; Function Attrs: nounwind
define dso_local i32 @bench_nbody_single() #0 {
  %1 = alloca float, align 4
  %2 = alloca float, align 4
  %3 = alloca float, align 4
  %4 = alloca float, align 4
  %5 = alloca float, align 4
  %6 = alloca float, align 4
  %7 = alloca float, align 4
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  %10 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #4
  call void @llvm.lifetime.start.p0(ptr %2) #4
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  call void @llvm.lifetime.start.p0(ptr %5) #4
  call void @llvm.lifetime.start.p0(ptr %6) #4
  call void @llvm.lifetime.start.p0(ptr %7) #4
  call void @llvm.lifetime.start.p0(ptr %8) #4
  call void @llvm.lifetime.start.p0(ptr %9) #4
  call void @llvm.lifetime.start.p0(ptr %10) #4
  store i32 0, ptr %8, align 2, !tbaa !8
  br label %11

11:                                               ; preds = %31, %0
  %12 = load i32, ptr %8, align 2, !tbaa !8
  %13 = icmp slt i32 %12, 6
  br i1 %13, label %14, label %34

14:                                               ; preds = %11
  %15 = load i32, ptr %8, align 2, !tbaa !8
  %16 = mul nsw i32 %15, 7
  %17 = sub nsw i32 %16, 15
  %18 = sitofp i32 %17 to float
  %19 = load i32, ptr %8, align 2, !tbaa !8
  %20 = getelementptr inbounds [6 x float], ptr @pos_x, i32 0, i32 %19
  store float %18, ptr %20, align 4, !tbaa !9
  %21 = load i32, ptr %8, align 2, !tbaa !8
  %22 = mul nsw i32 %21, 5
  %23 = sub nsw i32 %22, 12
  %24 = sitofp i32 %23 to float
  %25 = load i32, ptr %8, align 2, !tbaa !8
  %26 = getelementptr inbounds [6 x float], ptr @pos_y, i32 0, i32 %25
  store float %24, ptr %26, align 4, !tbaa !9
  %27 = load i32, ptr %8, align 2, !tbaa !8
  %28 = getelementptr inbounds [6 x float], ptr @vel_x, i32 0, i32 %27
  store float 0.000000e+00, ptr %28, align 4, !tbaa !9
  %29 = load i32, ptr %8, align 2, !tbaa !8
  %30 = getelementptr inbounds [6 x float], ptr @vel_y, i32 0, i32 %29
  store float 0.000000e+00, ptr %30, align 4, !tbaa !9
  br label %31

31:                                               ; preds = %14
  %32 = load i32, ptr %8, align 2, !tbaa !8
  %33 = add nsw i32 %32, 1
  store i32 %33, ptr %8, align 2, !tbaa !8
  br label %11, !llvm.loop !11

34:                                               ; preds = %11
  store i32 0, ptr %10, align 2, !tbaa !8
  br label %35

35:                                               ; preds = %152, %34
  %36 = load i32, ptr %10, align 2, !tbaa !8
  %37 = icmp slt i32 %36, 1000
  br i1 %37, label %38, label %155

38:                                               ; preds = %35
  store i32 0, ptr %8, align 2, !tbaa !8
  br label %39

39:                                               ; preds = %122, %38
  %40 = load i32, ptr %8, align 2, !tbaa !8
  %41 = icmp slt i32 %40, 6
  br i1 %41, label %42, label %125

42:                                               ; preds = %39
  store float 0.000000e+00, ptr %5, align 4, !tbaa !9
  store float 0.000000e+00, ptr %6, align 4, !tbaa !9
  store i32 0, ptr %9, align 2, !tbaa !8
  br label %43

43:                                               ; preds = %84, %42
  %44 = load i32, ptr %9, align 2, !tbaa !8
  %45 = icmp slt i32 %44, 6
  br i1 %45, label %46, label %87

46:                                               ; preds = %43
  %47 = load i32, ptr %9, align 2, !tbaa !8
  %48 = load i32, ptr %8, align 2, !tbaa !8
  %49 = icmp ne i32 %47, %48
  br i1 %49, label %50, label %83

50:                                               ; preds = %46
  %51 = load i32, ptr %9, align 2, !tbaa !8
  %52 = getelementptr inbounds [6 x float], ptr @pos_x, i32 0, i32 %51
  %53 = load float, ptr %52, align 4, !tbaa !9
  %54 = load i32, ptr %8, align 2, !tbaa !8
  %55 = getelementptr inbounds [6 x float], ptr @pos_x, i32 0, i32 %54
  %56 = load float, ptr %55, align 4, !tbaa !9
  %57 = fsub float %53, %56
  store float %57, ptr %1, align 4, !tbaa !9
  %58 = load i32, ptr %9, align 2, !tbaa !8
  %59 = getelementptr inbounds [6 x float], ptr @pos_y, i32 0, i32 %58
  %60 = load float, ptr %59, align 4, !tbaa !9
  %61 = load i32, ptr %8, align 2, !tbaa !8
  %62 = getelementptr inbounds [6 x float], ptr @pos_y, i32 0, i32 %61
  %63 = load float, ptr %62, align 4, !tbaa !9
  %64 = fsub float %60, %63
  store float %64, ptr %2, align 4, !tbaa !9
  %65 = load float, ptr %1, align 4, !tbaa !9
  %66 = load float, ptr %1, align 4, !tbaa !9
  %67 = load float, ptr %2, align 4, !tbaa !9
  %68 = load float, ptr %2, align 4, !tbaa !9
  %69 = fmul float %67, %68
  %70 = call float @llvm.fmuladd.f32(float %65, float %66, float %69)
  %71 = fadd float %70, 1.000000e+00
  store float %71, ptr %3, align 4, !tbaa !9
  %72 = load float, ptr %3, align 4, !tbaa !9
  %73 = fadd float %72, 1.000000e+00
  %74 = fdiv float 1.000000e+00, %73
  store float %74, ptr %4, align 4, !tbaa !9
  %75 = load float, ptr %5, align 4, !tbaa !9
  %76 = load float, ptr %1, align 4, !tbaa !9
  %77 = load float, ptr %4, align 4, !tbaa !9
  %78 = call float @llvm.fmuladd.f32(float %76, float %77, float %75)
  store float %78, ptr %5, align 4, !tbaa !9
  %79 = load float, ptr %6, align 4, !tbaa !9
  %80 = load float, ptr %2, align 4, !tbaa !9
  %81 = load float, ptr %4, align 4, !tbaa !9
  %82 = call float @llvm.fmuladd.f32(float %80, float %81, float %79)
  store float %82, ptr %6, align 4, !tbaa !9
  br label %83

83:                                               ; preds = %50, %46
  br label %84

84:                                               ; preds = %83
  %85 = load i32, ptr %9, align 2, !tbaa !8
  %86 = add nsw i32 %85, 1
  store i32 %86, ptr %9, align 2, !tbaa !8
  br label %43, !llvm.loop !13

87:                                               ; preds = %43
  %88 = load i32, ptr %8, align 2, !tbaa !8
  %89 = getelementptr inbounds [6 x float], ptr @vel_x, i32 0, i32 %88
  %90 = load float, ptr %89, align 4, !tbaa !9
  %91 = load float, ptr %5, align 4, !tbaa !9
  %92 = fadd float %90, %91
  %93 = load i32, ptr %8, align 2, !tbaa !8
  %94 = getelementptr inbounds [6 x float], ptr @vel_x, i32 0, i32 %93
  store float %92, ptr %94, align 4, !tbaa !9
  %95 = load i32, ptr %8, align 2, !tbaa !8
  %96 = getelementptr inbounds [6 x float], ptr @vel_y, i32 0, i32 %95
  %97 = load float, ptr %96, align 4, !tbaa !9
  %98 = load float, ptr %6, align 4, !tbaa !9
  %99 = fadd float %97, %98
  %100 = load i32, ptr %8, align 2, !tbaa !8
  %101 = getelementptr inbounds [6 x float], ptr @vel_y, i32 0, i32 %100
  store float %99, ptr %101, align 4, !tbaa !9
  %102 = load i32, ptr %8, align 2, !tbaa !8
  %103 = getelementptr inbounds [6 x float], ptr @vel_x, i32 0, i32 %102
  %104 = load float, ptr %103, align 4, !tbaa !9
  %105 = load i32, ptr %8, align 2, !tbaa !8
  %106 = getelementptr inbounds [6 x float], ptr @vel_x, i32 0, i32 %105
  %107 = load float, ptr %106, align 4, !tbaa !9
  %108 = fdiv float %107, 8.000000e+00
  %109 = fsub float %104, %108
  %110 = load i32, ptr %8, align 2, !tbaa !8
  %111 = getelementptr inbounds [6 x float], ptr @vel_x, i32 0, i32 %110
  store float %109, ptr %111, align 4, !tbaa !9
  %112 = load i32, ptr %8, align 2, !tbaa !8
  %113 = getelementptr inbounds [6 x float], ptr @vel_y, i32 0, i32 %112
  %114 = load float, ptr %113, align 4, !tbaa !9
  %115 = load i32, ptr %8, align 2, !tbaa !8
  %116 = getelementptr inbounds [6 x float], ptr @vel_y, i32 0, i32 %115
  %117 = load float, ptr %116, align 4, !tbaa !9
  %118 = fdiv float %117, 8.000000e+00
  %119 = fsub float %114, %118
  %120 = load i32, ptr %8, align 2, !tbaa !8
  %121 = getelementptr inbounds [6 x float], ptr @vel_y, i32 0, i32 %120
  store float %119, ptr %121, align 4, !tbaa !9
  br label %122

122:                                              ; preds = %87
  %123 = load i32, ptr %8, align 2, !tbaa !8
  %124 = add nsw i32 %123, 1
  store i32 %124, ptr %8, align 2, !tbaa !8
  br label %39, !llvm.loop !14

125:                                              ; preds = %39
  store i32 0, ptr %8, align 2, !tbaa !8
  br label %126

126:                                              ; preds = %148, %125
  %127 = load i32, ptr %8, align 2, !tbaa !8
  %128 = icmp slt i32 %127, 6
  br i1 %128, label %129, label %151

129:                                              ; preds = %126
  %130 = load i32, ptr %8, align 2, !tbaa !8
  %131 = getelementptr inbounds [6 x float], ptr @pos_x, i32 0, i32 %130
  %132 = load float, ptr %131, align 4, !tbaa !9
  %133 = load i32, ptr %8, align 2, !tbaa !8
  %134 = getelementptr inbounds [6 x float], ptr @vel_x, i32 0, i32 %133
  %135 = load float, ptr %134, align 4, !tbaa !9
  %136 = fadd float %132, %135
  %137 = load i32, ptr %8, align 2, !tbaa !8
  %138 = getelementptr inbounds [6 x float], ptr @pos_x, i32 0, i32 %137
  store float %136, ptr %138, align 4, !tbaa !9
  %139 = load i32, ptr %8, align 2, !tbaa !8
  %140 = getelementptr inbounds [6 x float], ptr @pos_y, i32 0, i32 %139
  %141 = load float, ptr %140, align 4, !tbaa !9
  %142 = load i32, ptr %8, align 2, !tbaa !8
  %143 = getelementptr inbounds [6 x float], ptr @vel_y, i32 0, i32 %142
  %144 = load float, ptr %143, align 4, !tbaa !9
  %145 = fadd float %141, %144
  %146 = load i32, ptr %8, align 2, !tbaa !8
  %147 = getelementptr inbounds [6 x float], ptr @pos_y, i32 0, i32 %146
  store float %145, ptr %147, align 4, !tbaa !9
  br label %148

148:                                              ; preds = %129
  %149 = load i32, ptr %8, align 2, !tbaa !8
  %150 = add nsw i32 %149, 1
  store i32 %150, ptr %8, align 2, !tbaa !8
  br label %126, !llvm.loop !15

151:                                              ; preds = %126
  br label %152

152:                                              ; preds = %151
  %153 = load i32, ptr %10, align 2, !tbaa !8
  %154 = add nsw i32 %153, 1
  store i32 %154, ptr %10, align 2, !tbaa !8
  br label %35, !llvm.loop !16

155:                                              ; preds = %35
  store float 0.000000e+00, ptr %7, align 4, !tbaa !9
  store i32 0, ptr %8, align 2, !tbaa !8
  br label %156

156:                                              ; preds = %180, %155
  %157 = load i32, ptr %8, align 2, !tbaa !8
  %158 = icmp slt i32 %157, 6
  br i1 %158, label %159, label %183

159:                                              ; preds = %156
  %160 = load float, ptr %7, align 4, !tbaa !9
  %161 = load i32, ptr %8, align 2, !tbaa !8
  %162 = add nsw i32 %161, 1
  %163 = sitofp i32 %162 to float
  %164 = load i32, ptr %8, align 2, !tbaa !8
  %165 = getelementptr inbounds [6 x float], ptr @pos_x, i32 0, i32 %164
  %166 = load float, ptr %165, align 4, !tbaa !9
  %167 = load i32, ptr %8, align 2, !tbaa !8
  %168 = getelementptr inbounds [6 x float], ptr @pos_y, i32 0, i32 %167
  %169 = load float, ptr %168, align 4, !tbaa !9
  %170 = call float @llvm.fmuladd.f32(float 3.000000e+00, float %169, float %166)
  %171 = load i32, ptr %8, align 2, !tbaa !8
  %172 = getelementptr inbounds [6 x float], ptr @vel_x, i32 0, i32 %171
  %173 = load float, ptr %172, align 4, !tbaa !9
  %174 = call float @llvm.fmuladd.f32(float 5.000000e+00, float %173, float %170)
  %175 = load i32, ptr %8, align 2, !tbaa !8
  %176 = getelementptr inbounds [6 x float], ptr @vel_y, i32 0, i32 %175
  %177 = load float, ptr %176, align 4, !tbaa !9
  %178 = call float @llvm.fmuladd.f32(float 7.000000e+00, float %177, float %174)
  %179 = call float @llvm.fmuladd.f32(float %163, float %178, float %160)
  store float %179, ptr %7, align 4, !tbaa !9
  br label %180

180:                                              ; preds = %159
  %181 = load i32, ptr %8, align 2, !tbaa !8
  %182 = add nsw i32 %181, 1
  store i32 %182, ptr %8, align 2, !tbaa !8
  br label %156, !llvm.loop !17

183:                                              ; preds = %156
  %184 = load float, ptr %7, align 4, !tbaa !9
  %185 = call float @llvm.fmuladd.f32(float %184, float 1.024000e+03, float 5.000000e-01)
  %186 = fptosi float %185 to i32
  call void @llvm.lifetime.end.p0(ptr %10) #4
  call void @llvm.lifetime.end.p0(ptr %9) #4
  call void @llvm.lifetime.end.p0(ptr %8) #4
  call void @llvm.lifetime.end.p0(ptr %7) #4
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  call void @llvm.lifetime.end.p0(ptr %1) #4
  ret i32 %186
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare float @llvm.fmuladd.f32(float, float, float) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 2
  store i32 0, ptr %1, align 2
  %2 = call i32 @bench_nbody_single() #5
  call void @report(i32 noundef %2) #5
  ret i32 0
}

declare void @report(i32 noundef) #3

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin "no-builtins" }

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
!8 = !{!5, !5, i64 0}
!9 = !{!10, !10, i64 0}
!10 = !{!"float", !6, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = distinct !{!14, !12}
!15 = distinct !{!15, !12}
!16 = distinct !{!16, !12}
!17 = distinct !{!17, !12}
