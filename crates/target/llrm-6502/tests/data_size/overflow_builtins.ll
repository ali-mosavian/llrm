target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8"
target triple = "msp430"

@sa = dso_local global [3 x i16] [i16 32000, i16 -32000, i16 7], align 2
@ua = dso_local global [3 x i16] [i16 -5536, i16 5, i16 300], align 2
@la = dso_local global [3 x i32] [i32 2000000000, i32 -2000000000, i32 70000], align 2
@ula = dso_local global [3 x i32] [i32 -294967296, i32 5, i32 70000], align 2
@qa = dso_local global [3 x i64] [i64 8070450532247928832, i64 -8070450532247928832, i64 3000000000], align 2
@uqa = dso_local global [3 x i64] [i64 -1152921504606846976, i64 5, i64 4000000000], align 2

; Function Attrs: nounwind optsize
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca ptr, align 2
  %3 = alloca ptr, align 2
  %4 = alloca ptr, align 2
  %5 = alloca ptr, align 2
  %6 = alloca ptr, align 2
  %7 = alloca ptr, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca i16, align 2
  %11 = alloca i16, align 2
  %12 = alloca i16, align 2
  %13 = alloca i16, align 2
  %14 = alloca i16, align 2
  %15 = alloca i16, align 2
  %16 = alloca i16, align 2
  %17 = alloca i16, align 2
  %18 = alloca i16, align 2
  %19 = alloca i16, align 2
  %20 = alloca i16, align 2
  %21 = alloca i16, align 2
  %22 = alloca i32, align 2
  %23 = alloca i16, align 2
  %24 = alloca i32, align 2
  %25 = alloca i16, align 2
  %26 = alloca i32, align 2
  %27 = alloca i16, align 2
  %28 = alloca i32, align 2
  %29 = alloca i16, align 2
  %30 = alloca i32, align 2
  %31 = alloca i16, align 2
  %32 = alloca i32, align 2
  %33 = alloca i16, align 2
  %34 = alloca i64, align 2
  %35 = alloca i16, align 2
  %36 = alloca i64, align 2
  %37 = alloca i16, align 2
  %38 = alloca i64, align 2
  %39 = alloca i16, align 2
  %40 = alloca i64, align 2
  %41 = alloca i16, align 2
  %42 = alloca i64, align 2
  %43 = alloca i16, align 2
  %44 = alloca i64, align 2
  %45 = alloca i16, align 2
  store i16 0, ptr %1, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #4
  store ptr @sa, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store ptr @ua, ptr %3, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %4) #4
  store ptr @la, ptr %4, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %5) #4
  store ptr @ula, ptr %5, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %6) #4
  store ptr @qa, ptr %6, align 2, !tbaa !12
  call void @llvm.lifetime.start.p0(ptr %7) #4
  store ptr @uqa, ptr %7, align 2, !tbaa !12
  call void @llvm.lifetime.start.p0(ptr %8) #4
  call void @llvm.lifetime.start.p0(ptr %9) #4
  store i16 0, ptr %8, align 2, !tbaa !14
  br label %46

46:                                               ; preds = %394, %0
  %47 = load i16, ptr %8, align 2, !tbaa !14
  %48 = icmp slt i16 %47, 3
  br i1 %48, label %49, label %397

49:                                               ; preds = %46
  store i16 0, ptr %9, align 2, !tbaa !14
  br label %50

50:                                               ; preds = %390, %49
  %51 = load i16, ptr %9, align 2, !tbaa !14
  %52 = icmp slt i16 %51, 3
  br i1 %52, label %53, label %393

53:                                               ; preds = %50
  br label %54

54:                                               ; preds = %53
  call void @llvm.lifetime.start.p0(ptr %10) #4
  call void @llvm.lifetime.start.p0(ptr %11) #4
  %55 = load ptr, ptr %2, align 2, !tbaa !7
  %56 = load i16, ptr %8, align 2, !tbaa !14
  %57 = getelementptr inbounds i16, ptr %55, i16 %56
  %58 = load volatile i16, ptr %57, align 2, !tbaa !15
  %59 = load ptr, ptr %2, align 2, !tbaa !7
  %60 = load i16, ptr %9, align 2, !tbaa !14
  %61 = getelementptr inbounds i16, ptr %59, i16 %60
  %62 = load volatile i16, ptr %61, align 2, !tbaa !15
  %63 = call { i16, i1 } @llvm.sadd.with.overflow.i16(i16 %58, i16 %62)
  %64 = extractvalue { i16, i1 } %63, 1
  %65 = extractvalue { i16, i1 } %63, 0
  store i16 %65, ptr %10, align 2
  %66 = zext i1 %64 to i16
  store i16 %66, ptr %11, align 2, !tbaa !14
  %67 = load i16, ptr %11, align 2, !tbaa !14
  %68 = sext i16 %67 to i32
  call void @report(i32 noundef %68) #5
  %69 = load i16, ptr %10, align 2, !tbaa !15
  %70 = sext i16 %69 to i32
  call void @report(i32 noundef %70) #5
  call void @llvm.lifetime.end.p0(ptr %11) #4
  call void @llvm.lifetime.end.p0(ptr %10) #4
  br label %71

