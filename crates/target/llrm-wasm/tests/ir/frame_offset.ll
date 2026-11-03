target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

@__main_void = hidden alias i32 (), ptr @main

define hidden i32 @bench_fpbench(i16 noundef signext %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" memory(none) nofree nosync norecurse {
  %2 = alloca [6 x float], align 16
  %3 = alloca [6 x float], align 16
  %4 = alloca [6 x float], align 16
  %5 = alloca [6 x float], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %3) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %4) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %5) nounwind
  store float -1.500000e+01, ptr %2, align 16, !tbaa !8
  store float -1.200000e+01, ptr %3, align 16, !tbaa !8
  store float 0.000000e+00, ptr %4, align 16, !tbaa !8
  store float 0.000000e+00, ptr %5, align 16, !tbaa !8
  %6 = getelementptr inbounds nuw i8, ptr %2, i32 4
  store float -8.000000e+00, ptr %6, align 4, !tbaa !8
  %7 = getelementptr inbounds nuw i8, ptr %3, i32 4
  store float -7.000000e+00, ptr %7, align 4, !tbaa !8
  %8 = getelementptr inbounds nuw i8, ptr %4, i32 4
  store float 0.000000e+00, ptr %8, align 4, !tbaa !8
  %9 = getelementptr inbounds nuw i8, ptr %5, i32 4
  store float 0.000000e+00, ptr %9, align 4, !tbaa !8
  %10 = getelementptr inbounds nuw i8, ptr %2, i32 8
  store float -1.000000e+00, ptr %10, align 8, !tbaa !8
  %11 = getelementptr inbounds nuw i8, ptr %3, i32 8
  store float -2.000000e+00, ptr %11, align 8, !tbaa !8
  %12 = getelementptr inbounds nuw i8, ptr %4, i32 8
  store float 0.000000e+00, ptr %12, align 8, !tbaa !8
  %13 = getelementptr inbounds nuw i8, ptr %5, i32 8
  store float 0.000000e+00, ptr %13, align 8, !tbaa !8
  %14 = getelementptr inbounds nuw i8, ptr %2, i32 12
  store float 6.000000e+00, ptr %14, align 4, !tbaa !8
  %15 = getelementptr inbounds nuw i8, ptr %3, i32 12
  store float 3.000000e+00, ptr %15, align 4, !tbaa !8
  %16 = getelementptr inbounds nuw i8, ptr %4, i32 12
  store float 0.000000e+00, ptr %16, align 4, !tbaa !8
  %17 = getelementptr inbounds nuw i8, ptr %5, i32 12
  store float 0.000000e+00, ptr %17, align 4, !tbaa !8
  %18 = getelementptr inbounds nuw i8, ptr %2, i32 16
  store float 1.300000e+01, ptr %18, align 16, !tbaa !8
  %19 = getelementptr inbounds nuw i8, ptr %3, i32 16
  store float 8.000000e+00, ptr %19, align 16, !tbaa !8
  %20 = getelementptr inbounds nuw i8, ptr %4, i32 16
  store float 0.000000e+00, ptr %20, align 16, !tbaa !8
  %21 = getelementptr inbounds nuw i8, ptr %5, i32 16
  store float 0.000000e+00, ptr %21, align 16, !tbaa !8
  %22 = getelementptr inbounds nuw i8, ptr %2, i32 20
  store float 2.000000e+01, ptr %22, align 4, !tbaa !8
  %23 = getelementptr inbounds nuw i8, ptr %3, i32 20
  store float 1.300000e+01, ptr %23, align 4, !tbaa !8
  %24 = getelementptr inbounds nuw i8, ptr %4, i32 20
  store float 0.000000e+00, ptr %24, align 4, !tbaa !8
  %25 = getelementptr inbounds nuw i8, ptr %5, i32 20
  store float 0.000000e+00, ptr %25, align 4, !tbaa !8
  %26 = icmp slt i16 %0, 1
  br i1 %26, label %42, label %27

27:
  br label %28

28:
  %29 = phi float [ %240, %201 ], [ 2.000000e+01, %27 ]
  %30 = phi float [ %232, %201 ], [ 1.300000e+01, %27 ]
  %31 = phi float [ %224, %201 ], [ 6.000000e+00, %27 ]
  %32 = phi float [ %216, %201 ], [ -1.000000e+00, %27 ]
  %33 = phi float [ %208, %201 ], [ -8.000000e+00, %27 ]
  %34 = phi float [ %203, %201 ], [ -1.500000e+01, %27 ]
  %35 = phi float [ %244, %201 ], [ 1.300000e+01, %27 ]
  %36 = phi float [ %236, %201 ], [ 8.000000e+00, %27 ]
  %37 = phi float [ %228, %201 ], [ 3.000000e+00, %27 ]
  %38 = phi float [ %220, %201 ], [ -2.000000e+00, %27 ]
  %39 = phi float [ %212, %201 ], [ -7.000000e+00, %27 ]
  %40 = phi float [ %205, %201 ], [ -1.200000e+01, %27 ]
  %41 = phi i16 [ %246, %201 ], [ 1, %27 ]
  br label %101

