target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@opq0 = internal global i32 0, align 4

define i64 @bench_fpbench(i16 noundef %0) noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %2 = alloca i16, align 2
  %3 = alloca [6 x float], align 4
  %4 = alloca [6 x float], align 4
  %5 = alloca [6 x float], align 4
  %6 = alloca [6 x float], align 4
  %7 = alloca float, align 4
  %8 = alloca float, align 4
  %9 = alloca float, align 4
  %10 = alloca float, align 4
  %11 = alloca float, align 4
  %12 = alloca float, align 4
  %13 = alloca i64, align 8
  %14 = alloca i16, align 2
  %15 = alloca i16, align 2
  %16 = alloca i16, align 2
  store i16 %0, ptr %2, align 2, !tbaa !14
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %6) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %7) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %8) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %9) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %10) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %11) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %12) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %13) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %14) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %15) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %16) nounwind
  store i16 0, ptr %15, align 2, !tbaa !14
  br label %17

17:
  %18 = load i16, ptr %15, align 2, !tbaa !14
  %19 = sext i16 %18 to i32
  %20 = icmp slt i32 %19, 6
  br i1 %20, label %21, label %47

21:
  %22 = load i16, ptr %15, align 2, !tbaa !14
  %23 = sext i16 %22 to i32
  %24 = mul nsw i32 %23, 7
  %25 = sub nsw i32 %24, 15
  %26 = sitofp i32 %25 to float
  %27 = load i16, ptr %15, align 2, !tbaa !14
  %28 = sext i16 %27 to i64
  %29 = getelementptr inbounds [6 x float], ptr %3, i64 0, i64 %28
  store float %26, ptr %29, align 4, !tbaa !16
  %30 = load i16, ptr %15, align 2, !tbaa !14
  %31 = sext i16 %30 to i32
  %32 = mul nsw i32 %31, 5
  %33 = sub nsw i32 %32, 12
  %34 = sitofp i32 %33 to float
  %35 = load i16, ptr %15, align 2, !tbaa !14
  %36 = sext i16 %35 to i64
  %37 = getelementptr inbounds [6 x float], ptr %4, i64 0, i64 %36
  store float %34, ptr %37, align 4, !tbaa !16
  %38 = load i16, ptr %15, align 2, !tbaa !14
  %39 = sext i16 %38 to i64
  %40 = getelementptr inbounds [6 x float], ptr %5, i64 0, i64 %39
  store float 0.000000e+00, ptr %40, align 4, !tbaa !16
  %41 = load i16, ptr %15, align 2, !tbaa !14
  %42 = sext i16 %41 to i64
  %43 = getelementptr inbounds [6 x float], ptr %6, i64 0, i64 %42
  store float 0.000000e+00, ptr %43, align 4, !tbaa !16
  br label %44

44:
  %45 = load i16, ptr %15, align 2, !tbaa !14
  %46 = add i16 %45, 1
  store i16 %46, ptr %15, align 2, !tbaa !14
  br label %17, !llvm.loop !18

47:
  store i16 1, ptr %14, align 2, !tbaa !14
  br label %48

48:
  %49 = load i16, ptr %14, align 2, !tbaa !14
  %50 = sext i16 %49 to i32
  %51 = load i16, ptr %2, align 2, !tbaa !14
  %52 = sext i16 %51 to i32
  %53 = icmp sle i32 %50, %52
  br i1 %53, label %54, label %195

54:
  store i16 0, ptr %15, align 2, !tbaa !14
  br label %55

55:
  %56 = load i16, ptr %15, align 2, !tbaa !14
  %57 = sext i16 %56 to i32
  %58 = icmp slt i32 %57, 6
  br i1 %58, label %59, label %158

59:
  store float 0.000000e+00, ptr %11, align 4, !tbaa !16
  store float 0.000000e+00, ptr %12, align 4, !tbaa !16
  store i16 0, ptr %16, align 2, !tbaa !14
  br label %60

60:
  %61 = load i16, ptr %16, align 2, !tbaa !14
  %62 = sext i16 %61 to i32
  %63 = icmp slt i32 %62, 6
  br i1 %63, label %64, label %110

