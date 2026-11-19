; ModuleID = 'min.c'
source_filename = "min.c"
target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"


; Function Attrs: noinline nounwind
define hidden zeroext i8 @f(i8 noundef zeroext %0, i8 noundef zeroext %1, i8 noundef zeroext %2) #0 {
  %4 = alloca i8, align 1
  %5 = alloca i8, align 1
  %6 = alloca i8, align 1
  %7 = alloca i8, align 1
  %8 = alloca i8, align 1
  %9 = alloca i8, align 1
  %10 = alloca i8, align 1
  %11 = alloca i32, align 4
  %12 = alloca i8, align 1
  %13 = alloca i8, align 1
  %14 = alloca i8, align 1
  %15 = alloca i8, align 1
  %16 = alloca i8, align 1
  %17 = alloca i8, align 1
  %18 = alloca i8, align 1
  %19 = alloca i8, align 1
  %20 = alloca i8, align 1
  %21 = alloca i8, align 1
  %22 = alloca i8, align 1
  %23 = alloca i8, align 1
  %24 = alloca i8, align 1
  %25 = alloca i8, align 1
  store i8 %0, ptr %4, align 1, !tbaa !6
  store i8 %1, ptr %5, align 1, !tbaa !6
  store i8 %2, ptr %6, align 1, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %7) #3
  store i8 0, ptr %7, align 1, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %8) #3
  store i8 0, ptr %8, align 1, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i8 0, ptr %9, align 1, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %10) #3
  store i8 0, ptr %10, align 1, !tbaa !6
  br label %26

26:                                               ; preds = %145, %3
  %27 = load i8, ptr %10, align 1, !tbaa !6
  %28 = zext i8 %27 to i32
  %29 = load i8, ptr %6, align 1, !tbaa !6
  %30 = zext i8 %29 to i32
  %31 = add nsw i32 %30, 0
  %32 = trunc i32 %31 to i8
  %33 = zext i8 %32 to i32
  %34 = icmp slt i32 %28, %33
  br i1 %34, label %36, label %35

35:                                               ; preds = %26
  store i32 2, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %10) #3
  br label %148

36:                                               ; preds = %26
  call void @llvm.lifetime.start.p0(ptr %12) #3
  store i8 0, ptr %12, align 1, !tbaa !6
  br label %37

37:                                               ; preds = %63, %36
  %38 = load i8, ptr %12, align 1, !tbaa !6
  %39 = zext i8 %38 to i32
  %40 = load i8, ptr %6, align 1, !tbaa !6
  %41 = zext i8 %40 to i32
  %42 = add nsw i32 %41, 0
  %43 = trunc i32 %42 to i8
  %44 = zext i8 %43 to i32
  %45 = icmp slt i32 %39, %44
  br i1 %45, label %47, label %46

46:                                               ; preds = %37
  store i32 5, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %12) #3
  br label %66

47:                                               ; preds = %37
  call void @llvm.lifetime.start.p0(ptr %13) #3
  store i8 0, ptr %13, align 1, !tbaa !6
  br label %48

48:                                               ; preds = %59, %47
  %49 = load i8, ptr %13, align 1, !tbaa !6
  %50 = zext i8 %49 to i32
  %51 = load i8, ptr %6, align 1, !tbaa !6
  %52 = zext i8 %51 to i32
  %53 = add nsw i32 %52, 1
  %54 = trunc i32 %53 to i8
  %55 = zext i8 %54 to i32
  %56 = icmp slt i32 %50, %55
  br i1 %56, label %58, label %57

57:                                               ; preds = %48
  store i32 8, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %13) #3
  br label %62

58:                                               ; preds = %48
  br label %59

59:                                               ; preds = %58
  %60 = load i8, ptr %13, align 1, !tbaa !6
  %61 = add i8 %60, 1
  store i8 %61, ptr %13, align 1, !tbaa !6
  br label %48, !llvm.loop !7

62:                                               ; preds = %57
  br label %63

