; ModuleID = 'bench/nbody_single/nbody_single.c'
source_filename = "bench/nbody_single/nbody_single.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@pos_x = internal global [6 x float] zeroinitializer, align 4
@pos_y = internal global [6 x float] zeroinitializer, align 4
@vel_x = internal global [6 x float] zeroinitializer, align 4
@vel_y = internal global [6 x float] zeroinitializer, align 4

; Function Attrs: nounwind uwtable
define dso_local i64 @bench_nbody_single() #0 {
  %1 = alloca float, align 4
  %2 = alloca float, align 4
  %3 = alloca float, align 4
  %4 = alloca float, align 4
  %5 = alloca float, align 4
  %6 = alloca float, align 4
  %7 = alloca float, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  store i32 0, ptr %8, align 4, !tbaa !6
  br label %11

11:                                               ; preds = %35, %0
  %12 = load i32, ptr %8, align 4, !tbaa !6
  %13 = icmp slt i32 %12, 6
  br i1 %13, label %14, label %38

14:                                               ; preds = %11
  %15 = load i32, ptr %8, align 4, !tbaa !6
  %16 = mul nsw i32 %15, 7
  %17 = sub nsw i32 %16, 15
  %18 = sitofp i32 %17 to float
  %19 = load i32, ptr %8, align 4, !tbaa !6
  %20 = sext i32 %19 to i64
  %21 = getelementptr inbounds [6 x float], ptr @pos_x, i64 0, i64 %20
  store float %18, ptr %21, align 4, !tbaa !10
  %22 = load i32, ptr %8, align 4, !tbaa !6
  %23 = mul nsw i32 %22, 5
  %24 = sub nsw i32 %23, 12
  %25 = sitofp i32 %24 to float
  %26 = load i32, ptr %8, align 4, !tbaa !6
  %27 = sext i32 %26 to i64
  %28 = getelementptr inbounds [6 x float], ptr @pos_y, i64 0, i64 %27
  store float %25, ptr %28, align 4, !tbaa !10
  %29 = load i32, ptr %8, align 4, !tbaa !6
  %30 = sext i32 %29 to i64
  %31 = getelementptr inbounds [6 x float], ptr @vel_x, i64 0, i64 %30
  store float 0.000000e+00, ptr %31, align 4, !tbaa !10
  %32 = load i32, ptr %8, align 4, !tbaa !6
  %33 = sext i32 %32 to i64
  %34 = getelementptr inbounds [6 x float], ptr @vel_y, i64 0, i64 %33
  store float 0.000000e+00, ptr %34, align 4, !tbaa !10
  br label %35

35:                                               ; preds = %14
  %36 = load i32, ptr %8, align 4, !tbaa !6
  %37 = add nsw i32 %36, 1
  store i32 %37, ptr %8, align 4, !tbaa !6
  br label %11, !llvm.loop !12

38:                                               ; preds = %11
  store i32 0, ptr %10, align 4, !tbaa !6
  br label %39

39:                                               ; preds = %176, %38
  %40 = load i32, ptr %10, align 4, !tbaa !6
  %41 = icmp slt i32 %40, 1000
  br i1 %41, label %42, label %179

42:                                               ; preds = %39
  store i32 0, ptr %8, align 4, !tbaa !6
  br label %43

43:                                               ; preds = %140, %42
  %44 = load i32, ptr %8, align 4, !tbaa !6
  %45 = icmp slt i32 %44, 6
  br i1 %45, label %46, label %143

46:                                               ; preds = %43
  store float 0.000000e+00, ptr %5, align 4, !tbaa !10
  store float 0.000000e+00, ptr %6, align 4, !tbaa !10
  store i32 0, ptr %9, align 4, !tbaa !6
  br label %47

47:                                               ; preds = %92, %46
  %48 = load i32, ptr %9, align 4, !tbaa !6
  %49 = icmp slt i32 %48, 6
  br i1 %49, label %50, label %95

50:                                               ; preds = %47
  %51 = load i32, ptr %9, align 4, !tbaa !6
  %52 = load i32, ptr %8, align 4, !tbaa !6
  %53 = icmp ne i32 %51, %52
  br i1 %53, label %54, label %91