42:
  %43 = phi float [ 2.000000e+01, %1 ], [ %240, %201 ]
  %44 = phi float [ 1.300000e+01, %1 ], [ %232, %201 ]
  %45 = phi float [ 6.000000e+00, %1 ], [ %224, %201 ]
  %46 = phi float [ -1.000000e+00, %1 ], [ %216, %201 ]
  %47 = phi float [ -8.000000e+00, %1 ], [ %208, %201 ]
  %48 = phi float [ -1.500000e+01, %1 ], [ %203, %201 ]
  %49 = phi float [ 1.300000e+01, %1 ], [ %244, %201 ]
  %50 = phi float [ 8.000000e+00, %1 ], [ %236, %201 ]
  %51 = phi float [ 3.000000e+00, %1 ], [ %228, %201 ]
  %52 = phi float [ -2.000000e+00, %1 ], [ %220, %201 ]
  %53 = phi float [ -7.000000e+00, %1 ], [ %212, %201 ]
  %54 = phi float [ -1.200000e+01, %1 ], [ %205, %201 ]
  %55 = fmul float %48, 6.400000e+01
  %56 = fptosi float %55 to i32
  %57 = fmul float %54, 6.400000e+01
  %58 = fptosi float %57 to i32
  %59 = mul nsw i32 %58, 7
  %60 = add i32 %59, %56
  %61 = fmul float %47, 6.400000e+01
  %62 = fptosi float %61 to i32
  %63 = shl nsw i32 %62, 1
  %64 = fmul float %53, 6.400000e+01
  %65 = fptosi float %64 to i32
  %66 = shl nsw i32 %65, 3
  %67 = add i32 %63, %60
  %68 = add i32 %67, %66
  %69 = fmul float %46, 6.400000e+01
  %70 = fptosi float %69 to i32
  %71 = mul nsw i32 %70, 3
  %72 = fmul float %52, 6.400000e+01
  %73 = fptosi float %72 to i32
  %74 = mul nsw i32 %73, 9
  %75 = add i32 %71, %68
  %76 = add i32 %75, %74
  %77 = fmul float %45, 6.400000e+01
  %78 = fptosi float %77 to i32
  %79 = shl nsw i32 %78, 2
  %80 = fmul float %51, 6.400000e+01
  %81 = fptosi float %80 to i32
  %82 = mul nsw i32 %81, 10
  %83 = add i32 %79, %76
  %84 = add i32 %83, %82
  %85 = fmul float %44, 6.400000e+01
  %86 = fptosi float %85 to i32
  %87 = mul nsw i32 %86, 5
  %88 = fmul float %50, 6.400000e+01
  %89 = fptosi float %88 to i32
  %90 = mul nsw i32 %89, 11
  %91 = add i32 %87, %84
  %92 = add i32 %91, %90
  %93 = fmul float %43, 6.400000e+01
  %94 = fptosi float %93 to i32
  %95 = mul nsw i32 %94, 6
  %96 = fmul float %49, 6.400000e+01
  %97 = fptosi float %96 to i32
  %98 = mul nsw i32 %97, 12
  %99 = add i32 %95, %92
  %100 = add i32 %99, %98
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %5) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  ret i32 %100

101:
  %102 = phi i32 [ %200, %187 ], [ 0, %28 ]
  %103 = getelementptr i8, ptr %2, i32 %102
  %104 = getelementptr i8, ptr %3, i32 %102
  %105 = icmp eq i32 %102, 0
  br i1 %105, label %118, label %106

106:
  %107 = load float, ptr %103, align 4, !tbaa !8
  %108 = fsub float %34, %107
  %109 = load float, ptr %104, align 4, !tbaa !8
  %110 = fsub float %40, %109
  %111 = fmul float %110, %110
  %112 = tail call float @llvm.fmuladd.f32(float %108, float %108, float %111)
  %113 = fadd float %112, 1.000000e+00
  %114 = fdiv float 1.000000e+00, %113
  %115 = tail call float @llvm.fmuladd.f32(float %108, float %114, float 0.000000e+00)
  %116 = tail call float @llvm.fmuladd.f32(float %110, float %114, float 0.000000e+00)
  %117 = icmp eq i32 4, %102
  br i1 %117, label %132, label %118

