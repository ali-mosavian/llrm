target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"X#" = internal global [8 x i8] zeroinitializer
@"Y#" = internal global [8 x i8] zeroinitializer
@"S!" = internal global [4 x i8] zeroinitializer
@"U!" = internal global [4 x i8] zeroinitializer
@$arguments$5 = internal global [158 x i8] zeroinitializer
@$float4 = internal constant [8 x i8] zeroinitializer
@$float5 = internal constant [8 x i8] c"\00\00\00\00\E0\FF\DF@"
@$float6 = internal constant [8 x i8] c"\00\00\00\00\00\00\F0?"
@$float7 = internal constant [4 x i8] zeroinitializer
@$float8 = internal constant [4 x i8] c"\00\FF\FFF"
@$float9 = internal constant [4 x i8] c"\00\00\80?"
@$float10 = internal constant [4 x i8] c"\00\00\00?"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  store i16 10, ptr @$arguments$5, !tbaa !2
  %0 = load i16, ptr @$arguments$5, !tbaa !2
  %1 = sitofp i16 %0 to double
  %2 = fpround double %1 to double
  store double %2, ptr @"X#", !tbaa !2
  %3 = getelementptr i8, ptr @$arguments$5, i16 2
  store i16 308, ptr %3, !tbaa !2
  %4 = getelementptr i8, ptr @$arguments$5, i16 2
  %5 = load i16, ptr %4, !tbaa !2
  %6 = sitofp i16 %5 to double
  %7 = fpround double %6 to double
  store double %7, ptr @"Y#", !tbaa !2
  %8 = load double, ptr @"X#", !tbaa !2
  %9 = load double, ptr @"Y#", !tbaa !2
  %10 = getelementptr i8, ptr @$arguments$5, i16 4
  store double %8, ptr %10, !tbaa !2
  %11 = getelementptr i8, ptr @$arguments$5, i16 12
  store double %9, ptr %11, !tbaa !2
  %12 = getelementptr i8, ptr @$arguments$5, i16 4
  %13 = load double, ptr %12, !tbaa !2
  %14 = load double, ptr @$float4, !tbaa !2
  %15 = fcmp ogt double %13, %14
  br i1 %15, label %b2, label %b3

b2:
  %16 = getelementptr i8, ptr @$arguments$5, i16 4
  %17 = load double, ptr %16, !tbaa !2
  %18 = call double @llvm.log2.f64(double %17)
  %19 = getelementptr i8, ptr @$arguments$5, i16 12
  %20 = load double, ptr %19, !tbaa !2
  %21 = fmul double %18, %20
  %22 = load double, ptr @$float5, !tbaa !2
  %23 = fcmp oge double %21, %22
  br i1 %23, label %b11, label %b12

b3:
  %24 = getelementptr i8, ptr @$arguments$5, i16 4
  %25 = load double, ptr %24, !tbaa !2
  %26 = load double, ptr @$float4, !tbaa !2
  %27 = fcmp oeq double %25, %26
  br i1 %27, label %b4, label %b6

b4:
  %28 = getelementptr i8, ptr @$arguments$5, i16 12
  %29 = load double, ptr %28, !tbaa !2
  %30 = load double, ptr @$float4, !tbaa !2
  %31 = fcmp ogt double %29, %30
  br i1 %31, label %b13, label %b5

b5:
  %32 = getelementptr i8, ptr @$arguments$5, i16 12
  %33 = load double, ptr %32, !tbaa !2
  %34 = load double, ptr @$float4, !tbaa !2
  %35 = fcmp oeq double %33, %34
  br i1 %35, label %b14, label %b9

b6:
  %36 = getelementptr i8, ptr @$arguments$5, i16 12
  %37 = load double, ptr %36, !tbaa !2
  %38 = call i32 @llvm.lrint.i32.f64(double %37)
  %39 = getelementptr i8, ptr @$arguments$5, i16 28
  store i32 %38, ptr %39, !tbaa !2
  %40 = getelementptr i8, ptr @$arguments$5, i16 28
  %41 = load i32, ptr %40, !tbaa !2
  %42 = getelementptr i8, ptr @$arguments$5, i16 32
  store i32 %41, ptr %42, !tbaa !2
  %43 = getelementptr i8, ptr @$arguments$5, i16 32
  %44 = load i32, ptr %43, !tbaa !2
  %45 = sitofp i32 %44 to double
  %46 = getelementptr i8, ptr @$arguments$5, i16 12
  %47 = load double, ptr %46, !tbaa !2
  %48 = fcmp oeq double %45, %47
  br i1 %48, label %b7, label %b9

