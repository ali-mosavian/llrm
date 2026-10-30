target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$data = internal global [0 x i8] zeroinitializer
@$qb$statementTable = internal constant [0 x i8] zeroinitializer
@"POSX!" = internal global [28 x i8] zeroinitializer, align 2
@POSX$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"POSX!" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"POSX!", [6 x i8] c"\04\00\07\00\00\00" }>
@"POSY!" = internal global [28 x i8] zeroinitializer, align 2
@POSY$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"POSY!" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"POSY!", [6 x i8] c"\04\00\07\00\00\00" }>
@"VELX!" = internal global [28 x i8] zeroinitializer, align 2
@VELX$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VELX!" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VELX!", [6 x i8] c"\04\00\07\00\00\00" }>
@"VELY!" = internal global [28 x i8] zeroinitializer, align 2
@VELY$descriptor = internal constant <{ ptr addrspace(1), [6 x i8], ptr, [6 x i8] }> <{ ptr addrspace(1) addrspacecast (ptr @"VELY!" to ptr addrspace(1)), [6 x i8] c"\00\00\00\00\01@", ptr @"VELY!", [6 x i8] c"\04\00\07\00\00\00" }>
@$float7 = internal constant [4 x i8] c"\00\00\80?"
@$float8 = internal constant [4 x i8] c"\00\00\00A"
@$float9 = internal constant [4 x i8] c"\00\00\80D"

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = call cc 1000 addrspace(1) i32 @"BENCHNBODYSINGLE&"()
  %1 = call cc 1000 addrspace(1) ptr @llrm.qb.B$STI4(i32 %0)
  %2 = call cc 1000 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %1)
  call cc 1000 addrspace(1) void @llrm.qb.B$PESD(ptr %2)
  call cc 1000 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 i32 @"BENCHNBODYSINGLE&"() addrspace(1) "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca float
  %27 = alloca float
  %28 = alloca float
  %29 = alloca float
  %30 = alloca float
  %31 = alloca float
  %32 = alloca float
  %33 = alloca i32
  store i16 0, ptr %25, !tbaa !2
  store i16 5, ptr %24, !tbaa !2
  store i16 1, ptr %23, !tbaa !2
  br label %b2

b2:
  %34 = load i16, ptr %23, !tbaa !2
  %35 = icmp sge i16 %34, 0
  br i1 %35, label %b3, label %b4

b3:
  %36 = load i16, ptr %25, !tbaa !2
  %37 = load i16, ptr %24, !tbaa !2
  %38 = icmp sle i16 %36, %37
  br i1 %38, label %b5, label %b6

b4:
  %39 = load i16, ptr %25, !tbaa !2
  %40 = load i16, ptr %24, !tbaa !2
  %41 = icmp sge i16 %39, %40
  br i1 %41, label %b5, label %b6

b5:
  %42 = load i16, ptr %25, !tbaa !2
  %43 = load i16, ptr %25, !tbaa !2
  %44 = mul i16 %43, 7
  %45 = sub i16 %44, 15
  store i16 %45, ptr %22, !tbaa !2
  %46 = load i16, ptr %22, !tbaa !2
  %47 = sitofp i16 %46 to float
  %48 = fpround float %47 to float
  %49 = sub i16 %42, 0
  %50 = getelementptr inbounds float, ptr @"POSX!", i16 %49
  store float %48, ptr %50, !tbaa !2
  %51 = load i16, ptr %25, !tbaa !2
  %52 = load i16, ptr %25, !tbaa !2
  %53 = mul i16 %52, 5
  %54 = sub i16 %53, 12
  store i16 %54, ptr %21, !tbaa !2
  %55 = load i16, ptr %21, !tbaa !2
  %56 = sitofp i16 %55 to float
  %57 = fpround float %56 to float
  %58 = sub i16 %51, 0
  %59 = getelementptr inbounds float, ptr @"POSY!", i16 %58
  store float %57, ptr %59, !tbaa !2
  %60 = load i16, ptr %25, !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  %61 = load i16, ptr %20, !tbaa !2
  %62 = sitofp i16 %61 to float
  %63 = fpround float %62 to float
  %64 = sub i16 %60, 0
  %65 = getelementptr inbounds float, ptr @"VELX!", i16 %64
  store float %63, ptr %65, !tbaa !2
  %66 = load i16, ptr %25, !tbaa !2
  store i16 0, ptr %19, !tbaa !2
  %67 = load i16, ptr %19, !tbaa !2
  %68 = sitofp i16 %67 to float
  %69 = fpround float %68 to float
  %70 = sub i16 %66, 0
  %71 = getelementptr inbounds float, ptr @"VELY!", i16 %70
  store float %69, ptr %71, !tbaa !2
  %72 = load i16, ptr %25, !tbaa !2
  %73 = load i16, ptr %23, !tbaa !2
  %74 = add nsw i16 %72, %73
  store i16 %74, ptr %25, !tbaa !2
  br label %b2

