target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"POSX!" = internal global [28 x i8] zeroinitializer, align 2
@"POSY!" = internal global [28 x i8] zeroinitializer, align 2
@"VELX!" = internal global [28 x i8] zeroinitializer, align 2
@"VELY!" = internal global [28 x i8] zeroinitializer, align 2

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  %0 = tail call cc 1000 addrspace(1) i32 @"BENCHNBODYSINGLE&"()
  %1 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %0)
  %2 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %1)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %2)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable
}

define cc 1000 i32 @"BENCHNBODYSINGLE&"() addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: none, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  tail call void @llvm.memset.p0.i16(ptr align 4 @"VELX!", i8 0, i16 24, i1 false)
  tail call void @llvm.memset.p0.i16(ptr align 4 @"VELY!", i8 0, i16 24, i1 false)
  store float -1.500000e+01, ptr @"POSX!", !tbaa !2
  store float -1.200000e+01, ptr @"POSY!", !tbaa !2
  store float -8.000000e+00, ptr getelementptr inbounds nuw (i8, ptr @"POSX!", i16 4), !tbaa !2
  store float -7.000000e+00, ptr getelementptr inbounds nuw (i8, ptr @"POSY!", i16 4), !tbaa !2
  store float -1.000000e+00, ptr getelementptr inbounds nuw (i8, ptr @"POSX!", i16 8), !tbaa !2
  store float -2.000000e+00, ptr getelementptr inbounds nuw (i8, ptr @"POSY!", i16 8), !tbaa !2
  store float 6.000000e+00, ptr getelementptr inbounds nuw (i8, ptr @"POSX!", i16 12), !tbaa !2
  store float 3.000000e+00, ptr getelementptr inbounds nuw (i8, ptr @"POSY!", i16 12), !tbaa !2
  store float 1.300000e+01, ptr getelementptr inbounds nuw (i8, ptr @"POSX!", i16 16), !tbaa !2
  store float 8.000000e+00, ptr getelementptr inbounds nuw (i8, ptr @"POSY!", i16 16), !tbaa !2
  store float 2.000000e+01, ptr getelementptr inbounds nuw (i8, ptr @"POSX!", i16 20), !tbaa !2
  store float 1.300000e+01, ptr getelementptr inbounds nuw (i8, ptr @"POSY!", i16 20), !tbaa !2
  br label %b10

b10:
  %0 = phi float [ 1.300000e+01, %b1 ], [ %105, %b16 ]
  %1 = phi float [ 2.000000e+01, %b1 ], [ %102, %b16 ]
  %2 = phi float [ 8.000000e+00, %b1 ], [ %99, %b16 ]
  %3 = phi float [ 1.300000e+01, %b1 ], [ %96, %b16 ]
  %4 = phi float [ 3.000000e+00, %b1 ], [ %93, %b16 ]
  %5 = phi float [ 6.000000e+00, %b1 ], [ %90, %b16 ]
  %6 = phi float [ -2.000000e+00, %b1 ], [ %87, %b16 ]
  %7 = phi float [ -1.000000e+00, %b1 ], [ %84, %b16 ]
  %8 = phi float [ -7.000000e+00, %b1 ], [ %81, %b16 ]
  %9 = phi float [ -8.000000e+00, %b1 ], [ %78, %b16 ]
  %10 = phi float [ -1.200000e+01, %b1 ], [ %75, %b16 ]
  %11 = phi float [ -1.500000e+01, %b1 ], [ %72, %b16 ]
  %12 = phi i16 [ 1, %b1 ], [ %106, %b16 ]
  br label %b15