b7:
  %49 = getelementptr i8, ptr @$arguments$5, i16 4
  %50 = load double, ptr %49, !tbaa !2
  %51 = fneg double %50
  %52 = call double @llvm.log2.f64(double %51)
  %53 = getelementptr i8, ptr @$arguments$5, i16 12
  %54 = load double, ptr %53, !tbaa !2
  %55 = fmul double %52, %54
  %56 = load double, ptr @$float5, !tbaa !2
  %57 = fcmp oge double %55, %56
  br i1 %57, label %b15, label %b16

b8:
  %58 = getelementptr i8, ptr @$arguments$5, i16 20
  %59 = load double, ptr %58, !tbaa !2
  %60 = fneg double %59
  %61 = fpround double %60 to double
  %62 = getelementptr i8, ptr @$arguments$5, i16 20
  store double %61, ptr %62, !tbaa !2
  br label %b10

b9:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b10:
  %63 = getelementptr i8, ptr @$arguments$5, i16 20
  %64 = load double, ptr %63, !tbaa !2
  %65 = getelementptr i8, ptr @$arguments$5, i16 36
  store double %64, ptr %65, !tbaa !2
  %66 = getelementptr i8, ptr @$arguments$5, i16 36
  %67 = load double, ptr %66, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %67)
  %68 = getelementptr i8, ptr @$arguments$5, i16 44
  store i16 10, ptr %68, !tbaa !2
  %69 = getelementptr i8, ptr @$arguments$5, i16 44
  %70 = load i16, ptr %69, !tbaa !2
  %71 = sitofp i16 %70 to float
  %72 = fpround float %71 to float
  store float %72, ptr @"S!", !tbaa !2
  %73 = getelementptr i8, ptr @$arguments$5, i16 46
  store i16 38, ptr %73, !tbaa !2
  %74 = getelementptr i8, ptr @$arguments$5, i16 46
  %75 = load i16, ptr %74, !tbaa !2
  %76 = sitofp i16 %75 to float
  %77 = fpround float %76 to float
  store float %77, ptr @"U!", !tbaa !2
  %78 = load float, ptr @"S!", !tbaa !2
  %79 = load float, ptr @"U!", !tbaa !2
  %80 = getelementptr i8, ptr @$arguments$5, i16 48
  store float %78, ptr %80, !tbaa !2
  %81 = getelementptr i8, ptr @$arguments$5, i16 52
  store float %79, ptr %81, !tbaa !2
  %82 = getelementptr i8, ptr @$arguments$5, i16 48
  %83 = load float, ptr %82, !tbaa !2
  %84 = load float, ptr @$float7, !tbaa !2
  %85 = fcmp ogt float %83, %84
  br i1 %85, label %b17, label %b18

b11:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b12:
  %86 = call double @llvm.exp2.f64(double %21)
  %87 = fpround double %86 to double
  %88 = getelementptr i8, ptr @$arguments$5, i16 20
  store double %87, ptr %88, !tbaa !2
  br label %b10

b13:
  %89 = load double, ptr @$float4, !tbaa !2
  %90 = getelementptr i8, ptr @$arguments$5, i16 20
  store double %89, ptr %90, !tbaa !2
  br label %b10

b14:
  %91 = load double, ptr @$float6, !tbaa !2
  %92 = getelementptr i8, ptr @$arguments$5, i16 20
  store double %91, ptr %92, !tbaa !2
  br label %b10

b15:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b16:
  %93 = call double @llvm.exp2.f64(double %55)
  %94 = fpround double %93 to double
  %95 = getelementptr i8, ptr @$arguments$5, i16 20
  store double %94, ptr %95, !tbaa !2
  %96 = getelementptr i8, ptr @$arguments$5, i16 28
  %97 = load i32, ptr %96, !tbaa !2
  %98 = and i32 %97, 1
  %99 = icmp ne i32 %98, 0
  br i1 %99, label %b8, label %b10

b17:
  %100 = getelementptr i8, ptr @$arguments$5, i16 48
  %101 = load float, ptr %100, !tbaa !2
  %102 = call float @llvm.log2.f32(float %101)
  %103 = getelementptr i8, ptr @$arguments$5, i16 52
  %104 = load float, ptr %103, !tbaa !2
  %105 = fmul float %102, %104
  %106 = load float, ptr @$float8, !tbaa !2
  %107 = fcmp oge float %105, %106
  br i1 %107, label %b26, label %b27