71:                                               ; preds = %54
  br label %72

72:                                               ; preds = %71
  br label %73

73:                                               ; preds = %72
  call void @llvm.lifetime.start.p0(ptr %12) #4
  call void @llvm.lifetime.start.p0(ptr %13) #4
  %74 = load ptr, ptr %2, align 2, !tbaa !7
  %75 = load i16, ptr %8, align 2, !tbaa !14
  %76 = getelementptr inbounds i16, ptr %74, i16 %75
  %77 = load volatile i16, ptr %76, align 2, !tbaa !15
  %78 = load ptr, ptr %2, align 2, !tbaa !7
  %79 = load i16, ptr %9, align 2, !tbaa !14
  %80 = getelementptr inbounds i16, ptr %78, i16 %79
  %81 = load volatile i16, ptr %80, align 2, !tbaa !15
  %82 = call { i16, i1 } @llvm.ssub.with.overflow.i16(i16 %77, i16 %81)
  %83 = extractvalue { i16, i1 } %82, 1
  %84 = extractvalue { i16, i1 } %82, 0
  store i16 %84, ptr %12, align 2
  %85 = zext i1 %83 to i16
  store i16 %85, ptr %13, align 2, !tbaa !14
  %86 = load i16, ptr %13, align 2, !tbaa !14
  %87 = sext i16 %86 to i32
  call void @report(i32 noundef %87) #5
  %88 = load i16, ptr %12, align 2, !tbaa !15
  %89 = sext i16 %88 to i32
  call void @report(i32 noundef %89) #5
  call void @llvm.lifetime.end.p0(ptr %13) #4
  call void @llvm.lifetime.end.p0(ptr %12) #4
  br label %90

90:                                               ; preds = %73
  br label %91

91:                                               ; preds = %90
  br label %92

92:                                               ; preds = %91
  call void @llvm.lifetime.start.p0(ptr %14) #4
  call void @llvm.lifetime.start.p0(ptr %15) #4
  %93 = load ptr, ptr %2, align 2, !tbaa !7
  %94 = load i16, ptr %8, align 2, !tbaa !14
  %95 = getelementptr inbounds i16, ptr %93, i16 %94
  %96 = load volatile i16, ptr %95, align 2, !tbaa !15
  %97 = load ptr, ptr %2, align 2, !tbaa !7
  %98 = load i16, ptr %9, align 2, !tbaa !14
  %99 = getelementptr inbounds i16, ptr %97, i16 %98
  %100 = load volatile i16, ptr %99, align 2, !tbaa !15
  %101 = call { i16, i1 } @llvm.smul.with.overflow.i16(i16 %96, i16 %100)
  %102 = extractvalue { i16, i1 } %101, 1
  %103 = extractvalue { i16, i1 } %101, 0
  store i16 %103, ptr %14, align 2
  %104 = zext i1 %102 to i16
  store i16 %104, ptr %15, align 2, !tbaa !14
  %105 = load i16, ptr %15, align 2, !tbaa !14
  %106 = sext i16 %105 to i32
  call void @report(i32 noundef %106) #5
  %107 = load i16, ptr %14, align 2, !tbaa !15
  %108 = sext i16 %107 to i32
  call void @report(i32 noundef %108) #5
  call void @llvm.lifetime.end.p0(ptr %15) #4
  call void @llvm.lifetime.end.p0(ptr %14) #4
  br label %109

109:                                              ; preds = %92
  br label %110

110:                                              ; preds = %109
  br label %111