63:                                               ; preds = %62
  %64 = load i8, ptr %12, align 1, !tbaa !6
  %65 = add i8 %64, 1
  store i8 %65, ptr %12, align 1, !tbaa !6
  br label %37, !llvm.loop !9

66:                                               ; preds = %46
  call void @llvm.lifetime.start.p0(ptr %14) #3
  store i8 0, ptr %14, align 1, !tbaa !6
  br label %67

67:                                               ; preds = %141, %66
  %68 = load i8, ptr %14, align 1, !tbaa !6
  %69 = zext i8 %68 to i32
  %70 = load i8, ptr %6, align 1, !tbaa !6
  %71 = zext i8 %70 to i32
  %72 = add nsw i32 %71, 0
  %73 = trunc i32 %72 to i8
  %74 = zext i8 %73 to i32
  %75 = icmp slt i32 %69, %74
  br i1 %75, label %77, label %76

76:                                               ; preds = %67
  store i32 11, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %14) #3
  br label %144

77:                                               ; preds = %67
  %78 = load i8, ptr %5, align 1, !tbaa !6
  %79 = zext i8 %78 to i32
  %80 = load i8, ptr %9, align 1, !tbaa !6
  %81 = zext i8 %80 to i32
  %82 = icmp sgt i32 %79, %81
  br i1 %82, label %83, label %99

83:                                               ; preds = %77
  call void @llvm.lifetime.start.p0(ptr %15) #3
  store i8 0, ptr %15, align 1, !tbaa !6
  br label %84

84:                                               ; preds = %95, %83
  %85 = load i8, ptr %15, align 1, !tbaa !6
  %86 = zext i8 %85 to i32
  %87 = load i8, ptr %6, align 1, !tbaa !6
  %88 = zext i8 %87 to i32
  %89 = add nsw i32 %88, 1
  %90 = trunc i32 %89 to i8
  %91 = zext i8 %90 to i32
  %92 = icmp slt i32 %86, %91
  br i1 %92, label %94, label %93

93:                                               ; preds = %84
  store i32 14, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %15) #3
  br label %98

94:                                               ; preds = %84
  br label %95

95:                                               ; preds = %94
  %96 = load i8, ptr %15, align 1, !tbaa !6
  %97 = add i8 %96, 1
  store i8 %97, ptr %15, align 1, !tbaa !6
  br label %84, !llvm.loop !10

98:                                               ; preds = %93
  br label %99

99:                                               ; preds = %98, %77
  call void @llvm.lifetime.start.p0(ptr %16) #3
  store i8 0, ptr %16, align 1, !tbaa !6
  br label %100

100:                                              ; preds = %118, %99
  %101 = load i8, ptr %16, align 1, !tbaa !6
  %102 = zext i8 %101 to i32
  %103 = load i8, ptr %6, align 1, !tbaa !6
  %104 = zext i8 %103 to i32
  %105 = add nsw i32 %104, 1
  %106 = trunc i32 %105 to i8
  %107 = zext i8 %106 to i32
  %108 = icmp slt i32 %102, %107
  br i1 %108, label %110, label %109

109:                                              ; preds = %100
  store i32 17, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %16) #3
  br label %121

110:                                              ; preds = %100
  %111 = load i8, ptr %4, align 1, !tbaa !6
  %112 = zext i8 %111 to i32
  %113 = load i8, ptr %4, align 1, !tbaa !6
  %114 = zext i8 %113 to i32
  %115 = icmp sgt i32 %112, %114
  br i1 %115, label %116, label %117

116:                                              ; preds = %110
  br label %117

117:                                              ; preds = %116, %110
  br label %118

118:                                              ; preds = %117
  %119 = load i8, ptr %16, align 1, !tbaa !6
  %120 = add i8 %119, 1
  store i8 %120, ptr %16, align 1, !tbaa !6
  br label %100, !llvm.loop !11