b18:
  %108 = getelementptr i8, ptr @$arguments$5, i16 48
  %109 = load float, ptr %108, !tbaa !2
  %110 = load float, ptr @$float7, !tbaa !2
  %111 = fcmp oeq float %109, %110
  br i1 %111, label %b19, label %b21

b19:
  %112 = getelementptr i8, ptr @$arguments$5, i16 52
  %113 = load float, ptr %112, !tbaa !2
  %114 = load float, ptr @$float7, !tbaa !2
  %115 = fcmp ogt float %113, %114
  br i1 %115, label %b28, label %b20

b20:
  %116 = getelementptr i8, ptr @$arguments$5, i16 52
  %117 = load float, ptr %116, !tbaa !2
  %118 = load float, ptr @$float7, !tbaa !2
  %119 = fcmp oeq float %117, %118
  br i1 %119, label %b29, label %b24

b21:
  %120 = getelementptr i8, ptr @$arguments$5, i16 52
  %121 = load float, ptr %120, !tbaa !2
  %122 = call i32 @llvm.lrint.i32.f32(float %121)
  %123 = getelementptr i8, ptr @$arguments$5, i16 60
  store i32 %122, ptr %123, !tbaa !2
  %124 = getelementptr i8, ptr @$arguments$5, i16 60
  %125 = load i32, ptr %124, !tbaa !2
  %126 = getelementptr i8, ptr @$arguments$5, i16 64
  store i32 %125, ptr %126, !tbaa !2
  %127 = getelementptr i8, ptr @$arguments$5, i16 64
  %128 = load i32, ptr %127, !tbaa !2
  %129 = sitofp i32 %128 to float
  %130 = getelementptr i8, ptr @$arguments$5, i16 52
  %131 = load float, ptr %130, !tbaa !2
  %132 = fcmp oeq float %129, %131
  br i1 %132, label %b22, label %b24

b22:
  %133 = getelementptr i8, ptr @$arguments$5, i16 48
  %134 = load float, ptr %133, !tbaa !2
  %135 = fneg float %134
  %136 = call float @llvm.log2.f32(float %135)
  %137 = getelementptr i8, ptr @$arguments$5, i16 52
  %138 = load float, ptr %137, !tbaa !2
  %139 = fmul float %136, %138
  %140 = load float, ptr @$float8, !tbaa !2
  %141 = fcmp oge float %139, %140
  br i1 %141, label %b30, label %b31

b23:
  %142 = getelementptr i8, ptr @$arguments$5, i16 56
  %143 = load float, ptr %142, !tbaa !2
  %144 = fneg float %143
  %145 = fpround float %144 to float
  %146 = getelementptr i8, ptr @$arguments$5, i16 56
  store float %145, ptr %146, !tbaa !2
  br label %b25

b24:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b25:
  %147 = getelementptr i8, ptr @$arguments$5, i16 56
  %148 = load float, ptr %147, !tbaa !2
  %149 = getelementptr i8, ptr @$arguments$5, i16 68
  store float %148, ptr %149, !tbaa !2
  %150 = getelementptr i8, ptr @$arguments$5, i16 68
  %151 = load float, ptr %150, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER4(float %151)
  %152 = getelementptr i8, ptr @$arguments$5, i16 72
  store i16 10, ptr %152, !tbaa !2
  %153 = getelementptr i8, ptr @$arguments$5, i16 72
  %154 = load i16, ptr %153, !tbaa !2
  %155 = sitofp i16 %154 to double
  %156 = fpround double %155 to double
  store double %156, ptr @"X#", !tbaa !2
  %157 = getelementptr i8, ptr @$arguments$5, i16 74
  store i16 22, ptr %157, !tbaa !2
  %158 = getelementptr i8, ptr @$arguments$5, i16 74
  %159 = load i16, ptr %158, !tbaa !2
  %160 = sitofp i16 %159 to double
  %161 = fpround double %160 to double
  store double %161, ptr @"Y#", !tbaa !2
  %162 = load double, ptr @"X#", !tbaa !2
  %163 = load double, ptr @"Y#", !tbaa !2
  %164 = getelementptr i8, ptr @$arguments$5, i16 76
  store double %162, ptr %164, !tbaa !2
  %165 = getelementptr i8, ptr @$arguments$5, i16 84
  store double %163, ptr %165, !tbaa !2
  %166 = getelementptr i8, ptr @$arguments$5, i16 76
  %167 = load double, ptr %166, !tbaa !2
  %168 = load double, ptr @$float4, !tbaa !2
  %169 = fcmp ogt double %167, %168
  br i1 %169, label %b32, label %b33