111:                                              ; preds = %110
  call void @llvm.lifetime.start.p0(ptr %16) #4
  call void @llvm.lifetime.start.p0(ptr %17) #4
  %112 = load ptr, ptr %3, align 2, !tbaa !7
  %113 = load i16, ptr %8, align 2, !tbaa !14
  %114 = getelementptr inbounds i16, ptr %112, i16 %113
  %115 = load volatile i16, ptr %114, align 2, !tbaa !15
  %116 = load ptr, ptr %3, align 2, !tbaa !7
  %117 = load i16, ptr %9, align 2, !tbaa !14
  %118 = getelementptr inbounds i16, ptr %116, i16 %117
  %119 = load volatile i16, ptr %118, align 2, !tbaa !15
  %120 = call { i16, i1 } @llvm.uadd.with.overflow.i16(i16 %115, i16 %119)
  %121 = extractvalue { i16, i1 } %120, 1
  %122 = extractvalue { i16, i1 } %120, 0
  store i16 %122, ptr %16, align 2
  %123 = zext i1 %121 to i16
  store i16 %123, ptr %17, align 2, !tbaa !14
  %124 = load i16, ptr %17, align 2, !tbaa !14
  %125 = sext i16 %124 to i32
  call void @report(i32 noundef %125) #5
  %126 = load i16, ptr %16, align 2, !tbaa !15
  %127 = zext i16 %126 to i32
  call void @report(i32 noundef %127) #5
  call void @llvm.lifetime.end.p0(ptr %17) #4
  call void @llvm.lifetime.end.p0(ptr %16) #4
  br label %128

128:                                              ; preds = %111
  br label %129

129:                                              ; preds = %128
  br label %130

130:                                              ; preds = %129
  call void @llvm.lifetime.start.p0(ptr %18) #4
  call void @llvm.lifetime.start.p0(ptr %19) #4
  %131 = load ptr, ptr %3, align 2, !tbaa !7
  %132 = load i16, ptr %8, align 2, !tbaa !14
  %133 = getelementptr inbounds i16, ptr %131, i16 %132
  %134 = load volatile i16, ptr %133, align 2, !tbaa !15
  %135 = load ptr, ptr %3, align 2, !tbaa !7
  %136 = load i16, ptr %9, align 2, !tbaa !14
  %137 = getelementptr inbounds i16, ptr %135, i16 %136
  %138 = load volatile i16, ptr %137, align 2, !tbaa !15
  %139 = call { i16, i1 } @llvm.usub.with.overflow.i16(i16 %134, i16 %138)
  %140 = extractvalue { i16, i1 } %139, 1
  %141 = extractvalue { i16, i1 } %139, 0
  store i16 %141, ptr %18, align 2
  %142 = zext i1 %140 to i16
  store i16 %142, ptr %19, align 2, !tbaa !14
  %143 = load i16, ptr %19, align 2, !tbaa !14
  %144 = sext i16 %143 to i32
  call void @report(i32 noundef %144) #5
  %145 = load i16, ptr %18, align 2, !tbaa !15
  %146 = zext i16 %145 to i32
  call void @report(i32 noundef %146) #5
  call void @llvm.lifetime.end.p0(ptr %19) #4
  call void @llvm.lifetime.end.p0(ptr %18) #4
  br label %147

147:                                              ; preds = %130
  br label %148

148:                                              ; preds = %147
  br label %149

149:                                              ; preds = %148
  call void @llvm.lifetime.start.p0(ptr %20) #4
  call void @llvm.lifetime.start.p0(ptr %21) #4
  %150 = load ptr, ptr %3, align 2, !tbaa !7
  %151 = load i16, ptr %8, align 2, !tbaa !14
  %152 = getelementptr inbounds i16, ptr %150, i16 %151
  %153 = load volatile i16, ptr %152, align 2, !tbaa !15
  %154 = load ptr, ptr %3, align 2, !tbaa !7
  %155 = load i16, ptr %9, align 2, !tbaa !14
  %156 = getelementptr inbounds i16, ptr %154, i16 %155
  %157 = load volatile i16, ptr %156, align 2, !tbaa !15
  %158 = call { i16, i1 } @llvm.umul.with.overflow.i16(i16 %153, i16 %157)
  %159 = extractvalue { i16, i1 } %158, 1
  %160 = extractvalue { i16, i1 } %158, 0
  store i16 %160, ptr %20, align 2
  %161 = zext i1 %159 to i16
  store i16 %161, ptr %21, align 2, !tbaa !14
  %162 = load i16, ptr %21, align 2, !tbaa !14
  %163 = sext i16 %162 to i32
  call void @report(i32 noundef %163) #5
  %164 = load i16, ptr %20, align 2, !tbaa !15
  %165 = zext i16 %164 to i32
  call void @report(i32 noundef %165) #5
  call void @llvm.lifetime.end.p0(ptr %21) #4
  call void @llvm.lifetime.end.p0(ptr %20) #4
  br label %166

