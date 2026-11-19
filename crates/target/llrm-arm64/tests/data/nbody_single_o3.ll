target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@pos_x = internal unnamed_addr global [6 x float] zeroinitializer, align 4
@pos_y = internal unnamed_addr global [6 x float] zeroinitializer, align 4
@vel_x = internal unnamed_addr global [6 x float] zeroinitializer, align 4
@vel_y = internal unnamed_addr global [6 x float] zeroinitializer, align 4

define i64 @bench_nbody_single() noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" memory(readwrite, argmem: none, inaccessiblemem: none) nofree nosync norecurse willreturn {
  br label %1

1:
  %2 = phi i64 [ %13, %1 ], [ 0, %0 ]
  %3 = phi i32 [ %12, %1 ], [ -15, %0 ]
  %4 = phi i32 [ %11, %1 ], [ -12, %0 ]
  %5 = sitofp i32 %3 to float
  %6 = getelementptr i8, ptr @pos_x, i64 %2
  store float %5, ptr %6, align 4, !tbaa !15
  %7 = sitofp i32 %4 to float
  %8 = getelementptr i8, ptr @pos_y, i64 %2
  store float %7, ptr %8, align 4, !tbaa !15
  %9 = getelementptr i8, ptr @vel_x, i64 %2
  store float 0.000000e+00, ptr %9, align 4, !tbaa !15
  %10 = getelementptr i8, ptr @vel_y, i64 %2
  store float 0.000000e+00, ptr %10, align 4, !tbaa !15
  %11 = add nsw i32 %4, 5
  %12 = add nsw i32 %3, 7
  %13 = add nuw nsw i64 %2, 4
  %exitcond = icmp eq i64 %13, 24
  br i1 %exitcond, label %14, label %1, !llvm.loop !17

14:
  %promoted = load float, ptr @pos_x, align 4, !tbaa !15
  %promoted1 = load float, ptr @pos_y, align 4, !tbaa !15
  %promoted2 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 4), align 4, !tbaa !15
  %promoted3 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 4), align 4, !tbaa !15
  %promoted4 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 8), align 4, !tbaa !15
  %promoted5 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 8), align 4, !tbaa !15
  %promoted6 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 12), align 4, !tbaa !15
  %promoted7 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 12), align 4, !tbaa !15
  %promoted8 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 16), align 4, !tbaa !15
  %promoted9 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 16), align 4, !tbaa !15
  %promoted10 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 20), align 4, !tbaa !15
  %promoted11 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 20), align 4, !tbaa !15
  %promoted12 = load float, ptr @vel_x, align 4, !tbaa !15
  %promoted13 = load float, ptr @vel_y, align 4, !tbaa !15
  %promoted14 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 4), align 4, !tbaa !15
  %promoted15 = load float, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 4), align 4, !tbaa !15
  br label %173

15:
  %16 = phi i64 [ 8, %173 ], [ %120, %107 ]
  %17 = getelementptr i8, ptr @pos_x, i64 %16
  %18 = getelementptr i8, ptr @pos_y, i64 %16
  %19 = load float, ptr %18, align 4, !tbaa !15
  %20 = fsub float %188, %19
  %21 = load float, ptr %17, align 4, !tbaa !15
  %22 = fsub float %189, %21
  %23 = fmul float %20, %20
  %24 = tail call float @llvm.fmuladd.f32(float %22, float %22, float %23)
  %25 = fadd float %24, 1.000000e+00
  %26 = fadd float %25, 1.000000e+00
  %27 = fdiv float 1.000000e+00, %26
  %28 = tail call float @llvm.fmuladd.f32(float %20, float %27, float 0.000000e+00)
  %29 = tail call float @llvm.fmuladd.f32(float %22, float %27, float 0.000000e+00)
  %30 = load float, ptr %17, align 4, !tbaa !15
  %31 = fsub float %187, %30
  %32 = load float, ptr %18, align 4, !tbaa !15
  %33 = fsub float %186, %32
  %34 = fmul float %33, %33
  %35 = tail call float @llvm.fmuladd.f32(float %31, float %31, float %34)
  %36 = fadd float %35, 1.000000e+00
  %37 = fadd float %36, 1.000000e+00
  %38 = fdiv float 1.000000e+00, %37
  %39 = tail call float @llvm.fmuladd.f32(float %31, float %38, float %29)
  %40 = tail call float @llvm.fmuladd.f32(float %33, float %38, float %28)
  %41 = icmp eq i64 %16, 8
  br i1 %41, label %57, label %42