b26:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b27:
  %170 = call float @llvm.exp2.f32(float %105)
  %171 = fpround float %170 to float
  %172 = getelementptr i8, ptr @$arguments$5, i16 56
  store float %171, ptr %172, !tbaa !2
  br label %b25

b28:
  %173 = load float, ptr @$float7, !tbaa !2
  %174 = getelementptr i8, ptr @$arguments$5, i16 56
  store float %173, ptr %174, !tbaa !2
  br label %b25

b29:
  %175 = load float, ptr @$float9, !tbaa !2
  %176 = getelementptr i8, ptr @$arguments$5, i16 56
  store float %175, ptr %176, !tbaa !2
  br label %b25

b30:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b31:
  %177 = call float @llvm.exp2.f32(float %139)
  %178 = fpround float %177 to float
  %179 = getelementptr i8, ptr @$arguments$5, i16 56
  store float %178, ptr %179, !tbaa !2
  %180 = getelementptr i8, ptr @$arguments$5, i16 60
  %181 = load i32, ptr %180, !tbaa !2
  %182 = and i32 %181, 1
  %183 = icmp ne i32 %182, 0
  br i1 %183, label %b23, label %b25

b32:
  %184 = getelementptr i8, ptr @$arguments$5, i16 76
  %185 = load double, ptr %184, !tbaa !2
  %186 = call double @llvm.log2.f64(double %185)
  %187 = getelementptr i8, ptr @$arguments$5, i16 84
  %188 = load double, ptr %187, !tbaa !2
  %189 = fmul double %186, %188
  %190 = load double, ptr @$float5, !tbaa !2
  %191 = fcmp oge double %189, %190
  br i1 %191, label %b41, label %b42

b33:
  %192 = getelementptr i8, ptr @$arguments$5, i16 76
  %193 = load double, ptr %192, !tbaa !2
  %194 = load double, ptr @$float4, !tbaa !2
  %195 = fcmp oeq double %193, %194
  br i1 %195, label %b34, label %b36

b34:
  %196 = getelementptr i8, ptr @$arguments$5, i16 84
  %197 = load double, ptr %196, !tbaa !2
  %198 = load double, ptr @$float4, !tbaa !2
  %199 = fcmp ogt double %197, %198
  br i1 %199, label %b43, label %b35

b35:
  %200 = getelementptr i8, ptr @$arguments$5, i16 84
  %201 = load double, ptr %200, !tbaa !2
  %202 = load double, ptr @$float4, !tbaa !2
  %203 = fcmp oeq double %201, %202
  br i1 %203, label %b44, label %b39

b36:
  %204 = getelementptr i8, ptr @$arguments$5, i16 84
  %205 = load double, ptr %204, !tbaa !2
  %206 = call i32 @llvm.lrint.i32.f64(double %205)
  %207 = getelementptr i8, ptr @$arguments$5, i16 100
  store i32 %206, ptr %207, !tbaa !2
  %208 = getelementptr i8, ptr @$arguments$5, i16 100
  %209 = load i32, ptr %208, !tbaa !2
  %210 = getelementptr i8, ptr @$arguments$5, i16 104
  store i32 %209, ptr %210, !tbaa !2
  %211 = getelementptr i8, ptr @$arguments$5, i16 104
  %212 = load i32, ptr %211, !tbaa !2
  %213 = sitofp i32 %212 to double
  %214 = getelementptr i8, ptr @$arguments$5, i16 84
  %215 = load double, ptr %214, !tbaa !2
  %216 = fcmp oeq double %213, %215
  br i1 %216, label %b37, label %b39

b37:
  %217 = getelementptr i8, ptr @$arguments$5, i16 76
  %218 = load double, ptr %217, !tbaa !2
  %219 = fneg double %218
  %220 = call double @llvm.log2.f64(double %219)
  %221 = getelementptr i8, ptr @$arguments$5, i16 84
  %222 = load double, ptr %221, !tbaa !2
  %223 = fmul double %220, %222
  %224 = load double, ptr @$float5, !tbaa !2
  %225 = fcmp oge double %223, %224
  br i1 %225, label %b45, label %b46