54:                                               ; preds = %50
  %55 = load i32, ptr %9, align 4, !tbaa !6
  %56 = sext i32 %55 to i64
  %57 = getelementptr inbounds [6 x float], ptr @pos_x, i64 0, i64 %56
  %58 = load float, ptr %57, align 4, !tbaa !10
  %59 = load i32, ptr %8, align 4, !tbaa !6
  %60 = sext i32 %59 to i64
  %61 = getelementptr inbounds [6 x float], ptr @pos_x, i64 0, i64 %60
  %62 = load float, ptr %61, align 4, !tbaa !10
  %63 = fsub float %58, %62
  store float %63, ptr %1, align 4, !tbaa !10
  %64 = load i32, ptr %9, align 4, !tbaa !6
  %65 = sext i32 %64 to i64
  %66 = getelementptr inbounds [6 x float], ptr @pos_y, i64 0, i64 %65
  %67 = load float, ptr %66, align 4, !tbaa !10
  %68 = load i32, ptr %8, align 4, !tbaa !6
  %69 = sext i32 %68 to i64
  %70 = getelementptr inbounds [6 x float], ptr @pos_y, i64 0, i64 %69
  %71 = load float, ptr %70, align 4, !tbaa !10
  %72 = fsub float %67, %71
  store float %72, ptr %2, align 4, !tbaa !10
  %73 = load float, ptr %1, align 4, !tbaa !10
  %74 = load float, ptr %1, align 4, !tbaa !10
  %75 = load float, ptr %2, align 4, !tbaa !10
  %76 = load float, ptr %2, align 4, !tbaa !10
  %77 = fmul float %75, %76
  %78 = call float @llvm.fmuladd.f32(float %73, float %74, float %77)
  %79 = fadd float %78, 1.000000e+00
  store float %79, ptr %3, align 4, !tbaa !10
  %80 = load float, ptr %3, align 4, !tbaa !10
  %81 = fadd float %80, 1.000000e+00
  %82 = fdiv float 1.000000e+00, %81
  store float %82, ptr %4, align 4, !tbaa !10
  %83 = load float, ptr %5, align 4, !tbaa !10
  %84 = load float, ptr %1, align 4, !tbaa !10
  %85 = load float, ptr %4, align 4, !tbaa !10
  %86 = call float @llvm.fmuladd.f32(float %84, float %85, float %83)
  store float %86, ptr %5, align 4, !tbaa !10
  %87 = load float, ptr %6, align 4, !tbaa !10
  %88 = load float, ptr %2, align 4, !tbaa !10
  %89 = load float, ptr %4, align 4, !tbaa !10
  %90 = call float @llvm.fmuladd.f32(float %88, float %89, float %87)
  store float %90, ptr %6, align 4, !tbaa !10
  br label %91

91:                                               ; preds = %54, %50
  br label %92

92:                                               ; preds = %91
  %93 = load i32, ptr %9, align 4, !tbaa !6
  %94 = add nsw i32 %93, 1
  store i32 %94, ptr %9, align 4, !tbaa !6
  br label %47, !llvm.loop !14