118:
  %119 = phi float [ %116, %106 ], [ 0.000000e+00, %101 ]
  %120 = phi float [ %115, %106 ], [ 0.000000e+00, %101 ]
  %121 = load float, ptr %103, align 4, !tbaa !8
  %122 = fsub float %33, %121
  %123 = load float, ptr %104, align 4, !tbaa !8
  %124 = fsub float %39, %123
  %125 = fmul float %124, %124
  %126 = tail call float @llvm.fmuladd.f32(float %122, float %122, float %125)
  %127 = fadd float %126, 1.000000e+00
  %128 = fdiv float 1.000000e+00, %127
  %129 = tail call float @llvm.fmuladd.f32(float %122, float %128, float %120)
  %130 = tail call float @llvm.fmuladd.f32(float %124, float %128, float %119)
  %131 = icmp eq i32 8, %102
  br i1 %131, label %146, label %132

132:
  %133 = phi float [ %130, %118 ], [ %116, %106 ]
  %134 = phi float [ %129, %118 ], [ %115, %106 ]
  %135 = load float, ptr %103, align 4, !tbaa !8
  %136 = fsub float %32, %135
  %137 = load float, ptr %104, align 4, !tbaa !8
  %138 = fsub float %38, %137
  %139 = fmul float %138, %138
  %140 = tail call float @llvm.fmuladd.f32(float %136, float %136, float %139)
  %141 = fadd float %140, 1.000000e+00
  %142 = fdiv float 1.000000e+00, %141
  %143 = tail call float @llvm.fmuladd.f32(float %136, float %142, float %134)
  %144 = tail call float @llvm.fmuladd.f32(float %138, float %142, float %133)
  %145 = icmp eq i32 12, %102
  br i1 %145, label %160, label %146

146:
  %147 = phi float [ %144, %132 ], [ %130, %118 ]
  %148 = phi float [ %143, %132 ], [ %129, %118 ]
  %149 = load float, ptr %103, align 4, !tbaa !8
  %150 = fsub float %31, %149
  %151 = load float, ptr %104, align 4, !tbaa !8
  %152 = fsub float %37, %151
  %153 = fmul float %152, %152
  %154 = tail call float @llvm.fmuladd.f32(float %150, float %150, float %153)
  %155 = fadd float %154, 1.000000e+00
  %156 = fdiv float 1.000000e+00, %155
  %157 = tail call float @llvm.fmuladd.f32(float %150, float %156, float %148)
  %158 = tail call float @llvm.fmuladd.f32(float %152, float %156, float %147)
  %159 = icmp eq i32 16, %102
  br i1 %159, label %174, label %160

160:
  %161 = phi float [ %158, %146 ], [ %144, %132 ]
  %162 = phi float [ %157, %146 ], [ %143, %132 ]
  %163 = load float, ptr %103, align 4, !tbaa !8
  %164 = fsub float %30, %163
  %165 = load float, ptr %104, align 4, !tbaa !8
  %166 = fsub float %36, %165
  %167 = fmul float %166, %166
  %168 = tail call float @llvm.fmuladd.f32(float %164, float %164, float %167)
  %169 = fadd float %168, 1.000000e+00
  %170 = fdiv float 1.000000e+00, %169
  %171 = tail call float @llvm.fmuladd.f32(float %164, float %170, float %162)
  %172 = tail call float @llvm.fmuladd.f32(float %166, float %170, float %161)
  %173 = icmp eq i32 20, %102
  br i1 %173, label %187, label %174

174:
  %175 = phi float [ %172, %160 ], [ %158, %146 ]
  %176 = phi float [ %171, %160 ], [ %157, %146 ]
  %177 = load float, ptr %103, align 4, !tbaa !8
  %178 = fsub float %29, %177
  %179 = load float, ptr %104, align 4, !tbaa !8
  %180 = fsub float %35, %179
  %181 = fmul float %180, %180
  %182 = tail call float @llvm.fmuladd.f32(float %178, float %178, float %181)
  %183 = fadd float %182, 1.000000e+00
  %184 = fdiv float 1.000000e+00, %183
  %185 = tail call float @llvm.fmuladd.f32(float %178, float %184, float %176)
  %186 = tail call float @llvm.fmuladd.f32(float %180, float %184, float %175)
  br label %187