b38:
  %226 = getelementptr i8, ptr @$arguments$5, i16 92
  %227 = load double, ptr %226, !tbaa !2
  %228 = fneg double %227
  %229 = fpround double %228 to double
  %230 = getelementptr i8, ptr @$arguments$5, i16 92
  store double %229, ptr %230, !tbaa !2
  br label %b40

b39:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b40:
  %231 = getelementptr i8, ptr @$arguments$5, i16 92
  %232 = load double, ptr %231, !tbaa !2
  %233 = getelementptr i8, ptr @$arguments$5, i16 108
  store double %232, ptr %233, !tbaa !2
  %234 = getelementptr i8, ptr @$arguments$5, i16 108
  %235 = load double, ptr %234, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %235)
  %236 = getelementptr i8, ptr @$arguments$5, i16 116
  store i16 2, ptr %236, !tbaa !2
  %237 = getelementptr i8, ptr @$arguments$5, i16 116
  %238 = load i16, ptr %237, !tbaa !2
  %239 = sitofp i16 %238 to double
  %240 = fpround double %239 to double
  store double %240, ptr @"X#", !tbaa !2
  %241 = load float, ptr @$float10, !tbaa !2
  %242 = fpext float %241 to double
  %243 = fpround double %242 to double
  store double %243, ptr @"Y#", !tbaa !2
  %244 = load double, ptr @"X#", !tbaa !2
  %245 = load double, ptr @"Y#", !tbaa !2
  %246 = getelementptr i8, ptr @$arguments$5, i16 118
  store double %244, ptr %246, !tbaa !2
  %247 = getelementptr i8, ptr @$arguments$5, i16 126
  store double %245, ptr %247, !tbaa !2
  %248 = getelementptr i8, ptr @$arguments$5, i16 118
  %249 = load double, ptr %248, !tbaa !2
  %250 = load double, ptr @$float4, !tbaa !2
  %251 = fcmp ogt double %249, %250
  br i1 %251, label %b47, label %b48

b41:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b42:
  %252 = call double @llvm.exp2.f64(double %189)
  %253 = fpround double %252 to double
  %254 = getelementptr i8, ptr @$arguments$5, i16 92
  store double %253, ptr %254, !tbaa !2
  br label %b40

b43:
  %255 = load double, ptr @$float4, !tbaa !2
  %256 = getelementptr i8, ptr @$arguments$5, i16 92
  store double %255, ptr %256, !tbaa !2
  br label %b40

b44:
  %257 = load double, ptr @$float6, !tbaa !2
  %258 = getelementptr i8, ptr @$arguments$5, i16 92
  store double %257, ptr %258, !tbaa !2
  br label %b40

b45:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b46:
  %259 = call double @llvm.exp2.f64(double %223)
  %260 = fpround double %259 to double
  %261 = getelementptr i8, ptr @$arguments$5, i16 92
  store double %260, ptr %261, !tbaa !2
  %262 = getelementptr i8, ptr @$arguments$5, i16 100
  %263 = load i32, ptr %262, !tbaa !2
  %264 = and i32 %263, 1
  %265 = icmp ne i32 %264, 0
  br i1 %265, label %b38, label %b40

b47:
  %266 = getelementptr i8, ptr @$arguments$5, i16 118
  %267 = load double, ptr %266, !tbaa !2
  %268 = call double @llvm.log2.f64(double %267)
  %269 = getelementptr i8, ptr @$arguments$5, i16 126
  %270 = load double, ptr %269, !tbaa !2
  %271 = fmul double %268, %270
  %272 = load double, ptr @$float5, !tbaa !2
  %273 = fcmp oge double %271, %272
  br i1 %273, label %b56, label %b57

b48:
  %274 = getelementptr i8, ptr @$arguments$5, i16 118
  %275 = load double, ptr %274, !tbaa !2
  %276 = load double, ptr @$float4, !tbaa !2
  %277 = fcmp oeq double %275, %276
  br i1 %277, label %b49, label %b51

b49:
  %278 = getelementptr i8, ptr @$arguments$5, i16 126
  %279 = load double, ptr %278, !tbaa !2
  %280 = load double, ptr @$float4, !tbaa !2
  %281 = fcmp ogt double %279, %280
  br i1 %281, label %b58, label %b50