b6:
  store i16 1, ptr %18, !tbaa !2
  store i16 1000, ptr %17, !tbaa !2
  store i16 1, ptr %16, !tbaa !2
  br label %b7

b7:
  %75 = load i16, ptr %16, !tbaa !2
  %76 = icmp sge i16 %75, 0
  br i1 %76, label %b8, label %b9

b8:
  %77 = load i16, ptr %18, !tbaa !2
  %78 = load i16, ptr %17, !tbaa !2
  %79 = icmp sle i16 %77, %78
  br i1 %79, label %b10, label %b11

b9:
  %80 = load i16, ptr %18, !tbaa !2
  %81 = load i16, ptr %17, !tbaa !2
  %82 = icmp sge i16 %80, %81
  br i1 %82, label %b10, label %b11

b10:
  store i16 0, ptr %25, !tbaa !2
  store i16 5, ptr %15, !tbaa !2
  store i16 1, ptr %14, !tbaa !2
  br label %b12

b11:
  store i16 0, ptr %6, !tbaa !2
  %83 = load i16, ptr %6, !tbaa !2
  %84 = sitofp i16 %83 to float
  %85 = fpround float %84 to float
  store float %85, ptr %26, !tbaa !2
  store i16 0, ptr %25, !tbaa !2
  store i16 5, ptr %5, !tbaa !2
  store i16 1, ptr %4, !tbaa !2
  br label %b30

b12:
  %86 = load i16, ptr %14, !tbaa !2
  %87 = icmp sge i16 %86, 0
  br i1 %87, label %b13, label %b14

b13:
  %88 = load i16, ptr %25, !tbaa !2
  %89 = load i16, ptr %15, !tbaa !2
  %90 = icmp sle i16 %88, %89
  br i1 %90, label %b15, label %b16

b14:
  %91 = load i16, ptr %25, !tbaa !2
  %92 = load i16, ptr %15, !tbaa !2
  %93 = icmp sge i16 %91, %92
  br i1 %93, label %b15, label %b16

b15:
  store i16 0, ptr %13, !tbaa !2
  %94 = load i16, ptr %13, !tbaa !2
  %95 = sitofp i16 %94 to float
  %96 = fpround float %95 to float
  store float %96, ptr %28, !tbaa !2
  store i16 0, ptr %12, !tbaa !2
  %97 = load i16, ptr %12, !tbaa !2
  %98 = sitofp i16 %97 to float
  %99 = fpround float %98 to float
  store float %99, ptr %27, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  store i16 5, ptr %10, !tbaa !2
  store i16 1, ptr %9, !tbaa !2
  br label %b17

b16:
  store i16 0, ptr %25, !tbaa !2
  store i16 5, ptr %8, !tbaa !2
  store i16 1, ptr %7, !tbaa !2
  br label %b25

b17:
  %100 = load i16, ptr %9, !tbaa !2
  %101 = icmp sge i16 %100, 0
  br i1 %101, label %b18, label %b19

b18:
  %102 = load i16, ptr %11, !tbaa !2
  %103 = load i16, ptr %10, !tbaa !2
  %104 = icmp sle i16 %102, %103
  br i1 %104, label %b20, label %b21

b19:
  %105 = load i16, ptr %11, !tbaa !2
  %106 = load i16, ptr %10, !tbaa !2
  %107 = icmp sge i16 %105, %106
  br i1 %107, label %b20, label %b21