b11:
  %13 = fmul float %75, 3.000000e+00
  %14 = fadd float %13, %72
  %15 = fmul float %70, 5.000000e+00
  %16 = fadd float %14, %15
  %17 = fmul float %73, 7.000000e+00
  %18 = fadd float %16, %17
  %19 = fadd float %18, 0.000000e+00
  %20 = fpround float %19 to float
  %21 = fmul float %81, 3.000000e+00
  %22 = fadd float %21, %78
  %23 = fmul float %76, 5.000000e+00
  %24 = fadd float %22, %23
  %25 = fmul float %79, 7.000000e+00
  %26 = fadd float %24, %25
  %27 = fmul float %26, 2.000000e+00
  %28 = fadd float %27, %20
  %29 = fpround float %28 to float
  %30 = fmul float %87, 3.000000e+00
  %31 = fadd float %30, %84
  %32 = fmul float %82, 5.000000e+00
  %33 = fadd float %31, %32
  %34 = fmul float %85, 7.000000e+00
  %35 = fadd float %33, %34
  %36 = fmul float %35, 3.000000e+00
  %37 = fadd float %36, %29
  %38 = fpround float %37 to float
  %39 = fmul float %93, 3.000000e+00
  %40 = fadd float %39, %90
  %41 = fmul float %88, 5.000000e+00
  %42 = fadd float %40, %41
  %43 = fmul float %91, 7.000000e+00
  %44 = fadd float %42, %43
  %45 = fmul float %44, 4.000000e+00
  %46 = fadd float %45, %38
  %47 = fpround float %46 to float
  %48 = fmul float %99, 3.000000e+00
  %49 = fadd float %48, %96
  %50 = fmul float %94, 5.000000e+00
  %51 = fadd float %49, %50
  %52 = fmul float %97, 7.000000e+00
  %53 = fadd float %51, %52
  %54 = fmul float %53, 5.000000e+00
  %55 = fadd float %54, %47
  %56 = fpround float %55 to float
  %57 = fmul float %105, 3.000000e+00
  %58 = fadd float %57, %102
  %59 = fmul float %100, 5.000000e+00
  %60 = fadd float %58, %59
  %61 = fmul float %103, 7.000000e+00
  %62 = fadd float %60, %61
  %63 = fmul float %62, 6.000000e+00
  %64 = fadd float %63, %56
  %65 = fpround float %64 to float
  %66 = fmul float %65, 1.024000e+03
  %67 = tail call i32 @llvm.lrint.i32.f32(float %66)
  ret i32 %67

b15:
  %68 = phi i16 [ %290, %271 ], [ -24, %b10 ]
  %69 = icmp eq i16 %68, -24
  br i1 %69, label %132, label %b24