166:                                              ; preds = %149
  br label %167

167:                                              ; preds = %166
  br label %168

168:                                              ; preds = %167
  call void @llvm.lifetime.start.p0(ptr %22) #4
  call void @llvm.lifetime.start.p0(ptr %23) #4
  %169 = load ptr, ptr %4, align 2, !tbaa !10
  %170 = load i16, ptr %8, align 2, !tbaa !14
  %171 = getelementptr inbounds i32, ptr %169, i16 %170
  %172 = load volatile i32, ptr %171, align 2, !tbaa !17
  %173 = load ptr, ptr %4, align 2, !tbaa !10
  %174 = load i16, ptr %9, align 2, !tbaa !14
  %175 = getelementptr inbounds i32, ptr %173, i16 %174
  %176 = load volatile i32, ptr %175, align 2, !tbaa !17
  %177 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %172, i32 %176)
  %178 = extractvalue { i32, i1 } %177, 1
  %179 = extractvalue { i32, i1 } %177, 0
  store i32 %179, ptr %22, align 2
  %180 = zext i1 %178 to i16
  store i16 %180, ptr %23, align 2, !tbaa !14
  %181 = load i16, ptr %23, align 2, !tbaa !14
  %182 = sext i16 %181 to i32
  call void @report(i32 noundef %182) #5
  %183 = load i32, ptr %22, align 2, !tbaa !17
  call void @report(i32 noundef %183) #5
  call void @llvm.lifetime.end.p0(ptr %23) #4
  call void @llvm.lifetime.end.p0(ptr %22) #4
  br label %184

184:                                              ; preds = %168
  br label %185

185:                                              ; preds = %184
  br label %186

186:                                              ; preds = %185
  call void @llvm.lifetime.start.p0(ptr %24) #4
  call void @llvm.lifetime.start.p0(ptr %25) #4
  %187 = load ptr, ptr %4, align 2, !tbaa !10
  %188 = load i16, ptr %8, align 2, !tbaa !14
  %189 = getelementptr inbounds i32, ptr %187, i16 %188
  %190 = load volatile i32, ptr %189, align 2, !tbaa !17
  %191 = load ptr, ptr %4, align 2, !tbaa !10
  %192 = load i16, ptr %9, align 2, !tbaa !14
  %193 = getelementptr inbounds i32, ptr %191, i16 %192
  %194 = load volatile i32, ptr %193, align 2, !tbaa !17
  %195 = call { i32, i1 } @llvm.ssub.with.overflow.i32(i32 %190, i32 %194)
  %196 = extractvalue { i32, i1 } %195, 1
  %197 = extractvalue { i32, i1 } %195, 0
  store i32 %197, ptr %24, align 2
  %198 = zext i1 %196 to i16
  store i16 %198, ptr %25, align 2, !tbaa !14
  %199 = load i16, ptr %25, align 2, !tbaa !14
  %200 = sext i16 %199 to i32
  call void @report(i32 noundef %200) #5
  %201 = load i32, ptr %24, align 2, !tbaa !17
  call void @report(i32 noundef %201) #5
  call void @llvm.lifetime.end.p0(ptr %25) #4
  call void @llvm.lifetime.end.p0(ptr %24) #4
  br label %202

202:                                              ; preds = %186
  br label %203

203:                                              ; preds = %202
  br label %204

204:                                              ; preds = %203
  call void @llvm.lifetime.start.p0(ptr %26) #4
  call void @llvm.lifetime.start.p0(ptr %27) #4
  %205 = load ptr, ptr %4, align 2, !tbaa !10
  %206 = load i16, ptr %8, align 2, !tbaa !14
  %207 = getelementptr inbounds i32, ptr %205, i16 %206
  %208 = load volatile i32, ptr %207, align 2, !tbaa !17
  %209 = load ptr, ptr %4, align 2, !tbaa !10
  %210 = load i16, ptr %9, align 2, !tbaa !14
  %211 = getelementptr inbounds i32, ptr %209, i16 %210
  %212 = load volatile i32, ptr %211, align 2, !tbaa !17
  %213 = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %208, i32 %212)
  %214 = extractvalue { i32, i1 } %213, 1
  %215 = extractvalue { i32, i1 } %213, 0
  store i32 %215, ptr %26, align 2
  %216 = zext i1 %214 to i16
  store i16 %216, ptr %27, align 2, !tbaa !14
  %217 = load i16, ptr %27, align 2, !tbaa !14
  %218 = sext i16 %217 to i32
  call void @report(i32 noundef %218) #5
  %219 = load i32, ptr %26, align 2, !tbaa !17
  call void @report(i32 noundef %219) #5
  call void @llvm.lifetime.end.p0(ptr %27) #4
  call void @llvm.lifetime.end.p0(ptr %26) #4
  br label %220