64:
  %65 = load i16, ptr %16, align 2, !tbaa !14
  %66 = sext i16 %65 to i32
  %67 = load i16, ptr %15, align 2, !tbaa !14
  %68 = sext i16 %67 to i32
  %69 = icmp ne i32 %66, %68
  br i1 %69, label %70, label %106

70:
  %71 = load i16, ptr %16, align 2, !tbaa !14
  %72 = sext i16 %71 to i64
  %73 = getelementptr inbounds [6 x float], ptr %3, i64 0, i64 %72
  %74 = load float, ptr %73, align 4, !tbaa !16
  %75 = load i16, ptr %15, align 2, !tbaa !14
  %76 = sext i16 %75 to i64
  %77 = getelementptr inbounds [6 x float], ptr %3, i64 0, i64 %76
  %78 = load float, ptr %77, align 4, !tbaa !16
  %79 = fsub float %74, %78
  store float %79, ptr %7, align 4, !tbaa !16
  %80 = load i16, ptr %16, align 2, !tbaa !14
  %81 = sext i16 %80 to i64
  %82 = getelementptr inbounds [6 x float], ptr %4, i64 0, i64 %81
  %83 = load float, ptr %82, align 4, !tbaa !16
  %84 = load i16, ptr %15, align 2, !tbaa !14
  %85 = sext i16 %84 to i64
  %86 = getelementptr inbounds [6 x float], ptr %4, i64 0, i64 %85
  %87 = load float, ptr %86, align 4, !tbaa !16
  %88 = fsub float %83, %87
  store float %88, ptr %8, align 4, !tbaa !16
  %89 = load float, ptr %7, align 4, !tbaa !16
  %90 = load float, ptr %7, align 4, !tbaa !16
  %91 = load float, ptr %8, align 4, !tbaa !16
  %92 = load float, ptr %8, align 4, !tbaa !16
  %93 = fmul float %91, %92
  %94 = call float @llvm.fmuladd.f32(float %89, float %90, float %93)
  %95 = fadd float %94, 1.000000e+00
  store float %95, ptr %9, align 4, !tbaa !16
  %96 = load float, ptr %9, align 4, !tbaa !16
  %97 = fdiv float 1.000000e+00, %96
  store float %97, ptr %10, align 4, !tbaa !16
  %98 = load float, ptr %11, align 4, !tbaa !16
  %99 = load float, ptr %7, align 4, !tbaa !16
  %100 = load float, ptr %10, align 4, !tbaa !16
  %101 = call float @llvm.fmuladd.f32(float %99, float %100, float %98)
  store float %101, ptr %11, align 4, !tbaa !16
  %102 = load float, ptr %12, align 4, !tbaa !16
  %103 = load float, ptr %8, align 4, !tbaa !16
  %104 = load float, ptr %10, align 4, !tbaa !16
  %105 = call float @llvm.fmuladd.f32(float %103, float %104, float %102)
  store float %105, ptr %12, align 4, !tbaa !16
  br label %106

106:
  br label %107

107:
  %108 = load i16, ptr %16, align 2, !tbaa !14
  %109 = add i16 %108, 1
  store i16 %109, ptr %16, align 2, !tbaa !14
  br label %60, !llvm.loop !20