187:
  %188 = phi float [ %185, %174 ], [ %171, %160 ]
  %189 = phi float [ %186, %174 ], [ %172, %160 ]
  %190 = getelementptr i8, ptr %4, i32 %102
  %191 = load float, ptr %190, align 4, !tbaa !8
  %192 = fadd float %188, %191
  %193 = getelementptr i8, ptr %5, i32 %102
  %194 = load float, ptr %193, align 4, !tbaa !8
  %195 = fadd float %189, %194
  %196 = fmul float %192, 6.250000e-02
  %197 = fsub float %192, %196
  store float %197, ptr %190, align 4, !tbaa !8
  %198 = fmul float %195, 6.250000e-02
  %199 = fsub float %195, %198
  store float %199, ptr %193, align 4, !tbaa !8
  %200 = add nuw nsw i32 %102, 4
  %exitcond3 = icmp eq i32 24, %200
  br i1 %exitcond3, label %201, label %101, !llvm.loop !13

201:
  %202 = load float, ptr %4, align 16, !tbaa !8
  %203 = fadd float %34, %202
  store float %203, ptr %2, align 16, !tbaa !8
  %204 = load float, ptr %5, align 16, !tbaa !8
  %205 = fadd float %40, %204
  store float %205, ptr %3, align 16, !tbaa !8
  %206 = getelementptr i8, ptr %4, i32 4
  %207 = load float, ptr %206, align 4, !tbaa !8
  %208 = fadd float %33, %207
  %209 = getelementptr i8, ptr %2, i32 4
  store float %208, ptr %209, align 4, !tbaa !8
  %210 = getelementptr i8, ptr %5, i32 4
  %211 = load float, ptr %210, align 4, !tbaa !8
  %212 = fadd float %39, %211
  %213 = getelementptr i8, ptr %3, i32 4
  store float %212, ptr %213, align 4, !tbaa !8
  %214 = getelementptr i8, ptr %4, i32 8
  %215 = load float, ptr %214, align 8, !tbaa !8
  %216 = fadd float %32, %215
  %217 = getelementptr i8, ptr %2, i32 8
  store float %216, ptr %217, align 8, !tbaa !8
  %218 = getelementptr i8, ptr %5, i32 8
  %219 = load float, ptr %218, align 8, !tbaa !8
  %220 = fadd float %38, %219
  %221 = getelementptr i8, ptr %3, i32 8
  store float %220, ptr %221, align 8, !tbaa !8
  %222 = getelementptr i8, ptr %4, i32 12
  %223 = load float, ptr %222, align 4, !tbaa !8
  %224 = fadd float %31, %223
  %225 = getelementptr i8, ptr %2, i32 12
  store float %224, ptr %225, align 4, !tbaa !8
  %226 = getelementptr i8, ptr %5, i32 12
  %227 = load float, ptr %226, align 4, !tbaa !8
  %228 = fadd float %37, %227
  %229 = getelementptr i8, ptr %3, i32 12
  store float %228, ptr %229, align 4, !tbaa !8
  %230 = getelementptr i8, ptr %4, i32 16
  %231 = load float, ptr %230, align 16, !tbaa !8
  %232 = fadd float %30, %231
  %233 = getelementptr i8, ptr %2, i32 16
  store float %232, ptr %233, align 16, !tbaa !8
  %234 = getelementptr i8, ptr %5, i32 16
  %235 = load float, ptr %234, align 16, !tbaa !8
  %236 = fadd float %36, %235
  %237 = getelementptr i8, ptr %3, i32 16
  store float %236, ptr %237, align 16, !tbaa !8
  %238 = getelementptr i8, ptr %4, i32 20
  %239 = load float, ptr %238, align 4, !tbaa !8
  %240 = fadd float %29, %239
  %241 = getelementptr i8, ptr %2, i32 20
  store float %240, ptr %241, align 4, !tbaa !8
  %242 = getelementptr i8, ptr %5, i32 20
  %243 = load float, ptr %242, align 4, !tbaa !8
  %244 = fadd float %35, %243
  %245 = getelementptr i8, ptr %3, i32 20
  store float %244, ptr %245, align 4, !tbaa !8
  %246 = add nuw nsw i16 %41, 1
  %247 = icmp slt i16 %41, %0
  br i1 %247, label %28, label %42, !llvm.loop !15
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare float @llvm.fmuladd.f32(float, float, float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define hidden i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" {
  %1 = tail call i32 @bench_fpbench(i16 noundef signext 20) nobuiltin "no-builtins"
  tail call void @report(i32 noundef %1) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext"

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!7, !7, i64 0}
!7 = !{!"short", !4, i64 0}
!8 = !{!9, !9, i64 0}
!9 = !{!"float", !4, i64 0}
!10 = distinct !{!10, !11}
!11 = !{!"llvm.loop.mustprogress"}
!12 = distinct !{!12, !11}
!13 = distinct !{!13, !11}
!14 = distinct !{!14, !11}
!15 = distinct !{!15, !11}
!16 = !{!17, !17, i64 0}
!17 = !{!"long", !4, i64 0}
!18 = distinct !{!18, !11}