42:
  %43 = getelementptr i8, ptr @pos_x, i64 %16
  %44 = load float, ptr %43, align 4, !tbaa !15
  %45 = fsub float %185, %44
  %46 = getelementptr i8, ptr @pos_y, i64 %16
  %47 = load float, ptr %46, align 4, !tbaa !15
  %48 = fsub float %184, %47
  %49 = fmul float %48, %48
  %50 = tail call float @llvm.fmuladd.f32(float %45, float %45, float %49)
  %51 = fadd float %50, 1.000000e+00
  %52 = fadd float %51, 1.000000e+00
  %53 = fdiv float 1.000000e+00, %52
  %54 = tail call float @llvm.fmuladd.f32(float %45, float %53, float %39)
  %55 = tail call float @llvm.fmuladd.f32(float %48, float %53, float %40)
  %56 = icmp eq i64 %16, 12
  br i1 %56, label %74, label %57

57:
  %58 = phi float [ %55, %42 ], [ %40, %15 ]
  %59 = phi float [ %54, %42 ], [ %39, %15 ]
  %60 = getelementptr i8, ptr @pos_x, i64 %16
  %61 = load float, ptr %60, align 4, !tbaa !15
  %62 = fsub float %183, %61
  %63 = getelementptr i8, ptr @pos_y, i64 %16
  %64 = load float, ptr %63, align 4, !tbaa !15
  %65 = fsub float %182, %64
  %66 = fmul float %65, %65
  %67 = tail call float @llvm.fmuladd.f32(float %62, float %62, float %66)
  %68 = fadd float %67, 1.000000e+00
  %69 = fadd float %68, 1.000000e+00
  %70 = fdiv float 1.000000e+00, %69
  %71 = tail call float @llvm.fmuladd.f32(float %62, float %70, float %59)
  %72 = tail call float @llvm.fmuladd.f32(float %65, float %70, float %58)
  %73 = icmp eq i64 %16, 16
  br i1 %73, label %91, label %74

74:
  %75 = phi float [ %72, %57 ], [ %55, %42 ]
  %76 = phi float [ %71, %57 ], [ %54, %42 ]
  %77 = getelementptr i8, ptr @pos_x, i64 %16
  %78 = load float, ptr %77, align 4, !tbaa !15
  %79 = fsub float %181, %78
  %80 = getelementptr i8, ptr @pos_y, i64 %16
  %81 = load float, ptr %80, align 4, !tbaa !15
  %82 = fsub float %180, %81
  %83 = fmul float %82, %82
  %84 = tail call float @llvm.fmuladd.f32(float %79, float %79, float %83)
  %85 = fadd float %84, 1.000000e+00
  %86 = fadd float %85, 1.000000e+00
  %87 = fdiv float 1.000000e+00, %86
  %88 = tail call float @llvm.fmuladd.f32(float %79, float %87, float %76)
  %89 = tail call float @llvm.fmuladd.f32(float %82, float %87, float %75)
  %90 = icmp eq i64 %16, 20
  br i1 %90, label %107, label %91

91:
  %92 = phi float [ %89, %74 ], [ %72, %57 ]
  %93 = phi float [ %88, %74 ], [ %71, %57 ]
  %94 = getelementptr i8, ptr @pos_x, i64 %16
  %95 = load float, ptr %94, align 4, !tbaa !15
  %96 = fsub float %179, %95
  %97 = getelementptr i8, ptr @pos_y, i64 %16
  %98 = load float, ptr %97, align 4, !tbaa !15
  %99 = fsub float %178, %98
  %100 = fmul float %99, %99
  %101 = tail call float @llvm.fmuladd.f32(float %96, float %96, float %100)
  %102 = fadd float %101, 1.000000e+00
  %103 = fadd float %102, 1.000000e+00
  %104 = fdiv float 1.000000e+00, %103
  %105 = tail call float @llvm.fmuladd.f32(float %96, float %104, float %93)
  %106 = tail call float @llvm.fmuladd.f32(float %99, float %104, float %92)
  br label %107

107:
  %108 = phi float [ %105, %91 ], [ %88, %74 ]
  %109 = phi float [ %106, %91 ], [ %89, %74 ]
  %110 = getelementptr i8, ptr @vel_x, i64 %16
  %111 = load float, ptr %110, align 4, !tbaa !15
  %112 = fadd float %108, %111
  %113 = getelementptr i8, ptr @vel_y, i64 %16
  %114 = load float, ptr %113, align 4, !tbaa !15
  %115 = fadd float %109, %114
  %116 = fmul float %112, 1.250000e-01
  %117 = fsub float %112, %116
  store float %117, ptr %110, align 4, !tbaa !15
  %118 = fmul float %115, 1.250000e-01
  %119 = fsub float %115, %118
  store float %119, ptr %113, align 4, !tbaa !15
  %120 = add nuw nsw i64 %16, 4
  %exitcond2 = icmp eq i64 %120, 24
  br i1 %exitcond2, label %121, label %15, !llvm.loop !20