121:                                              ; preds = %109
  %122 = load i8, ptr %5, align 1, !tbaa !6
  %123 = zext i8 %122 to i32
  %124 = load i8, ptr %9, align 1, !tbaa !6
  %125 = zext i8 %124 to i32
  %126 = icmp ne i32 %123, %125
  br i1 %126, label %127, label %140

127:                                              ; preds = %121
  %128 = load i8, ptr %4, align 1, !tbaa !6
  %129 = zext i8 %128 to i32
  %130 = load i8, ptr %9, align 1, !tbaa !6
  %131 = zext i8 %130 to i32
  %132 = icmp ne i32 %129, %131
  br i1 %132, label %133, label %134

133:                                              ; preds = %127
  br label %134

134:                                              ; preds = %133, %127
  %135 = load i8, ptr %4, align 1, !tbaa !6
  %136 = zext i8 %135 to i32
  %137 = icmp slt i32 %136, 100
  br i1 %137, label %138, label %139

138:                                              ; preds = %134
  br label %139

139:                                              ; preds = %138, %134
  br label %140

140:                                              ; preds = %139, %121
  br label %141

141:                                              ; preds = %140
  %142 = load i8, ptr %14, align 1, !tbaa !6
  %143 = add i8 %142, 1
  store i8 %143, ptr %14, align 1, !tbaa !6
  br label %67, !llvm.loop !12

144:                                              ; preds = %76
  br label %145

145:                                              ; preds = %144
  %146 = load i8, ptr %10, align 1, !tbaa !6
  %147 = add i8 %146, 1
  store i8 %147, ptr %10, align 1, !tbaa !6
  br label %26, !llvm.loop !13

148:                                              ; preds = %35
  call void @llvm.lifetime.start.p0(ptr %17) #3
  store i8 0, ptr %17, align 1, !tbaa !6
  br label %149

149:                                              ; preds = %205, %148
  %150 = load i8, ptr %17, align 1, !tbaa !6
  %151 = zext i8 %150 to i32
  %152 = load i8, ptr %6, align 1, !tbaa !6
  %153 = zext i8 %152 to i32
  %154 = add nsw i32 %153, 1
  %155 = trunc i32 %154 to i8
  %156 = zext i8 %155 to i32
  %157 = icmp slt i32 %151, %156
  br i1 %157, label %159, label %158

158:                                              ; preds = %149
  store i32 20, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %17) #3
  br label %208

159:                                              ; preds = %149
  store i8 1, ptr %9, align 1, !tbaa !6
  call void @llvm.lifetime.start.p0(ptr %18) #3
  store i8 0, ptr %18, align 1, !tbaa !6
  br label %160

160:                                              ; preds = %201, %159
  %161 = load i8, ptr %18, align 1, !tbaa !6
  %162 = zext i8 %161 to i32
  %163 = load i8, ptr %6, align 1, !tbaa !6
  %164 = zext i8 %163 to i32
  %165 = add nsw i32 %164, 0
  %166 = trunc i32 %165 to i8
  %167 = zext i8 %166 to i32
  %168 = icmp slt i32 %162, %167
  br i1 %168, label %170, label %169

169:                                              ; preds = %160
  store i32 23, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %18) #3
  br label %204

170:                                              ; preds = %160
  call void @llvm.lifetime.start.p0(ptr %19) #3
  store i8 0, ptr %19, align 1, !tbaa !6
  br label %171

171:                                              ; preds = %197, %170
  %172 = load i8, ptr %19, align 1, !tbaa !6
  %173 = zext i8 %172 to i32
  %174 = load i8, ptr %6, align 1, !tbaa !6
  %175 = zext i8 %174 to i32
  %176 = add nsw i32 %175, 0
  %177 = trunc i32 %176 to i8
  %178 = zext i8 %177 to i32
  %179 = icmp slt i32 %173, %178
  br i1 %179, label %181, label %180

180:                                              ; preds = %171
  store i32 26, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %19) #3
  br label %200