220:                                              ; preds = %204
  br label %221

221:                                              ; preds = %220
  br label %222

222:                                              ; preds = %221
  call void @llvm.lifetime.start.p0(ptr %28) #4
  call void @llvm.lifetime.start.p0(ptr %29) #4
  %223 = load ptr, ptr %5, align 2, !tbaa !10
  %224 = load i16, ptr %8, align 2, !tbaa !14
  %225 = getelementptr inbounds i32, ptr %223, i16 %224
  %226 = load volatile i32, ptr %225, align 2, !tbaa !17
  %227 = load ptr, ptr %5, align 2, !tbaa !10
  %228 = load i16, ptr %9, align 2, !tbaa !14
  %229 = getelementptr inbounds i32, ptr %227, i16 %228
  %230 = load volatile i32, ptr %229, align 2, !tbaa !17
  %231 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %226, i32 %230)
  %232 = extractvalue { i32, i1 } %231, 1
  %233 = extractvalue { i32, i1 } %231, 0
  store i32 %233, ptr %28, align 2
  %234 = zext i1 %232 to i16
  store i16 %234, ptr %29, align 2, !tbaa !14
  %235 = load i16, ptr %29, align 2, !tbaa !14
  %236 = sext i16 %235 to i32
  call void @report(i32 noundef %236) #5
  %237 = load i32, ptr %28, align 2, !tbaa !17
  call void @report(i32 noundef %237) #5
  call void @llvm.lifetime.end.p0(ptr %29) #4
  call void @llvm.lifetime.end.p0(ptr %28) #4
  br label %238

238:                                              ; preds = %222
  br label %239

239:                                              ; preds = %238
  br label %240

240:                                              ; preds = %239
  call void @llvm.lifetime.start.p0(ptr %30) #4
  call void @llvm.lifetime.start.p0(ptr %31) #4
  %241 = load ptr, ptr %5, align 2, !tbaa !10
  %242 = load i16, ptr %8, align 2, !tbaa !14
  %243 = getelementptr inbounds i32, ptr %241, i16 %242
  %244 = load volatile i32, ptr %243, align 2, !tbaa !17
  %245 = load ptr, ptr %5, align 2, !tbaa !10
  %246 = load i16, ptr %9, align 2, !tbaa !14
  %247 = getelementptr inbounds i32, ptr %245, i16 %246
  %248 = load volatile i32, ptr %247, align 2, !tbaa !17
  %249 = call { i32, i1 } @llvm.usub.with.overflow.i32(i32 %244, i32 %248)
  %250 = extractvalue { i32, i1 } %249, 1
  %251 = extractvalue { i32, i1 } %249, 0
  store i32 %251, ptr %30, align 2
  %252 = zext i1 %250 to i16
  store i16 %252, ptr %31, align 2, !tbaa !14
  %253 = load i16, ptr %31, align 2, !tbaa !14
  %254 = sext i16 %253 to i32
  call void @report(i32 noundef %254) #5
  %255 = load i32, ptr %30, align 2, !tbaa !17
  call void @report(i32 noundef %255) #5
  call void @llvm.lifetime.end.p0(ptr %31) #4
  call void @llvm.lifetime.end.p0(ptr %30) #4
  br label %256

256:                                              ; preds = %240
  br label %257

257:                                              ; preds = %256
  br label %258