121:
  %122 = fadd float %189, %239
  %123 = fadd float %188, %241
  %124 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_x, i64 4), align 4, !tbaa !15
  %125 = fadd float %187, %124
  store float %125, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 4), align 4, !tbaa !15
  %126 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_y, i64 4), align 4, !tbaa !15
  %127 = fadd float %186, %126
  store float %127, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 4), align 4, !tbaa !15
  %128 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_x, i64 8), align 4, !tbaa !15
  %129 = fadd float %185, %128
  store float %129, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 8), align 4, !tbaa !15
  %130 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_y, i64 8), align 4, !tbaa !15
  %131 = fadd float %184, %130
  store float %131, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 8), align 4, !tbaa !15
  %132 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_x, i64 12), align 4, !tbaa !15
  %133 = fadd float %183, %132
  store float %133, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 12), align 4, !tbaa !15
  %134 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_y, i64 12), align 4, !tbaa !15
  %135 = fadd float %182, %134
  store float %135, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 12), align 4, !tbaa !15
  %136 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_x, i64 16), align 4, !tbaa !15
  %137 = fadd float %181, %136
  store float %137, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 16), align 4, !tbaa !15
  %138 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_y, i64 16), align 4, !tbaa !15
  %139 = fadd float %180, %138
  store float %139, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 16), align 4, !tbaa !15
  %140 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_x, i64 20), align 4, !tbaa !15
  %141 = fadd float %179, %140
  store float %141, ptr getelementptr inbounds nuw (i8, ptr @pos_x, i64 20), align 4, !tbaa !15
  %142 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_y, i64 20), align 4, !tbaa !15
  %143 = fadd float %178, %142
  store float %143, ptr getelementptr inbounds nuw (i8, ptr @pos_y, i64 20), align 4, !tbaa !15
  %144 = add nuw nsw i32 %190, 1
  %exitcond3 = icmp eq i32 %144, 1000
  br i1 %exitcond3, label %145, label %173, !llvm.loop !22

145:
  store float %122, ptr @pos_x, align 4, !tbaa !15
  store float %123, ptr @pos_y, align 4, !tbaa !15
  store float %239, ptr @vel_x, align 4, !tbaa !15
  store float %241, ptr @vel_y, align 4, !tbaa !15
  %146 = getelementptr i8, ptr @pos_x, i64 24
  %147 = getelementptr i8, ptr @pos_y, i64 24
  %148 = getelementptr i8, ptr @vel_x, i64 24
  %149 = getelementptr i8, ptr @vel_y, i64 24
  br label %150

150:
  %151 = phi i64 [ %math, %150 ], [ -6, %145 ]
  %152 = phi float [ %168, %150 ], [ 0.000000e+00, %145 ]
  %153 = add i64 %151, 7
  %tmp = trunc i64 %153 to i32
  %154 = uitofp nneg i32 %tmp to float
  %155 = shl nsw i64 %151, 2
  %156 = getelementptr i8, ptr %146, i64 %155
  %157 = load float, ptr %156, align 4, !tbaa !15
  %158 = getelementptr i8, ptr %147, i64 %155
  %159 = load float, ptr %158, align 4, !tbaa !15
  %160 = tail call float @llvm.fmuladd.f32(float 3.000000e+00, float %159, float %157)
  %161 = shl nsw i64 %151, 2
  %162 = getelementptr i8, ptr %148, i64 %161
  %163 = load float, ptr %162, align 4, !tbaa !15
  %164 = tail call float @llvm.fmuladd.f32(float 5.000000e+00, float %163, float %160)
  %165 = getelementptr i8, ptr %149, i64 %161
  %166 = load float, ptr %165, align 4, !tbaa !15
  %167 = tail call float @llvm.fmuladd.f32(float 7.000000e+00, float %166, float %164)
  %168 = tail call float @llvm.fmuladd.f32(float %154, float %167, float %152)
  %169 = call { i64, i1 } @llvm.uadd.with.overflow.i64(i64 %151, i64 1)
  %math = extractvalue { i64, i1 } %169, 0
  %ov = extractvalue { i64, i1 } %169, 1
  br i1 %ov, label %170, label %150, !llvm.loop !23