b20:
  %108 = load i16, ptr %11, !tbaa !2
  %109 = load i16, ptr %25, !tbaa !2
  %110 = icmp ne i16 %108, %109
  br i1 %110, label %b22, label %b23

b21:
  %111 = load i16, ptr %25, !tbaa !2
  %112 = load i16, ptr %25, !tbaa !2
  %113 = sub i16 %112, 0
  %114 = getelementptr inbounds float, ptr @"VELX!", i16 %113
  %115 = load float, ptr %114, !tbaa !2
  %116 = load float, ptr %28, !tbaa !2
  %117 = fadd float %115, %116
  %118 = fpround float %117 to float
  %119 = sub i16 %111, 0
  %120 = getelementptr inbounds float, ptr @"VELX!", i16 %119
  store float %118, ptr %120, !tbaa !2
  %121 = load i16, ptr %25, !tbaa !2
  %122 = load i16, ptr %25, !tbaa !2
  %123 = sub i16 %122, 0
  %124 = getelementptr inbounds float, ptr @"VELY!", i16 %123
  %125 = load float, ptr %124, !tbaa !2
  %126 = load float, ptr %27, !tbaa !2
  %127 = fadd float %125, %126
  %128 = fpround float %127 to float
  %129 = sub i16 %121, 0
  %130 = getelementptr inbounds float, ptr @"VELY!", i16 %129
  store float %128, ptr %130, !tbaa !2
  %131 = load i16, ptr %25, !tbaa !2
  %132 = load i16, ptr %25, !tbaa !2
  %133 = sub i16 %132, 0
  %134 = getelementptr inbounds float, ptr @"VELX!", i16 %133
  %135 = load float, ptr %134, !tbaa !2
  %136 = load i16, ptr %25, !tbaa !2
  %137 = sub i16 %136, 0
  %138 = getelementptr inbounds float, ptr @"VELX!", i16 %137
  %139 = load float, ptr %138, !tbaa !2
  %140 = load float, ptr @$float8, !tbaa !2
  %141 = fdiv float %139, %140
  %142 = fsub float %135, %141
  %143 = fpround float %142 to float
  %144 = sub i16 %131, 0
  %145 = getelementptr inbounds float, ptr @"VELX!", i16 %144
  store float %143, ptr %145, !tbaa !2
  %146 = load i16, ptr %25, !tbaa !2
  %147 = load i16, ptr %25, !tbaa !2
  %148 = sub i16 %147, 0
  %149 = getelementptr inbounds float, ptr @"VELY!", i16 %148
  %150 = load float, ptr %149, !tbaa !2
  %151 = load i16, ptr %25, !tbaa !2
  %152 = sub i16 %151, 0
  %153 = getelementptr inbounds float, ptr @"VELY!", i16 %152
  %154 = load float, ptr %153, !tbaa !2
  %155 = load float, ptr @$float8, !tbaa !2
  %156 = fdiv float %154, %155
  %157 = fsub float %150, %156
  %158 = fpround float %157 to float
  %159 = sub i16 %146, 0
  %160 = getelementptr inbounds float, ptr @"VELY!", i16 %159
  store float %158, ptr %160, !tbaa !2
  %161 = load i16, ptr %25, !tbaa !2
  %162 = load i16, ptr %14, !tbaa !2
  %163 = add nsw i16 %161, %162
  store i16 %163, ptr %25, !tbaa !2
  br label %b12