95:                                               ; preds = %47
  %96 = load i32, ptr %8, align 4, !tbaa !6
  %97 = sext i32 %96 to i64
  %98 = getelementptr inbounds [6 x float], ptr @vel_x, i64 0, i64 %97
  %99 = load float, ptr %98, align 4, !tbaa !10
  %100 = load float, ptr %5, align 4, !tbaa !10
  %101 = fadd float %99, %100
  %102 = load i32, ptr %8, align 4, !tbaa !6
  %103 = sext i32 %102 to i64
  %104 = getelementptr inbounds [6 x float], ptr @vel_x, i64 0, i64 %103
  store float %101, ptr %104, align 4, !tbaa !10
  %105 = load i32, ptr %8, align 4, !tbaa !6
  %106 = sext i32 %105 to i64
  %107 = getelementptr inbounds [6 x float], ptr @vel_y, i64 0, i64 %106
  %108 = load float, ptr %107, align 4, !tbaa !10
  %109 = load float, ptr %6, align 4, !tbaa !10
  %110 = fadd float %108, %109
  %111 = load i32, ptr %8, align 4, !tbaa !6
  %112 = sext i32 %111 to i64
  %113 = getelementptr inbounds [6 x float], ptr @vel_y, i64 0, i64 %112
  store float %110, ptr %113, align 4, !tbaa !10
  %114 = load i32, ptr %8, align 4, !tbaa !6
  %115 = sext i32 %114 to i64
  %116 = getelementptr inbounds [6 x float], ptr @vel_x, i64 0, i64 %115
  %117 = load float, ptr %116, align 4, !tbaa !10
  %118 = load i32, ptr %8, align 4, !tbaa !6
  %119 = sext i32 %118 to i64
  %120 = getelementptr inbounds [6 x float], ptr @vel_x, i64 0, i64 %119
  %121 = load float, ptr %120, align 4, !tbaa !10
  %122 = fdiv float %121, 8.000000e+00
  %123 = fsub float %117, %122
  %124 = load i32, ptr %8, align 4, !tbaa !6
  %125 = sext i32 %124 to i64
  %126 = getelementptr inbounds [6 x float], ptr @vel_x, i64 0, i64 %125
  store float %123, ptr %126, align 4, !tbaa !10
  %127 = load i32, ptr %8, align 4, !tbaa !6
  %128 = sext i32 %127 to i64
  %129 = getelementptr inbounds [6 x float], ptr @vel_y, i64 0, i64 %128
  %130 = load float, ptr %129, align 4, !tbaa !10
  %131 = load i32, ptr %8, align 4, !tbaa !6
  %132 = sext i32 %131 to i64
  %133 = getelementptr inbounds [6 x float], ptr @vel_y, i64 0, i64 %132
  %134 = load float, ptr %133, align 4, !tbaa !10
  %135 = fdiv float %134, 8.000000e+00
  %136 = fsub float %130, %135
  %137 = load i32, ptr %8, align 4, !tbaa !6
  %138 = sext i32 %137 to i64
  %139 = getelementptr inbounds [6 x float], ptr @vel_y, i64 0, i64 %138
  store float %136, ptr %139, align 4, !tbaa !10
  br label %140

140:                                              ; preds = %95
  %141 = load i32, ptr %8, align 4, !tbaa !6
  %142 = add nsw i32 %141, 1
  store i32 %142, ptr %8, align 4, !tbaa !6
  br label %43, !llvm.loop !15

143:                                              ; preds = %43
  store i32 0, ptr %8, align 4, !tbaa !6
  br label %144

144:                                              ; preds = %172, %143
  %145 = load i32, ptr %8, align 4, !tbaa !6
  %146 = icmp slt i32 %145, 6
  br i1 %146, label %147, label %175

147:                                              ; preds = %144
  %148 = load i32, ptr %8, align 4, !tbaa !6
  %149 = sext i32 %148 to i64
  %150 = getelementptr inbounds [6 x float], ptr @pos_x, i64 0, i64 %149
  %151 = load float, ptr %150, align 4, !tbaa !10
  %152 = load i32, ptr %8, align 4, !tbaa !6
  %153 = sext i32 %152 to i64
  %154 = getelementptr inbounds [6 x float], ptr @vel_x, i64 0, i64 %153
  %155 = load float, ptr %154, align 4, !tbaa !10
  %156 = fadd float %151, %155
  %157 = load i32, ptr %8, align 4, !tbaa !6
  %158 = sext i32 %157 to i64
  %159 = getelementptr inbounds [6 x float], ptr @pos_x, i64 0, i64 %158
  store float %156, ptr %159, align 4, !tbaa !10
  %160 = load i32, ptr %8, align 4, !tbaa !6
  %161 = sext i32 %160 to i64
  %162 = getelementptr inbounds [6 x float], ptr @pos_y, i64 0, i64 %161
  %163 = load float, ptr %162, align 4, !tbaa !10
  %164 = load i32, ptr %8, align 4, !tbaa !6
  %165 = sext i32 %164 to i64
  %166 = getelementptr inbounds [6 x float], ptr @vel_y, i64 0, i64 %165
  %167 = load float, ptr %166, align 4, !tbaa !10
  %168 = fadd float %163, %167
  %169 = load i32, ptr %8, align 4, !tbaa !6
  %170 = sext i32 %169 to i64
  %171 = getelementptr inbounds [6 x float], ptr @pos_y, i64 0, i64 %170
  store float %168, ptr %171, align 4, !tbaa !10
  br label %172

172:                                              ; preds = %147
  %173 = load i32, ptr %8, align 4, !tbaa !6
  %174 = add nsw i32 %173, 1
  store i32 %174, ptr %8, align 4, !tbaa !6
  br label %144, !llvm.loop !16