170:
  %171 = tail call float @llvm.fmuladd.f32(float %168, float 1.024000e+03, float 5.000000e-01)
  %172 = fptosi float %171 to i64
  ret i64 %172

173:
  %174 = phi float [ %127, %121 ], [ %promoted15, %14 ]
  %175 = phi float [ %125, %121 ], [ %promoted14, %14 ]
  %176 = phi float [ %241, %121 ], [ %promoted13, %14 ]
  %177 = phi float [ %239, %121 ], [ %promoted12, %14 ]
  %178 = phi float [ %143, %121 ], [ %promoted11, %14 ]
  %179 = phi float [ %141, %121 ], [ %promoted10, %14 ]
  %180 = phi float [ %139, %121 ], [ %promoted9, %14 ]
  %181 = phi float [ %137, %121 ], [ %promoted8, %14 ]
  %182 = phi float [ %135, %121 ], [ %promoted7, %14 ]
  %183 = phi float [ %133, %121 ], [ %promoted6, %14 ]
  %184 = phi float [ %131, %121 ], [ %promoted5, %14 ]
  %185 = phi float [ %129, %121 ], [ %promoted4, %14 ]
  %186 = phi float [ %127, %121 ], [ %promoted3, %14 ]
  %187 = phi float [ %125, %121 ], [ %promoted2, %14 ]
  %188 = phi float [ %123, %121 ], [ %promoted1, %14 ]
  %189 = phi float [ %122, %121 ], [ %promoted, %14 ]
  %190 = phi i32 [ %144, %121 ], [ 0, %14 ]
  %191 = fsub float %187, %189
  %192 = fsub float %186, %188
  %193 = fmul float %192, %192
  %194 = tail call float @llvm.fmuladd.f32(float %191, float %191, float %193)
  %195 = fadd float %194, 1.000000e+00
  %196 = fadd float %195, 1.000000e+00
  %197 = fdiv float 1.000000e+00, %196
  %198 = tail call float @llvm.fmuladd.f32(float %191, float %197, float 0.000000e+00)
  %199 = tail call float @llvm.fmuladd.f32(float %192, float %197, float 0.000000e+00)
  %200 = fsub float %185, %189
  %201 = fsub float %184, %188
  %202 = fmul float %201, %201
  %203 = tail call float @llvm.fmuladd.f32(float %200, float %200, float %202)
  %204 = fadd float %203, 1.000000e+00
  %205 = fadd float %204, 1.000000e+00
  %206 = fdiv float 1.000000e+00, %205
  %207 = tail call float @llvm.fmuladd.f32(float %200, float %206, float %198)
  %208 = tail call float @llvm.fmuladd.f32(float %201, float %206, float %199)
  %209 = fsub float %183, %189
  %210 = fsub float %182, %188
  %211 = fmul float %210, %210
  %212 = tail call float @llvm.fmuladd.f32(float %209, float %209, float %211)
  %213 = fadd float %212, 1.000000e+00
  %214 = fadd float %213, 1.000000e+00
  %215 = fdiv float 1.000000e+00, %214
  %216 = tail call float @llvm.fmuladd.f32(float %209, float %215, float %207)
  %217 = tail call float @llvm.fmuladd.f32(float %210, float %215, float %208)
  %218 = fsub float %181, %189
  %219 = fsub float %180, %188
  %220 = fmul float %219, %219
  %221 = tail call float @llvm.fmuladd.f32(float %218, float %218, float %220)
  %222 = fadd float %221, 1.000000e+00
  %223 = fadd float %222, 1.000000e+00
  %224 = fdiv float 1.000000e+00, %223
  %225 = tail call float @llvm.fmuladd.f32(float %218, float %224, float %216)
  %226 = tail call float @llvm.fmuladd.f32(float %219, float %224, float %217)
  %227 = fsub float %178, %188
  %228 = fsub float %179, %189
  %229 = fmul float %227, %227
  %230 = tail call float @llvm.fmuladd.f32(float %228, float %228, float %229)
  %231 = fadd float %230, 1.000000e+00
  %232 = fadd float %231, 1.000000e+00
  %233 = fdiv float 1.000000e+00, %232
  %234 = tail call float @llvm.fmuladd.f32(float %227, float %233, float %226)
  %235 = tail call float @llvm.fmuladd.f32(float %228, float %233, float %225)
  %236 = fadd float %235, %177
  %237 = fadd float %234, %176
  %238 = fmul float %236, 1.250000e-01
  %239 = fsub float %236, %238
  %240 = fmul float %237, 1.250000e-01
  %241 = fsub float %237, %240
  %242 = fsub float %189, %175
  %243 = fsub float %188, %174
  %244 = fmul float %243, %243
  %245 = tail call float @llvm.fmuladd.f32(float %242, float %242, float %244)
  %246 = fadd float %245, 1.000000e+00
  %247 = fadd float %246, 1.000000e+00
  %248 = fdiv float 1.000000e+00, %247
  %249 = tail call float @llvm.fmuladd.f32(float %242, float %248, float 0.000000e+00)
  %250 = tail call float @llvm.fmuladd.f32(float %243, float %248, float 0.000000e+00)
  %251 = fsub float %185, %175
  %252 = fsub float %184, %174
  %253 = fmul float %252, %252
  %254 = tail call float @llvm.fmuladd.f32(float %251, float %251, float %253)
  %255 = fadd float %254, 1.000000e+00
  %256 = fadd float %255, 1.000000e+00
  %257 = fdiv float 1.000000e+00, %256
  %258 = tail call float @llvm.fmuladd.f32(float %251, float %257, float %249)
  %259 = tail call float @llvm.fmuladd.f32(float %252, float %257, float %250)
  %260 = fsub float %183, %175
  %261 = fsub float %182, %174
  %262 = fmul float %261, %261
  %263 = tail call float @llvm.fmuladd.f32(float %260, float %260, float %262)
  %264 = fadd float %263, 1.000000e+00
  %265 = fadd float %264, 1.000000e+00
  %266 = fdiv float 1.000000e+00, %265
  %267 = tail call float @llvm.fmuladd.f32(float %260, float %266, float %258)
  %268 = tail call float @llvm.fmuladd.f32(float %261, float %266, float %259)
  %269 = fsub float %181, %175
  %270 = fsub float %180, %174
  %271 = fmul float %270, %270
  %272 = tail call float @llvm.fmuladd.f32(float %269, float %269, float %271)
  %273 = fadd float %272, 1.000000e+00
  %274 = fadd float %273, 1.000000e+00
  %275 = fdiv float 1.000000e+00, %274
  %276 = tail call float @llvm.fmuladd.f32(float %269, float %275, float %267)
  %277 = tail call float @llvm.fmuladd.f32(float %270, float %275, float %268)
  %278 = fsub float %179, %175
  %279 = fsub float %178, %174
  %280 = fmul float %279, %279
  %281 = tail call float @llvm.fmuladd.f32(float %278, float %278, float %280)
  %282 = fadd float %281, 1.000000e+00
  %283 = fadd float %282, 1.000000e+00
  %284 = fdiv float 1.000000e+00, %283
  %285 = tail call float @llvm.fmuladd.f32(float %278, float %284, float %276)
  %286 = tail call float @llvm.fmuladd.f32(float %279, float %284, float %277)
  %287 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_x, i64 4), align 4, !tbaa !15
  %288 = fadd float %285, %287
  %289 = load float, ptr getelementptr inbounds nuw (i8, ptr @vel_y, i64 4), align 4, !tbaa !15
  %290 = fadd float %286, %289
  %291 = fmul float %288, 1.250000e-01
  %292 = fsub float %288, %291
  store float %292, ptr getelementptr inbounds nuw (i8, ptr @vel_x, i64 4), align 4, !tbaa !15
  %293 = fmul float %290, 1.250000e-01
  %294 = fsub float %290, %293
  store float %294, ptr getelementptr inbounds nuw (i8, ptr @vel_y, i64 4), align 4, !tbaa !15
  br label %15
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare float @llvm.fmuladd.f32(float, float, float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %1 = tail call i64 @bench_nbody_single() nobuiltin "no-builtins"
  tail call void @report(i64 noundef %1) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i64 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"

declare { i64, i1 } @llvm.uadd.with.overflow.i64(i64, i64) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7}
!llvm.ident = !{!8}
!llvm.errno.tbaa = !{!9}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!9 = !{!10, !11, i64 0}
!10 = !{!"__libc_errno", !11, i64 0}
!11 = !{!"int", !12, i64 0}
!12 = !{!"omnipotent char", !13, i64 0}
!13 = !{!"Simple C/C++ TBAA"}
!14 = !{!11, !11, i64 0}
!15 = !{!16, !16, i64 0}
!16 = !{!"float", !12, i64 0}
!17 = distinct !{!17, !18}
!18 = !{!"llvm.loop.mustprogress"}
!19 = distinct !{!19, !18}
!20 = distinct !{!20, !18}
!21 = distinct !{!21, !18}
!22 = distinct !{!22, !18}
!23 = distinct !{!23, !18}