258:                                              ; preds = %257
  call void @llvm.lifetime.start.p0(ptr %32) #4
  call void @llvm.lifetime.start.p0(ptr %33) #4
  %259 = load ptr, ptr %5, align 2, !tbaa !10
  %260 = load i16, ptr %8, align 2, !tbaa !14
  %261 = getelementptr inbounds i32, ptr %259, i16 %260
  %262 = load volatile i32, ptr %261, align 2, !tbaa !17
  %263 = load ptr, ptr %5, align 2, !tbaa !10
  %264 = load i16, ptr %9, align 2, !tbaa !14
  %265 = getelementptr inbounds i32, ptr %263, i16 %264
  %266 = load volatile i32, ptr %265, align 2, !tbaa !17
  %267 = call { i32, i1 } @llvm.umul.with.overflow.i32(i32 %262, i32 %266)
  %268 = extractvalue { i32, i1 } %267, 1
  %269 = extractvalue { i32, i1 } %267, 0
  store i32 %269, ptr %32, align 2
  %270 = zext i1 %268 to i16
  store i16 %270, ptr %33, align 2, !tbaa !14
  %271 = load i16, ptr %33, align 2, !tbaa !14
  %272 = sext i16 %271 to i32
  call void @report(i32 noundef %272) #5
  %273 = load i32, ptr %32, align 2, !tbaa !17
  call void @report(i32 noundef %273) #5
  call void @llvm.lifetime.end.p0(ptr %33) #4
  call void @llvm.lifetime.end.p0(ptr %32) #4
  br label %274

274:                                              ; preds = %258
  br label %275

275:                                              ; preds = %274
  br label %276

276:                                              ; preds = %275
  call void @llvm.lifetime.start.p0(ptr %34) #4
  call void @llvm.lifetime.start.p0(ptr %35) #4
  %277 = load ptr, ptr %6, align 2, !tbaa !12
  %278 = load i16, ptr %8, align 2, !tbaa !14
  %279 = getelementptr inbounds i64, ptr %277, i16 %278
  %280 = load volatile i64, ptr %279, align 2, !tbaa !19
  %281 = load ptr, ptr %6, align 2, !tbaa !12
  %282 = load i16, ptr %9, align 2, !tbaa !14
  %283 = getelementptr inbounds i64, ptr %281, i16 %282
  %284 = load volatile i64, ptr %283, align 2, !tbaa !19
  %285 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %280, i64 %284)
  %286 = extractvalue { i64, i1 } %285, 1
  %287 = extractvalue { i64, i1 } %285, 0
  store i64 %287, ptr %34, align 2
  %288 = zext i1 %286 to i16
  store i16 %288, ptr %35, align 2, !tbaa !14
  %289 = load i16, ptr %35, align 2, !tbaa !14
  %290 = sext i16 %289 to i32
  call void @report(i32 noundef %290) #5
  %291 = load i64, ptr %34, align 2, !tbaa !19
  %292 = trunc i64 %291 to i32
  call void @report(i32 noundef %292) #5
  call void @llvm.lifetime.end.p0(ptr %35) #4
  call void @llvm.lifetime.end.p0(ptr %34) #4
  br label %293

293:                                              ; preds = %276
  br label %294

294:                                              ; preds = %293
  br label %295

295:                                              ; preds = %294
  call void @llvm.lifetime.start.p0(ptr %36) #4
  call void @llvm.lifetime.start.p0(ptr %37) #4
  %296 = load ptr, ptr %6, align 2, !tbaa !12
  %297 = load i16, ptr %8, align 2, !tbaa !14
  %298 = getelementptr inbounds i64, ptr %296, i16 %297
  %299 = load volatile i64, ptr %298, align 2, !tbaa !19
  %300 = load ptr, ptr %6, align 2, !tbaa !12
  %301 = load i16, ptr %9, align 2, !tbaa !14
  %302 = getelementptr inbounds i64, ptr %300, i16 %301
  %303 = load volatile i64, ptr %302, align 2, !tbaa !19
  %304 = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %299, i64 %303)
  %305 = extractvalue { i64, i1 } %304, 1
  %306 = extractvalue { i64, i1 } %304, 0
  store i64 %306, ptr %36, align 2
  %307 = zext i1 %305 to i16
  store i16 %307, ptr %37, align 2, !tbaa !14
  %308 = load i16, ptr %37, align 2, !tbaa !14
  %309 = sext i16 %308 to i32
  call void @report(i32 noundef %309) #5
  %310 = load i64, ptr %36, align 2, !tbaa !19
  %311 = trunc i64 %310 to i32
  call void @report(i32 noundef %311) #5
  call void @llvm.lifetime.end.p0(ptr %37) #4
  call void @llvm.lifetime.end.p0(ptr %36) #4
  br label %312

312:                                              ; preds = %295
  br label %313

313:                                              ; preds = %312
  br label %314