b22:
  %164 = load i16, ptr %11, !tbaa !2
  %165 = sub i16 %164, 0
  %166 = getelementptr inbounds float, ptr @"POSX!", i16 %165
  %167 = load float, ptr %166, !tbaa !2
  %168 = load i16, ptr %25, !tbaa !2
  %169 = sub i16 %168, 0
  %170 = getelementptr inbounds float, ptr @"POSX!", i16 %169
  %171 = load float, ptr %170, !tbaa !2
  %172 = fsub float %167, %171
  %173 = fpround float %172 to float
  store float %173, ptr %32, !tbaa !2
  %174 = load i16, ptr %11, !tbaa !2
  %175 = sub i16 %174, 0
  %176 = getelementptr inbounds float, ptr @"POSY!", i16 %175
  %177 = load float, ptr %176, !tbaa !2
  %178 = load i16, ptr %25, !tbaa !2
  %179 = sub i16 %178, 0
  %180 = getelementptr inbounds float, ptr @"POSY!", i16 %179
  %181 = load float, ptr %180, !tbaa !2
  %182 = fsub float %177, %181
  %183 = fpround float %182 to float
  store float %183, ptr %31, !tbaa !2
  %184 = load float, ptr %32, !tbaa !2
  %185 = load float, ptr %32, !tbaa !2
  %186 = fmul float %184, %185
  %187 = load float, ptr %31, !tbaa !2
  %188 = load float, ptr %31, !tbaa !2
  %189 = fmul float %187, %188
  %190 = fadd float %186, %189
  %191 = load float, ptr @$float7, !tbaa !2
  %192 = fadd float %190, %191
  %193 = fpround float %192 to float
  store float %193, ptr %30, !tbaa !2
  %194 = load float, ptr @$float7, !tbaa !2
  %195 = load float, ptr %30, !tbaa !2
  %196 = load float, ptr @$float7, !tbaa !2
  %197 = fadd float %195, %196
  %198 = fdiv float %194, %197
  %199 = fpround float %198 to float
  store float %199, ptr %29, !tbaa !2
  %200 = load float, ptr %28, !tbaa !2
  %201 = load float, ptr %32, !tbaa !2
  %202 = load float, ptr %29, !tbaa !2
  %203 = fmul float %201, %202
  %204 = fadd float %200, %203
  %205 = fpround float %204 to float
  store float %205, ptr %28, !tbaa !2
  %206 = load float, ptr %27, !tbaa !2
  %207 = load float, ptr %31, !tbaa !2
  %208 = load float, ptr %29, !tbaa !2
  %209 = fmul float %207, %208
  %210 = fadd float %206, %209
  %211 = fpround float %210 to float
  store float %211, ptr %27, !tbaa !2
  br label %b24

b23:
  br label %b24

b24:
  %212 = load i16, ptr %11, !tbaa !2
  %213 = load i16, ptr %9, !tbaa !2
  %214 = add nsw i16 %212, %213
  store i16 %214, ptr %11, !tbaa !2
  br label %b17

b25:
  %215 = load i16, ptr %7, !tbaa !2
  %216 = icmp sge i16 %215, 0
  br i1 %216, label %b26, label %b27

b26:
  %217 = load i16, ptr %25, !tbaa !2
  %218 = load i16, ptr %8, !tbaa !2
  %219 = icmp sle i16 %217, %218
  br i1 %219, label %b28, label %b29

b27:
  %220 = load i16, ptr %25, !tbaa !2
  %221 = load i16, ptr %8, !tbaa !2
  %222 = icmp sge i16 %220, %221
  br i1 %222, label %b28, label %b29

b28:
  %223 = load i16, ptr %25, !tbaa !2
  %224 = load i16, ptr %25, !tbaa !2
  %225 = sub i16 %224, 0
  %226 = getelementptr inbounds float, ptr @"POSX!", i16 %225
  %227 = load float, ptr %226, !tbaa !2
  %228 = load i16, ptr %25, !tbaa !2
  %229 = sub i16 %228, 0
  %230 = getelementptr inbounds float, ptr @"VELX!", i16 %229
  %231 = load float, ptr %230, !tbaa !2
  %232 = fadd float %227, %231
  %233 = fpround float %232 to float
  %234 = sub i16 %223, 0
  %235 = getelementptr inbounds float, ptr @"POSX!", i16 %234
  store float %233, ptr %235, !tbaa !2
  %236 = load i16, ptr %25, !tbaa !2
  %237 = load i16, ptr %25, !tbaa !2
  %238 = sub i16 %237, 0
  %239 = getelementptr inbounds float, ptr @"POSY!", i16 %238
  %240 = load float, ptr %239, !tbaa !2
  %241 = load i16, ptr %25, !tbaa !2
  %242 = sub i16 %241, 0
  %243 = getelementptr inbounds float, ptr @"VELY!", i16 %242
  %244 = load float, ptr %243, !tbaa !2
  %245 = fadd float %240, %244
  %246 = fpround float %245 to float
  %247 = sub i16 %236, 0
  %248 = getelementptr inbounds float, ptr @"POSY!", i16 %247
  store float %246, ptr %248, !tbaa !2
  %249 = load i16, ptr %25, !tbaa !2
  %250 = load i16, ptr %7, !tbaa !2
  %251 = add nsw i16 %249, %250
  store i16 %251, ptr %25, !tbaa !2
  br label %b25