110:
  %111 = load i16, ptr %15, align 2, !tbaa !14
  %112 = sext i16 %111 to i64
  %113 = getelementptr inbounds [6 x float], ptr %5, i64 0, i64 %112
  %114 = load float, ptr %113, align 4, !tbaa !16
  %115 = load float, ptr %11, align 4, !tbaa !16
  %116 = fadd float %114, %115
  %117 = load i16, ptr %15, align 2, !tbaa !14
  %118 = sext i16 %117 to i64
  %119 = getelementptr inbounds [6 x float], ptr %5, i64 0, i64 %118
  store float %116, ptr %119, align 4, !tbaa !16
  %120 = load i16, ptr %15, align 2, !tbaa !14
  %121 = sext i16 %120 to i64
  %122 = getelementptr inbounds [6 x float], ptr %6, i64 0, i64 %121
  %123 = load float, ptr %122, align 4, !tbaa !16
  %124 = load float, ptr %12, align 4, !tbaa !16
  %125 = fadd float %123, %124
  %126 = load i16, ptr %15, align 2, !tbaa !14
  %127 = sext i16 %126 to i64
  %128 = getelementptr inbounds [6 x float], ptr %6, i64 0, i64 %127
  store float %125, ptr %128, align 4, !tbaa !16
  %129 = load i16, ptr %15, align 2, !tbaa !14
  %130 = sext i16 %129 to i64
  %131 = getelementptr inbounds [6 x float], ptr %5, i64 0, i64 %130
  %132 = load float, ptr %131, align 4, !tbaa !16
  %133 = load i16, ptr %15, align 2, !tbaa !14
  %134 = sext i16 %133 to i64
  %135 = getelementptr inbounds [6 x float], ptr %5, i64 0, i64 %134
  %136 = load float, ptr %135, align 4, !tbaa !16
  %137 = fdiv float %136, 1.600000e+01
  %138 = fsub float %132, %137
  %139 = load i16, ptr %15, align 2, !tbaa !14
  %140 = sext i16 %139 to i64
  %141 = getelementptr inbounds [6 x float], ptr %5, i64 0, i64 %140
  store float %138, ptr %141, align 4, !tbaa !16
  %142 = load i16, ptr %15, align 2, !tbaa !14
  %143 = sext i16 %142 to i64
  %144 = getelementptr inbounds [6 x float], ptr %6, i64 0, i64 %143
  %145 = load float, ptr %144, align 4, !tbaa !16
  %146 = load i16, ptr %15, align 2, !tbaa !14
  %147 = sext i16 %146 to i64
  %148 = getelementptr inbounds [6 x float], ptr %6, i64 0, i64 %147
  %149 = load float, ptr %148, align 4, !tbaa !16
  %150 = fdiv float %149, 1.600000e+01
  %151 = fsub float %145, %150
  %152 = load i16, ptr %15, align 2, !tbaa !14
  %153 = sext i16 %152 to i64
  %154 = getelementptr inbounds [6 x float], ptr %6, i64 0, i64 %153
  store float %151, ptr %154, align 4, !tbaa !16
  br label %155

155:
  %156 = load i16, ptr %15, align 2, !tbaa !14
  %157 = add i16 %156, 1
  store i16 %157, ptr %15, align 2, !tbaa !14
  br label %55, !llvm.loop !21

158:
  store i16 0, ptr %15, align 2, !tbaa !14
  br label %159

159:
  %160 = load i16, ptr %15, align 2, !tbaa !14
  %161 = sext i16 %160 to i32
  %162 = icmp slt i32 %161, 6
  br i1 %162, label %163, label %191

163:
  %164 = load i16, ptr %15, align 2, !tbaa !14
  %165 = sext i16 %164 to i64
  %166 = getelementptr inbounds [6 x float], ptr %3, i64 0, i64 %165
  %167 = load float, ptr %166, align 4, !tbaa !16
  %168 = load i16, ptr %15, align 2, !tbaa !14
  %169 = sext i16 %168 to i64
  %170 = getelementptr inbounds [6 x float], ptr %5, i64 0, i64 %169
  %171 = load float, ptr %170, align 4, !tbaa !16
  %172 = fadd float %167, %171
  %173 = load i16, ptr %15, align 2, !tbaa !14
  %174 = sext i16 %173 to i64
  %175 = getelementptr inbounds [6 x float], ptr %3, i64 0, i64 %174
  store float %172, ptr %175, align 4, !tbaa !16
  %176 = load i16, ptr %15, align 2, !tbaa !14
  %177 = sext i16 %176 to i64
  %178 = getelementptr inbounds [6 x float], ptr %4, i64 0, i64 %177
  %179 = load float, ptr %178, align 4, !tbaa !16
  %180 = load i16, ptr %15, align 2, !tbaa !14
  %181 = sext i16 %180 to i64
  %182 = getelementptr inbounds [6 x float], ptr %6, i64 0, i64 %181
  %183 = load float, ptr %182, align 4, !tbaa !16
  %184 = fadd float %179, %183
  %185 = load i16, ptr %15, align 2, !tbaa !14
  %186 = sext i16 %185 to i64
  %187 = getelementptr inbounds [6 x float], ptr %4, i64 0, i64 %186
  store float %184, ptr %187, align 4, !tbaa !16
  br label %188