181:                                              ; preds = %171
  %182 = load i8, ptr %8, align 1, !tbaa !6
  %183 = zext i8 %182 to i32
  %184 = load i8, ptr %9, align 1, !tbaa !6
  %185 = zext i8 %184 to i32
  %186 = icmp ne i32 %183, %185
  br i1 %186, label %187, label %189

187:                                              ; preds = %181
  %188 = load i8, ptr %9, align 1, !tbaa !6
  store i8 %188, ptr %7, align 1, !tbaa !6
  br label %189

189:                                              ; preds = %187, %181
  %190 = load i8, ptr %9, align 1, !tbaa !6
  %191 = zext i8 %190 to i32
  %192 = load i8, ptr %9, align 1, !tbaa !6
  %193 = zext i8 %192 to i32
  %194 = icmp eq i32 %191, %193
  br i1 %194, label %195, label %196

195:                                              ; preds = %189
  store i8 0, ptr %9, align 1, !tbaa !6
  br label %196

196:                                              ; preds = %195, %189
  br label %197

197:                                              ; preds = %196
  %198 = load i8, ptr %19, align 1, !tbaa !6
  %199 = add i8 %198, 1
  store i8 %199, ptr %19, align 1, !tbaa !6
  br label %171, !llvm.loop !14

200:                                              ; preds = %180
  br label %201

201:                                              ; preds = %200
  %202 = load i8, ptr %18, align 1, !tbaa !6
  %203 = add i8 %202, 1
  store i8 %203, ptr %18, align 1, !tbaa !6
  br label %160, !llvm.loop !15

204:                                              ; preds = %169
  br label %205

205:                                              ; preds = %204
  %206 = load i8, ptr %17, align 1, !tbaa !6
  %207 = add i8 %206, 1
  store i8 %207, ptr %17, align 1, !tbaa !6
  br label %149, !llvm.loop !16

208:                                              ; preds = %158
  call void @llvm.lifetime.start.p0(ptr %20) #3
  store i8 0, ptr %20, align 1, !tbaa !6
  br label %209

209:                                              ; preds = %319, %208
  %210 = load i8, ptr %20, align 1, !tbaa !6
  %211 = zext i8 %210 to i32
  %212 = load i8, ptr %6, align 1, !tbaa !6
  %213 = zext i8 %212 to i32
  %214 = add nsw i32 %213, 1
  %215 = trunc i32 %214 to i8
  %216 = zext i8 %215 to i32
  %217 = icmp slt i32 %211, %216
  br i1 %217, label %219, label %218

218:                                              ; preds = %209
  store i32 29, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %20) #3
  br label %322

219:                                              ; preds = %209
  %220 = load i8, ptr %4, align 1, !tbaa !6
  %221 = zext i8 %220 to i32
  %222 = load i8, ptr %8, align 1, !tbaa !6
  %223 = zext i8 %222 to i32
  %224 = icmp eq i32 %221, %223
  br i1 %224, label %225, label %283

225:                                              ; preds = %219
  call void @llvm.lifetime.start.p0(ptr %21) #3
  store i8 0, ptr %21, align 1, !tbaa !6
  br label %226

226:                                              ; preds = %259, %225
  %227 = load i8, ptr %21, align 1, !tbaa !6
  %228 = zext i8 %227 to i32
  %229 = load i8, ptr %6, align 1, !tbaa !6
  %230 = zext i8 %229 to i32
  %231 = add nsw i32 %230, 1
  %232 = trunc i32 %231 to i8
  %233 = zext i8 %232 to i32
  %234 = icmp slt i32 %228, %233
  br i1 %234, label %236, label %235

235:                                              ; preds = %226
  store i32 32, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %21) #3
  br label %262

236:                                              ; preds = %226
  call void @llvm.lifetime.start.p0(ptr %22) #3
  store i8 0, ptr %22, align 1, !tbaa !6
  br label %237