b50:
  %282 = getelementptr i8, ptr @$arguments$5, i16 126
  %283 = load double, ptr %282, !tbaa !2
  %284 = load double, ptr @$float4, !tbaa !2
  %285 = fcmp oeq double %283, %284
  br i1 %285, label %b59, label %b54

b51:
  %286 = getelementptr i8, ptr @$arguments$5, i16 126
  %287 = load double, ptr %286, !tbaa !2
  %288 = call i32 @llvm.lrint.i32.f64(double %287)
  %289 = getelementptr i8, ptr @$arguments$5, i16 142
  store i32 %288, ptr %289, !tbaa !2
  %290 = getelementptr i8, ptr @$arguments$5, i16 142
  %291 = load i32, ptr %290, !tbaa !2
  %292 = getelementptr i8, ptr @$arguments$5, i16 146
  store i32 %291, ptr %292, !tbaa !2
  %293 = getelementptr i8, ptr @$arguments$5, i16 146
  %294 = load i32, ptr %293, !tbaa !2
  %295 = sitofp i32 %294 to double
  %296 = getelementptr i8, ptr @$arguments$5, i16 126
  %297 = load double, ptr %296, !tbaa !2
  %298 = fcmp oeq double %295, %297
  br i1 %298, label %b52, label %b54

b52:
  %299 = getelementptr i8, ptr @$arguments$5, i16 118
  %300 = load double, ptr %299, !tbaa !2
  %301 = fneg double %300
  %302 = call double @llvm.log2.f64(double %301)
  %303 = getelementptr i8, ptr @$arguments$5, i16 126
  %304 = load double, ptr %303, !tbaa !2
  %305 = fmul double %302, %304
  %306 = load double, ptr @$float5, !tbaa !2
  %307 = fcmp oge double %305, %306
  br i1 %307, label %b60, label %b61

b53:
  %308 = getelementptr i8, ptr @$arguments$5, i16 134
  %309 = load double, ptr %308, !tbaa !2
  %310 = fneg double %309
  %311 = fpround double %310 to double
  %312 = getelementptr i8, ptr @$arguments$5, i16 134
  store double %311, ptr %312, !tbaa !2
  br label %b55

b54:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 5) cold
  unreachable

b55:
  %313 = getelementptr i8, ptr @$arguments$5, i16 134
  %314 = load double, ptr %313, !tbaa !2
  %315 = getelementptr i8, ptr @$arguments$5, i16 150
  store double %314, ptr %315, !tbaa !2
  %316 = getelementptr i8, ptr @$arguments$5, i16 150
  %317 = load double, ptr %316, !tbaa !2
  call cc 1000 addrspace(1) void @llrm.qb.B$PER8(double %317)
  ret void

b56:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b57:
  %318 = call double @llvm.exp2.f64(double %271)
  %319 = fpround double %318 to double
  %320 = getelementptr i8, ptr @$arguments$5, i16 134
  store double %319, ptr %320, !tbaa !2
  br label %b55

b58:
  %321 = load double, ptr @$float4, !tbaa !2
  %322 = getelementptr i8, ptr @$arguments$5, i16 134
  store double %321, ptr %322, !tbaa !2
  br label %b55

b59:
  %323 = load double, ptr @$float6, !tbaa !2
  %324 = getelementptr i8, ptr @$arguments$5, i16 134
  store double %323, ptr %324, !tbaa !2
  br label %b55

b60:
  call cc 1000 addrspace(1) void @llrm.qb.B$SERR(i16 6) cold
  unreachable

b61:
  %325 = call double @llvm.exp2.f64(double %305)
  %326 = fpround double %325 to double
  %327 = getelementptr i8, ptr @$arguments$5, i16 134
  store double %326, ptr %327, !tbaa !2
  %328 = getelementptr i8, ptr @$arguments$5, i16 142
  %329 = load i32, ptr %328, !tbaa !2
  %330 = and i32 %329, 1
  %331 = icmp ne i32 %330, 0
  br i1 %331, label %b53, label %b55
}

declare double @llvm.log2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$SERR(i16) addrspace(1)

declare cc 1000 void @llrm.qb.B$PER8(double) addrspace(1)

declare double @llvm.exp2.f64(double) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare float @llvm.log2.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare cc 1000 void @llrm.qb.B$PER4(float) addrspace(1)

declare float @llvm.exp2.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