b16:
  %70 = load float, ptr @"VELX!", !tbaa !2
  %71 = fadd float %11, %70
  %72 = fpround float %71 to float
  store float %72, ptr @"POSX!", !tbaa !2
  %73 = load float, ptr @"VELY!", !tbaa !2
  %74 = fadd float %10, %73
  %75 = fpround float %74 to float
  store float %75, ptr @"POSY!", !tbaa !2
  %76 = load float, ptr getelementptr inbounds nuw (i8, ptr @"VELX!", i16 4), !tbaa !2
  %77 = fadd float %9, %76
  %78 = fpround float %77 to float
  store float %78, ptr getelementptr inbounds nuw (i8, ptr @"POSX!", i16 4), !tbaa !2
  %79 = load float, ptr getelementptr inbounds nuw (i8, ptr @"VELY!", i16 4), !tbaa !2
  %80 = fadd float %8, %79
  %81 = fpround float %80 to float
  store float %81, ptr getelementptr inbounds nuw (i8, ptr @"POSY!", i16 4), !tbaa !2
  %82 = load float, ptr getelementptr inbounds nuw (i8, ptr @"VELX!", i16 8), !tbaa !2
  %83 = fadd float %7, %82
  %84 = fpround float %83 to float
  store float %84, ptr getelementptr inbounds nuw (i8, ptr @"POSX!", i16 8), !tbaa !2
  %85 = load float, ptr getelementptr inbounds nuw (i8, ptr @"VELY!", i16 8), !tbaa !2
  %86 = fadd float %6, %85
  %87 = fpround float %86 to float
  store float %87, ptr getelementptr inbounds nuw (i8, ptr @"POSY!", i16 8), !tbaa !2
  %88 = load float, ptr getelementptr inbounds nuw (i8, ptr @"VELX!", i16 12), !tbaa !2
  %89 = fadd float %5, %88
  %90 = fpround float %89 to float
  store float %90, ptr getelementptr inbounds nuw (i8, ptr @"POSX!", i16 12), !tbaa !2
  %91 = load float, ptr getelementptr inbounds nuw (i8, ptr @"VELY!", i16 12), !tbaa !2
  %92 = fadd float %4, %91
  %93 = fpround float %92 to float
  store float %93, ptr getelementptr inbounds nuw (i8, ptr @"POSY!", i16 12), !tbaa !2
  %94 = load float, ptr getelementptr inbounds nuw (i8, ptr @"VELX!", i16 16), !tbaa !2
  %95 = fadd float %3, %94
  %96 = fpround float %95 to float
  store float %96, ptr getelementptr inbounds nuw (i8, ptr @"POSX!", i16 16), !tbaa !2
  %97 = load float, ptr getelementptr inbounds nuw (i8, ptr @"VELY!", i16 16), !tbaa !2
  %98 = fadd float %2, %97
  %99 = fpround float %98 to float
  store float %99, ptr getelementptr inbounds nuw (i8, ptr @"POSY!", i16 16), !tbaa !2
  %100 = load float, ptr getelementptr inbounds nuw (i8, ptr @"VELX!", i16 20), !tbaa !2
  %101 = fadd float %1, %100
  %102 = fpround float %101 to float
  store float %102, ptr getelementptr inbounds nuw (i8, ptr @"POSX!", i16 20), !tbaa !2
  %103 = load float, ptr getelementptr inbounds nuw (i8, ptr @"VELY!", i16 20), !tbaa !2
  %104 = fadd float %0, %103
  %105 = fpround float %104 to float
  store float %105, ptr getelementptr inbounds nuw (i8, ptr @"POSY!", i16 20), !tbaa !2
  %106 = add nuw nsw i16 %12, 1
  %exitcond4 = icmp eq i16 %106, 1001
  br i1 %exitcond4, label %b11, label %b10

b24:
  %107 = getelementptr i8, ptr @"POSX!", i16 %68
  %108 = getelementptr i8, ptr %107, i16 24
  %109 = load float, ptr %108, !tbaa !2
  %110 = fsub float %11, %109
  %111 = fpround float %110 to float
  %112 = getelementptr i8, ptr @"POSY!", i16 %68
  %113 = getelementptr i8, ptr %112, i16 24
  %114 = load float, ptr %113, !tbaa !2
  %115 = fsub float %10, %114
  %116 = fpround float %115 to float
  %117 = fmul float %111, %111
  %118 = fmul float %116, %116
  %119 = fadd float %117, %118
  %120 = fadd float %119, 1.000000e+00
  %121 = fpround float %120 to float
  %122 = fadd float %121, 1.000000e+00
  %123 = fdiv float 1.000000e+00, %122
  %124 = fpround float %123 to float
  %125 = fmul float %111, %124
  %126 = fadd float %125, 0.000000e+00
  %127 = fpround float %126 to float
  %128 = fmul float %116, %124
  %129 = fadd float %128, 0.000000e+00
  %130 = fpround float %129 to float
  %131 = icmp eq i16 %68, -20
  br i1 %131, label %160, label %132