237:                                              ; preds = %248, %236
  %238 = load i8, ptr %22, align 1, !tbaa !6
  %239 = zext i8 %238 to i32
  %240 = load i8, ptr %6, align 1, !tbaa !6
  %241 = zext i8 %240 to i32
  %242 = add nsw i32 %241, 0
  %243 = trunc i32 %242 to i8
  %244 = zext i8 %243 to i32
  %245 = icmp slt i32 %239, %244
  br i1 %245, label %247, label %246

246:                                              ; preds = %237
  store i32 35, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %22) #3
  br label %251

247:                                              ; preds = %237
  br label %248

248:                                              ; preds = %247
  %249 = load i8, ptr %22, align 1, !tbaa !6
  %250 = add i8 %249, 1
  store i8 %250, ptr %22, align 1, !tbaa !6
  br label %237, !llvm.loop !17

251:                                              ; preds = %246
  %252 = load i8, ptr %5, align 1, !tbaa !6
  %253 = zext i8 %252 to i32
  %254 = load i8, ptr %8, align 1, !tbaa !6
  %255 = zext i8 %254 to i32
  %256 = icmp slt i32 %253, %255
  br i1 %256, label %257, label %258

257:                                              ; preds = %251
  br label %258

258:                                              ; preds = %257, %251
  br label %259

259:                                              ; preds = %258
  %260 = load i8, ptr %21, align 1, !tbaa !6
  %261 = add i8 %260, 1
  store i8 %261, ptr %21, align 1, !tbaa !6
  br label %226, !llvm.loop !18

262:                                              ; preds = %235
  %263 = load i8, ptr %8, align 1, !tbaa !6
  %264 = zext i8 %263 to i32
  %265 = icmp ne i32 %264, 3
  br i1 %265, label %266, label %282

266:                                              ; preds = %262
  call void @llvm.lifetime.start.p0(ptr %23) #3
  store i8 0, ptr %23, align 1, !tbaa !6
  br label %267

267:                                              ; preds = %278, %266
  %268 = load i8, ptr %23, align 1, !tbaa !6
  %269 = zext i8 %268 to i32
  %270 = load i8, ptr %6, align 1, !tbaa !6
  %271 = zext i8 %270 to i32
  %272 = add nsw i32 %271, 1
  %273 = trunc i32 %272 to i8
  %274 = zext i8 %273 to i32
  %275 = icmp slt i32 %269, %274
  br i1 %275, label %277, label %276

276:                                              ; preds = %267
  store i32 38, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %23) #3
  br label %281

277:                                              ; preds = %267
  br label %278

278:                                              ; preds = %277
  %279 = load i8, ptr %23, align 1, !tbaa !6
  %280 = add i8 %279, 1
  store i8 %280, ptr %23, align 1, !tbaa !6
  br label %267, !llvm.loop !19

281:                                              ; preds = %276
  br label %282

282:                                              ; preds = %281, %262
  br label %283

283:                                              ; preds = %282, %219
  call void @llvm.lifetime.start.p0(ptr %24) #3
  store i8 0, ptr %24, align 1, !tbaa !6
  br label %284

284:                                              ; preds = %315, %283
  %285 = load i8, ptr %24, align 1, !tbaa !6
  %286 = zext i8 %285 to i32
  %287 = load i8, ptr %6, align 1, !tbaa !6
  %288 = zext i8 %287 to i32
  %289 = add nsw i32 %288, 1
  %290 = trunc i32 %289 to i8
  %291 = zext i8 %290 to i32
  %292 = icmp slt i32 %286, %291
  br i1 %292, label %294, label %293

293:                                              ; preds = %284
  store i32 41, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %24) #3
  br label %318

294:                                              ; preds = %284
  call void @llvm.lifetime.start.p0(ptr %25) #3
  store i8 0, ptr %25, align 1, !tbaa !6
  br label %295