175:                                              ; preds = %144
  br label %176

176:                                              ; preds = %175
  %177 = load i32, ptr %10, align 4, !tbaa !6
  %178 = add nsw i32 %177, 1
  store i32 %178, ptr %10, align 4, !tbaa !6
  br label %39, !llvm.loop !17

179:                                              ; preds = %39
  store float 0.000000e+00, ptr %7, align 4, !tbaa !10
  store i32 0, ptr %8, align 4, !tbaa !6
  br label %180

180:                                              ; preds = %208, %179
  %181 = load i32, ptr %8, align 4, !tbaa !6
  %182 = icmp slt i32 %181, 6
  br i1 %182, label %183, label %211

183:                                              ; preds = %180
  %184 = load float, ptr %7, align 4, !tbaa !10
  %185 = load i32, ptr %8, align 4, !tbaa !6
  %186 = add nsw i32 %185, 1
  %187 = sitofp i32 %186 to float
  %188 = load i32, ptr %8, align 4, !tbaa !6
  %189 = sext i32 %188 to i64
  %190 = getelementptr inbounds [6 x float], ptr @pos_x, i64 0, i64 %189
  %191 = load float, ptr %190, align 4, !tbaa !10
  %192 = load i32, ptr %8, align 4, !tbaa !6
  %193 = sext i32 %192 to i64
  %194 = getelementptr inbounds [6 x float], ptr @pos_y, i64 0, i64 %193
  %195 = load float, ptr %194, align 4, !tbaa !10
  %196 = call float @llvm.fmuladd.f32(float 3.000000e+00, float %195, float %191)
  %197 = load i32, ptr %8, align 4, !tbaa !6
  %198 = sext i32 %197 to i64
  %199 = getelementptr inbounds [6 x float], ptr @vel_x, i64 0, i64 %198
  %200 = load float, ptr %199, align 4, !tbaa !10
  %201 = call float @llvm.fmuladd.f32(float 5.000000e+00, float %200, float %196)
  %202 = load i32, ptr %8, align 4, !tbaa !6
  %203 = sext i32 %202 to i64
  %204 = getelementptr inbounds [6 x float], ptr @vel_y, i64 0, i64 %203
  %205 = load float, ptr %204, align 4, !tbaa !10
  %206 = call float @llvm.fmuladd.f32(float 7.000000e+00, float %205, float %201)
  %207 = call float @llvm.fmuladd.f32(float %187, float %206, float %184)
  store float %207, ptr %7, align 4, !tbaa !10
  br label %208

208:                                              ; preds = %183
  %209 = load i32, ptr %8, align 4, !tbaa !6
  %210 = add nsw i32 %209, 1
  store i32 %210, ptr %8, align 4, !tbaa !6
  br label %180, !llvm.loop !18

211:                                              ; preds = %180
  %212 = load float, ptr %7, align 4, !tbaa !10
  %213 = call float @llvm.fmuladd.f32(float %212, float 1.024000e+03, float 5.000000e-01)
  %214 = fptosi float %213 to i64
  ret i64 %214
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare float @llvm.fmuladd.f32(float, float, float) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i64 @bench_nbody_single()
  call void @report(i64 noundef %2)
  ret i32 0
}

declare void @report(i64 noundef) #2

attributes #0 = { nounwind uwtable "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #2 = { "frame-pointer"="non-leaf" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }

!llvm.module.flags = !{!0, !1, !2, !3, !4}
!llvm.ident = !{!5}

!0 = !{i32 1, !"wchar_size", i32 4}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{i32 7, !"frame-pointer", i32 1}
!5 = !{!"Ubuntu clang version 20.1.8 (0ubuntu4)"}
!6 = !{!7, !7, i64 0}
!7 = !{!"int", !8, i64 0}
!8 = !{!"omnipotent char", !9, i64 0}
!9 = !{!"Simple C/C++ TBAA"}
!10 = !{!11, !11, i64 0}
!11 = !{!"float", !8, i64 0}
!12 = distinct !{!12, !13}
!13 = !{!"llvm.loop.mustprogress"}
!14 = distinct !{!14, !13}
!15 = distinct !{!15, !13}
!16 = distinct !{!16, !13}
!17 = distinct !{!17, !13}
!18 = distinct !{!18, !13}