132:
  %133 = phi float [ %127, %b24 ], [ 0.000000e+00, %b15 ]
  %134 = phi float [ %130, %b24 ], [ 0.000000e+00, %b15 ]
  %135 = getelementptr i8, ptr @"POSX!", i16 %68
  %136 = getelementptr i8, ptr %135, i16 24
  %137 = load float, ptr %136, !tbaa !2
  %138 = fsub float %9, %137
  %139 = fpround float %138 to float
  %140 = getelementptr i8, ptr @"POSY!", i16 %68
  %141 = getelementptr i8, ptr %140, i16 24
  %142 = load float, ptr %141, !tbaa !2
  %143 = fsub float %8, %142
  %144 = fpround float %143 to float
  %145 = fmul float %139, %139
  %146 = fmul float %144, %144
  %147 = fadd float %145, %146
  %148 = fadd float %147, 1.000000e+00
  %149 = fpround float %148 to float
  %150 = fadd float %149, 1.000000e+00
  %151 = fdiv float 1.000000e+00, %150
  %152 = fpround float %151 to float
  %153 = fmul float %139, %152
  %154 = fadd float %133, %153
  %155 = fpround float %154 to float
  %156 = fmul float %144, %152
  %157 = fadd float %134, %156
  %158 = fpround float %157 to float
  %159 = icmp eq i16 %68, -16
  br i1 %159, label %188, label %160

160:
  %161 = phi float [ %155, %132 ], [ %127, %b24 ]
  %162 = phi float [ %158, %132 ], [ %130, %b24 ]
  %163 = getelementptr i8, ptr @"POSX!", i16 %68
  %164 = getelementptr i8, ptr %163, i16 24
  %165 = load float, ptr %164, !tbaa !2
  %166 = fsub float %7, %165
  %167 = fpround float %166 to float
  %168 = getelementptr i8, ptr @"POSY!", i16 %68
  %169 = getelementptr i8, ptr %168, i16 24
  %170 = load float, ptr %169, !tbaa !2
  %171 = fsub float %6, %170
  %172 = fpround float %171 to float
  %173 = fmul float %167, %167
  %174 = fmul float %172, %172
  %175 = fadd float %173, %174
  %176 = fadd float %175, 1.000000e+00
  %177 = fpround float %176 to float
  %178 = fadd float %177, 1.000000e+00
  %179 = fdiv float 1.000000e+00, %178
  %180 = fpround float %179 to float
  %181 = fmul float %167, %180
  %182 = fadd float %161, %181
  %183 = fpround float %182 to float
  %184 = fmul float %172, %180
  %185 = fadd float %162, %184
  %186 = fpround float %185 to float
  %187 = icmp eq i16 %68, -12
  br i1 %187, label %216, label %188

188:
  %189 = phi float [ %183, %160 ], [ %155, %132 ]
  %190 = phi float [ %186, %160 ], [ %158, %132 ]
  %191 = getelementptr i8, ptr @"POSX!", i16 %68
  %192 = getelementptr i8, ptr %191, i16 24
  %193 = load float, ptr %192, !tbaa !2
  %194 = fsub float %5, %193
  %195 = fpround float %194 to float
  %196 = getelementptr i8, ptr @"POSY!", i16 %68
  %197 = getelementptr i8, ptr %196, i16 24
  %198 = load float, ptr %197, !tbaa !2
  %199 = fsub float %4, %198
  %200 = fpround float %199 to float
  %201 = fmul float %195, %195
  %202 = fmul float %200, %200
  %203 = fadd float %201, %202
  %204 = fadd float %203, 1.000000e+00
  %205 = fpround float %204 to float
  %206 = fadd float %205, 1.000000e+00
  %207 = fdiv float 1.000000e+00, %206
  %208 = fpround float %207 to float
  %209 = fmul float %195, %208
  %210 = fadd float %189, %209
  %211 = fpround float %210 to float
  %212 = fmul float %200, %208
  %213 = fadd float %190, %212
  %214 = fpround float %213 to float
  %215 = icmp eq i16 %68, -8
  br i1 %215, label %244, label %216