295:                                              ; preds = %311, %294
  %296 = load i8, ptr %25, align 1, !tbaa !6
  %297 = zext i8 %296 to i32
  %298 = load i8, ptr %6, align 1, !tbaa !6
  %299 = zext i8 %298 to i32
  %300 = add nsw i32 %299, 0
  %301 = trunc i32 %300 to i8
  %302 = zext i8 %301 to i32
  %303 = icmp slt i32 %297, %302
  br i1 %303, label %305, label %304

304:                                              ; preds = %295
  store i32 44, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %25) #3
  br label %314

305:                                              ; preds = %295
  %306 = load i8, ptr %8, align 1, !tbaa !6
  %307 = zext i8 %306 to i32
  %308 = icmp slt i32 %307, 100
  br i1 %308, label %309, label %310

309:                                              ; preds = %305
  br label %310

310:                                              ; preds = %309, %305
  br label %311

311:                                              ; preds = %310
  %312 = load i8, ptr %25, align 1, !tbaa !6
  %313 = add i8 %312, 1
  store i8 %313, ptr %25, align 1, !tbaa !6
  br label %295, !llvm.loop !20

314:                                              ; preds = %304
  br label %315

315:                                              ; preds = %314
  %316 = load i8, ptr %24, align 1, !tbaa !6
  %317 = add i8 %316, 1
  store i8 %317, ptr %24, align 1, !tbaa !6
  br label %284, !llvm.loop !21

318:                                              ; preds = %293
  br label %319

319:                                              ; preds = %318
  %320 = load i8, ptr %20, align 1, !tbaa !6
  %321 = add i8 %320, 1
  store i8 %321, ptr %20, align 1, !tbaa !6
  br label %209, !llvm.loop !22

322:                                              ; preds = %218
  %323 = load i8, ptr %4, align 1, !tbaa !6
  %324 = zext i8 %323 to i32
  %325 = load i8, ptr %5, align 1, !tbaa !6
  %326 = zext i8 %325 to i32
  %327 = mul nsw i32 3, %326
  %328 = add nsw i32 %324, %327
  %329 = load i8, ptr %7, align 1, !tbaa !6
  %330 = zext i8 %329 to i32
  %331 = mul nsw i32 5, %330
  %332 = add nsw i32 %328, %331
  %333 = load i8, ptr %8, align 1, !tbaa !6
  %334 = zext i8 %333 to i32
  %335 = mul nsw i32 7, %334
  %336 = add nsw i32 %332, %335
  %337 = load i8, ptr %9, align 1, !tbaa !6
  %338 = zext i8 %337 to i32
  %339 = mul nsw i32 11, %338
  %340 = add nsw i32 %336, %339
  %341 = trunc i32 %340 to i8
  store i32 1, ptr %11, align 4
  call void @llvm.lifetime.end.p0(ptr %9) #3
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  ret i8 %341
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind

attributes #0 = { noinline nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+bulk-memory,+bulk-memory-opt,+call-indirect-overlong,+multivalue,+mutable-globals,+nontrapping-fptoint,+reference-types,+sign-ext" }
attributes #3 = { nounwind }

!llvm.ident = !{!0}
!llvm.errno.tbaa = !{!1}

!0 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!1 = !{!2, !3, i64 0}
!2 = !{!"__libc_errno", !3, i64 0}
!3 = !{!"int", !4, i64 0}
!4 = !{!"omnipotent char", !5, i64 0}
!5 = !{!"Simple C/C++ TBAA"}
!6 = !{!4, !4, i64 0}
!7 = distinct !{!7, !8}
!8 = !{!"llvm.loop.mustprogress"}
!9 = distinct !{!9, !8}
!10 = distinct !{!10, !8}
!11 = distinct !{!11, !8}
!12 = distinct !{!12, !8}
!13 = distinct !{!13, !8}
!14 = distinct !{!14, !8}
!15 = distinct !{!15, !8}
!16 = distinct !{!16, !8}
!17 = distinct !{!17, !8}
!18 = distinct !{!18, !8}
!19 = distinct !{!19, !8}
!20 = distinct !{!20, !8}
!21 = distinct !{!21, !8}
!22 = distinct !{!22, !8}