b29:
  %252 = load i16, ptr %18, !tbaa !2
  %253 = load i16, ptr %16, !tbaa !2
  %254 = add nsw i16 %252, %253
  store i16 %254, ptr %18, !tbaa !2
  br label %b7

b30:
  %255 = load i16, ptr %4, !tbaa !2
  %256 = icmp sge i16 %255, 0
  br i1 %256, label %b31, label %b32

b31:
  %257 = load i16, ptr %25, !tbaa !2
  %258 = load i16, ptr %5, !tbaa !2
  %259 = icmp sle i16 %257, %258
  br i1 %259, label %b33, label %b34

b32:
  %260 = load i16, ptr %25, !tbaa !2
  %261 = load i16, ptr %5, !tbaa !2
  %262 = icmp sge i16 %260, %261
  br i1 %262, label %b33, label %b34

b33:
  %263 = load float, ptr %26, !tbaa !2
  %264 = load i16, ptr %25, !tbaa !2
  %265 = add i16 %264, 1
  %266 = load i16, ptr %25, !tbaa !2
  %267 = sub i16 %266, 0
  %268 = getelementptr inbounds float, ptr @"POSX!", i16 %267
  %269 = load float, ptr %268, !tbaa !2
  %270 = load i16, ptr %25, !tbaa !2
  %271 = sub i16 %270, 0
  %272 = getelementptr inbounds float, ptr @"POSY!", i16 %271
  %273 = load float, ptr %272, !tbaa !2
  store i16 3, ptr %3, !tbaa !2
  %274 = load i16, ptr %3, !tbaa !2
  %275 = sitofp i16 %274 to float
  %276 = fmul float %275, %273
  %277 = fadd float %269, %276
  %278 = load i16, ptr %25, !tbaa !2
  %279 = sub i16 %278, 0
  %280 = getelementptr inbounds float, ptr @"VELX!", i16 %279
  %281 = load float, ptr %280, !tbaa !2
  store i16 5, ptr %2, !tbaa !2
  %282 = load i16, ptr %2, !tbaa !2
  %283 = sitofp i16 %282 to float
  %284 = fmul float %283, %281
  %285 = fadd float %277, %284
  %286 = load i16, ptr %25, !tbaa !2
  %287 = sub i16 %286, 0
  %288 = getelementptr inbounds float, ptr @"VELY!", i16 %287
  %289 = load float, ptr %288, !tbaa !2
  store i16 7, ptr %1, !tbaa !2
  %290 = load i16, ptr %1, !tbaa !2
  %291 = sitofp i16 %290 to float
  %292 = fmul float %291, %289
  %293 = fadd float %285, %292
  store i16 %265, ptr %0, !tbaa !2
  %294 = load i16, ptr %0, !tbaa !2
  %295 = sitofp i16 %294 to float
  %296 = fmul float %295, %293
  %297 = fadd float %263, %296
  %298 = fpround float %297 to float
  store float %298, ptr %26, !tbaa !2
  %299 = load i16, ptr %25, !tbaa !2
  %300 = load i16, ptr %4, !tbaa !2
  %301 = add nsw i16 %299, %300
  store i16 %301, ptr %25, !tbaa !2
  br label %b30

b34:
  %302 = load float, ptr %26, !tbaa !2
  %303 = load float, ptr @$float9, !tbaa !2
  %304 = fmul float %302, %303
  %305 = call i32 @llvm.lrint.i32.f32(float %304)
  store i32 %305, ptr %33, !tbaa !2
  br label %b35

b35:
  %306 = load i32, ptr %33, !tbaa !2
  ret i32 %306
}

declare cc 1000 ptr @llrm.qb.B$STI4(i32) addrspace(1)

declare cc 1000 ptr @llrm.qb.B$LTRM(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$PESD(ptr) addrspace(1)

declare cc 1000 void @llrm.qb.B$CEND() addrspace(1)

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