314:                                              ; preds = %313
  call void @llvm.lifetime.start.p0(ptr %38) #4
  call void @llvm.lifetime.start.p0(ptr %39) #4
  %315 = load ptr, ptr %6, align 2, !tbaa !12
  %316 = load i16, ptr %8, align 2, !tbaa !14
  %317 = getelementptr inbounds i64, ptr %315, i16 %316
  %318 = load volatile i64, ptr %317, align 2, !tbaa !19
  %319 = load ptr, ptr %6, align 2, !tbaa !12
  %320 = load i16, ptr %9, align 2, !tbaa !14
  %321 = getelementptr inbounds i64, ptr %319, i16 %320
  %322 = load volatile i64, ptr %321, align 2, !tbaa !19
  %323 = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %318, i64 %322)
  %324 = extractvalue { i64, i1 } %323, 1
  %325 = extractvalue { i64, i1 } %323, 0
  store i64 %325, ptr %38, align 2
  %326 = zext i1 %324 to i16
  store i16 %326, ptr %39, align 2, !tbaa !14
  %327 = load i16, ptr %39, align 2, !tbaa !14
  %328 = sext i16 %327 to i32
  call void @report(i32 noundef %328) #5
  %329 = load i64, ptr %38, align 2, !tbaa !19
  %330 = trunc i64 %329 to i32
  call void @report(i32 noundef %330) #5
  call void @llvm.lifetime.end.p0(ptr %39) #4
  call void @llvm.lifetime.end.p0(ptr %38) #4
  br label %331

331:                                              ; preds = %314
  br label %332

332:                                              ; preds = %331
  br label %333

333:                                              ; preds = %332
  call void @llvm.lifetime.start.p0(ptr %40) #4
  call void @llvm.lifetime.start.p0(ptr %41) #4
  %334 = load ptr, ptr %7, align 2, !tbaa !12
  %335 = load i16, ptr %8, align 2, !tbaa !14
  %336 = getelementptr inbounds i64, ptr %334, i16 %335
  %337 = load volatile i64, ptr %336, align 2, !tbaa !19
  %338 = load ptr, ptr %7, align 2, !tbaa !12
  %339 = load i16, ptr %9, align 2, !tbaa !14
  %340 = getelementptr inbounds i64, ptr %338, i16 %339
  %341 = load volatile i64, ptr %340, align 2, !tbaa !19
  %342 = call { i64, i1 } @llvm.uadd.with.overflow.i64(i64 %337, i64 %341)
  %343 = extractvalue { i64, i1 } %342, 1
  %344 = extractvalue { i64, i1 } %342, 0
  store i64 %344, ptr %40, align 2
  %345 = zext i1 %343 to i16
  store i16 %345, ptr %41, align 2, !tbaa !14
  %346 = load i16, ptr %41, align 2, !tbaa !14
  %347 = sext i16 %346 to i32
  call void @report(i32 noundef %347) #5
  %348 = load i64, ptr %40, align 2, !tbaa !19
  %349 = trunc i64 %348 to i32
  call void @report(i32 noundef %349) #5
  call void @llvm.lifetime.end.p0(ptr %41) #4
  call void @llvm.lifetime.end.p0(ptr %40) #4
  br label %350

350:                                              ; preds = %333
  br label %351

351:                                              ; preds = %350
  br label %352

352:                                              ; preds = %351
  call void @llvm.lifetime.start.p0(ptr %42) #4
  call void @llvm.lifetime.start.p0(ptr %43) #4
  %353 = load ptr, ptr %7, align 2, !tbaa !12
  %354 = load i16, ptr %8, align 2, !tbaa !14
  %355 = getelementptr inbounds i64, ptr %353, i16 %354
  %356 = load volatile i64, ptr %355, align 2, !tbaa !19
  %357 = load ptr, ptr %7, align 2, !tbaa !12
  %358 = load i16, ptr %9, align 2, !tbaa !14
  %359 = getelementptr inbounds i64, ptr %357, i16 %358
  %360 = load volatile i64, ptr %359, align 2, !tbaa !19
  %361 = call { i64, i1 } @llvm.usub.with.overflow.i64(i64 %356, i64 %360)
  %362 = extractvalue { i64, i1 } %361, 1
  %363 = extractvalue { i64, i1 } %361, 0
  store i64 %363, ptr %42, align 2
  %364 = zext i1 %362 to i16
  store i16 %364, ptr %43, align 2, !tbaa !14
  %365 = load i16, ptr %43, align 2, !tbaa !14
  %366 = sext i16 %365 to i32
  call void @report(i32 noundef %366) #5
  %367 = load i64, ptr %42, align 2, !tbaa !19
  %368 = trunc i64 %367 to i32
  call void @report(i32 noundef %368) #5
  call void @llvm.lifetime.end.p0(ptr %43) #4
  call void @llvm.lifetime.end.p0(ptr %42) #4
  br label %369