216:
  %217 = phi float [ %211, %188 ], [ %183, %160 ]
  %218 = phi float [ %214, %188 ], [ %186, %160 ]
  %219 = getelementptr i8, ptr @"POSX!", i16 %68
  %220 = getelementptr i8, ptr %219, i16 24
  %221 = load float, ptr %220, !tbaa !2
  %222 = fsub float %3, %221
  %223 = fpround float %222 to float
  %224 = getelementptr i8, ptr @"POSY!", i16 %68
  %225 = getelementptr i8, ptr %224, i16 24
  %226 = load float, ptr %225, !tbaa !2
  %227 = fsub float %2, %226
  %228 = fpround float %227 to float
  %229 = fmul float %223, %223
  %230 = fmul float %228, %228
  %231 = fadd float %229, %230
  %232 = fadd float %231, 1.000000e+00
  %233 = fpround float %232 to float
  %234 = fadd float %233, 1.000000e+00
  %235 = fdiv float 1.000000e+00, %234
  %236 = fpround float %235 to float
  %237 = fmul float %223, %236
  %238 = fadd float %217, %237
  %239 = fpround float %238 to float
  %240 = fmul float %228, %236
  %241 = fadd float %218, %240
  %242 = fpround float %241 to float
  %243 = icmp eq i16 %68, -4
  br i1 %243, label %271, label %244

244:
  %245 = phi float [ %239, %216 ], [ %211, %188 ]
  %246 = phi float [ %242, %216 ], [ %214, %188 ]
  %247 = getelementptr i8, ptr @"POSX!", i16 %68
  %248 = getelementptr i8, ptr %247, i16 24
  %249 = load float, ptr %248, !tbaa !2
  %250 = fsub float %1, %249
  %251 = fpround float %250 to float
  %252 = getelementptr i8, ptr @"POSY!", i16 %68
  %253 = getelementptr i8, ptr %252, i16 24
  %254 = load float, ptr %253, !tbaa !2
  %255 = fsub float %0, %254
  %256 = fpround float %255 to float
  %257 = fmul float %251, %251
  %258 = fmul float %256, %256
  %259 = fadd float %257, %258
  %260 = fadd float %259, 1.000000e+00
  %261 = fpround float %260 to float
  %262 = fadd float %261, 1.000000e+00
  %263 = fdiv float 1.000000e+00, %262
  %264 = fpround float %263 to float
  %265 = fmul float %251, %264
  %266 = fadd float %245, %265
  %267 = fpround float %266 to float
  %268 = fmul float %256, %264
  %269 = fadd float %246, %268
  %270 = fpround float %269 to float
  br label %271

271:
  %272 = phi float [ %270, %244 ], [ %242, %216 ]
  %273 = phi float [ %267, %244 ], [ %239, %216 ]
  %274 = getelementptr i8, ptr @"VELX!", i16 %68
  %275 = getelementptr i8, ptr %274, i16 24
  %276 = load float, ptr %275, !tbaa !2
  %277 = fadd float %273, %276
  %278 = fpround float %277 to float
  %279 = getelementptr i8, ptr @"VELY!", i16 %68
  %280 = getelementptr i8, ptr %279, i16 24
  %281 = load float, ptr %280, !tbaa !2
  %282 = fadd float %272, %281
  %283 = fpround float %282 to float
  %284 = fmul float %278, 1.250000e-01
  %285 = fsub float %278, %284
  %286 = fpround float %285 to float
  store float %286, ptr %275, !tbaa !2
  %287 = fmul float %283, 1.250000e-01
  %288 = fsub float %283, %287
  %289 = fpround float %288 to float
  store float %289, ptr %280, !tbaa !2
  %290 = add nsw i16 %68, 4
  %exitcond3 = icmp eq i16 %290, 0
  br i1 %exitcond3, label %b16, label %b15
}

declare cc 1005 ptr @llrm.qb.B$STI4(i32) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$LTRM(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare i32 @llvm.lrint.i32.f32(float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