188:
  %189 = load i16, ptr %15, align 2, !tbaa !14
  %190 = add i16 %189, 1
  store i16 %190, ptr %15, align 2, !tbaa !14
  br label %159, !llvm.loop !22

191:
  br label %192

192:
  %193 = load i16, ptr %14, align 2, !tbaa !14
  %194 = add i16 %193, 1
  store i16 %194, ptr %14, align 2, !tbaa !14
  br label %48, !llvm.loop !23

195:
  store i64 0, ptr %13, align 8, !tbaa !24
  store i16 0, ptr %15, align 2, !tbaa !14
  br label %196

196:
  %197 = load i16, ptr %15, align 2, !tbaa !14
  %198 = sext i16 %197 to i32
  %199 = icmp slt i32 %198, 6
  br i1 %199, label %200, label %229

200:
  %201 = load i16, ptr %15, align 2, !tbaa !14
  %202 = sext i16 %201 to i64
  %203 = getelementptr inbounds [6 x float], ptr %3, i64 0, i64 %202
  %204 = load float, ptr %203, align 4, !tbaa !16
  %205 = fmul float %204, 6.400000e+01
  %206 = fptosi float %205 to i64
  %207 = load i16, ptr %15, align 2, !tbaa !14
  %208 = sext i16 %207 to i32
  %209 = add nsw i32 %208, 1
  %210 = sext i32 %209 to i64
  %211 = mul nsw i64 %206, %210
  %212 = load i16, ptr %15, align 2, !tbaa !14
  %213 = sext i16 %212 to i64
  %214 = getelementptr inbounds [6 x float], ptr %4, i64 0, i64 %213
  %215 = load float, ptr %214, align 4, !tbaa !16
  %216 = fmul float %215, 6.400000e+01
  %217 = fptosi float %216 to i64
  %218 = load i16, ptr %15, align 2, !tbaa !14
  %219 = sext i16 %218 to i32
  %220 = add nsw i32 %219, 7
  %221 = sext i32 %220 to i64
  %222 = mul nsw i64 %217, %221
  %223 = add nsw i64 %211, %222
  %224 = load i64, ptr %13, align 8, !tbaa !24
  %225 = add nsw i64 %224, %223
  store i64 %225, ptr %13, align 8, !tbaa !24
  br label %226

226:
  %227 = load i16, ptr %15, align 2, !tbaa !14
  %228 = add i16 %227, 1
  store i16 %228, ptr %15, align 2, !tbaa !14
  br label %196, !llvm.loop !26

229:
  %230 = load i64, ptr %13, align 8, !tbaa !24
  call void @llvm.lifetime.end.p0(i64 -1, ptr %16) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %15) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %14) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %13) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %12) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %11) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %10) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %9) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %8) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %7) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %6) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  ret i64 %230
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare float @llvm.fmuladd.f32(float, float, float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %1 = load volatile i32, ptr @opq0, align 4, !tbaa !27
  %2 = add nsw i32 20, %1
  %3 = trunc i32 %2 to i16
  %4 = call i64 @bench_fpbench(i16 noundef %3) nobuiltin "no-builtins"
  call void @report(i64 noundef %4) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i64 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"

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
!14 = !{!15, !15, i64 0}
!15 = !{!"short", !12, i64 0}
!16 = !{!17, !17, i64 0}
!17 = !{!"float", !12, i64 0}
!18 = distinct !{!18, !19}
!19 = !{!"llvm.loop.mustprogress"}
!20 = distinct !{!20, !19}
!21 = distinct !{!21, !19}
!22 = distinct !{!22, !19}
!23 = distinct !{!23, !19}
!24 = !{!25, !25, i64 0}
!25 = !{!"long", !12, i64 0}
!26 = distinct !{!26, !19}
!27 = !{!11, !11, i64 0}