369:                                              ; preds = %352
  br label %370

370:                                              ; preds = %369
  br label %371

371:                                              ; preds = %370
  call void @llvm.lifetime.start.p0(ptr %44) #4
  call void @llvm.lifetime.start.p0(ptr %45) #4
  %372 = load ptr, ptr %7, align 2, !tbaa !12
  %373 = load i16, ptr %8, align 2, !tbaa !14
  %374 = getelementptr inbounds i64, ptr %372, i16 %373
  %375 = load volatile i64, ptr %374, align 2, !tbaa !19
  %376 = load ptr, ptr %7, align 2, !tbaa !12
  %377 = load i16, ptr %9, align 2, !tbaa !14
  %378 = getelementptr inbounds i64, ptr %376, i16 %377
  %379 = load volatile i64, ptr %378, align 2, !tbaa !19
  %380 = call { i64, i1 } @llvm.umul.with.overflow.i64(i64 %375, i64 %379)
  %381 = extractvalue { i64, i1 } %380, 1
  %382 = extractvalue { i64, i1 } %380, 0
  store i64 %382, ptr %44, align 2
  %383 = zext i1 %381 to i16
  store i16 %383, ptr %45, align 2, !tbaa !14
  %384 = load i16, ptr %45, align 2, !tbaa !14
  %385 = sext i16 %384 to i32
  call void @report(i32 noundef %385) #5
  %386 = load i64, ptr %44, align 2, !tbaa !19
  %387 = trunc i64 %386 to i32
  call void @report(i32 noundef %387) #5
  call void @llvm.lifetime.end.p0(ptr %45) #4
  call void @llvm.lifetime.end.p0(ptr %44) #4
  br label %388

388:                                              ; preds = %371
  br label %389

389:                                              ; preds = %388
  br label %390

390:                                              ; preds = %389
  %391 = load i16, ptr %9, align 2, !tbaa !14
  %392 = add nsw i16 %391, 1
  store i16 %392, ptr %9, align 2, !tbaa !14
  br label %50, !llvm.loop !21

393:                                              ; preds = %50
  br label %394

394:                                              ; preds = %393
  %395 = load i16, ptr %8, align 2, !tbaa !14
  %396 = add nsw i16 %395, 1
  store i16 %396, ptr %8, align 2, !tbaa !14
  br label %46, !llvm.loop !23

397:                                              ; preds = %46
  call void @llvm.lifetime.end.p0(ptr %9) #4
  call void @llvm.lifetime.end.p0(ptr %8) #4
  call void @llvm.lifetime.end.p0(ptr %7) #4
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i16, i1 } @llvm.sadd.with.overflow.i16(i16, i16) #2

; Function Attrs: optsize
declare dso_local void @report(i32 noundef) #3

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i16, i1 } @llvm.ssub.with.overflow.i16(i16, i16) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i16, i1 } @llvm.smul.with.overflow.i16(i16, i16) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i16, i1 } @llvm.uadd.with.overflow.i16(i16, i16) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i16, i1 } @llvm.usub.with.overflow.i16(i16, i16) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i16, i1 } @llvm.umul.with.overflow.i16(i16, i16) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.uadd.with.overflow.i32(i32, i32) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.usub.with.overflow.i32(i32, i32) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.umul.with.overflow.i32(i32, i32) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.uadd.with.overflow.i64(i64, i64) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.usub.with.overflow.i64(i64, i64) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i64, i1 } @llvm.umul.with.overflow.i64(i64, i64) #2

attributes #0 = { nounwind optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { optsize "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nounwind }
attributes #5 = { optsize }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"p1 short", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!11, !11, i64 0}
!11 = !{!"p1 long", !9, i64 0}
!12 = !{!13, !13, i64 0}
!13 = !{!"p1 long long", !9, i64 0}
!14 = !{!4, !4, i64 0}
!15 = !{!16, !16, i64 0}
!16 = !{!"short", !5, i64 0}
!17 = !{!18, !18, i64 0}
!18 = !{!"long", !5, i64 0}
!19 = !{!20, !20, i64 0}
!20 = !{!"long long", !5, i64 0}
!21 = distinct !{!21, !22}
!22 = !{!"llvm.loop.mustprogress"}
!23 = distinct !{!23, !22}
