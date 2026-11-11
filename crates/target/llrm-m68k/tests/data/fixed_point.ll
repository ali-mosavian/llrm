; ModuleID = 'tests/run/c/fixed_point.c'
source_filename = "tests/run/c/fixed_point.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

%union.anon = type { i8 }
%union.anon.0 = type { i16 }
%union.anon.1 = type { i32 }
%union.anon.2 = type { i8 }
%union.anon.3 = type { i16 }
%union.anon.4 = type { i32 }
%union.anon.5 = type { i16 }
%union.anon.6 = type { i32 }
%union.anon.7 = type { i64 }
%union.anon.8 = type { i16 }
%union.anon.9 = type { i32 }
%union.anon.10 = type { i64 }
%union.anon.11 = type { i8 }
%union.anon.12 = type { i16 }
%union.anon.13 = type { i32 }
%union.anon.14 = type { i16 }
%union.anon.15 = type { i16 }
%union.anon.16 = type { i64 }
%union.anon.17 = type { i64 }

@sf = dso_local global [3 x i8] c"@\A0~", align 1
@f = dso_local global [3 x i16] [i16 16384, i16 -24576, i16 32440], align 2
@lf = dso_local global [3 x i32] [i32 1073741824, i32 -1610612736, i32 2126008811], align 4
@usf = dso_local global [3 x i8] c"\80\C0\FD", align 1
@uf = dso_local global [3 x i16] [i16 -32768, i16 -16384, i16 -656], align 2
@ulf = dso_local global [3 x i32] [i32 -2147483648, i32 -1073741824, i32 -42949673], align 4
@sa = dso_local global [3 x i16] [i16 192, i16 -288, i16 12800], align 2
@a = dso_local global [3 x i32] [i32 49152, i32 -73728, i32 3276800], align 4
@la = dso_local global [3 x i64] [i64 3221225472, i64 -4831838208, i64 64424509440000], align 8
@usa = dso_local global [3 x i16] [i16 384, i16 576, i16 25600], align 2
@ua = dso_local global [3 x i32] [i32 98304, i32 147456, i32 6553600], align 4
@ula = dso_local global [3 x i64] [i64 6442450944, i64 9663676416, i64 128849018880000], align 8

; Function Attrs: noinline nounwind optnone
define dso_local i32 @main() #0 {
  %1 = alloca ptr, align 2
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i8, align 1
  %5 = alloca %union.anon, align 1
  %6 = alloca ptr, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i16, align 2
  %10 = alloca %union.anon.0, align 2
  %11 = alloca ptr, align 2
  %12 = alloca i32, align 2
  %13 = alloca i32, align 2
  %14 = alloca i32, align 4
  %15 = alloca %union.anon.1, align 4
  %16 = alloca ptr, align 2
  %17 = alloca i32, align 2
  %18 = alloca i32, align 2
  %19 = alloca i8, align 1
  %20 = alloca %union.anon.2, align 1
  %21 = alloca ptr, align 2
  %22 = alloca i32, align 2
  %23 = alloca i32, align 2
  %24 = alloca i16, align 2
  %25 = alloca %union.anon.3, align 2
  %26 = alloca ptr, align 2
  %27 = alloca i32, align 2
  %28 = alloca i32, align 2
  %29 = alloca i32, align 4
  %30 = alloca %union.anon.4, align 4
  %31 = alloca ptr, align 2
  %32 = alloca i32, align 2
  %33 = alloca i32, align 2
  %34 = alloca i16, align 2
  %35 = alloca %union.anon.5, align 2
  %36 = alloca ptr, align 2
  %37 = alloca i32, align 2
  %38 = alloca i32, align 2
  %39 = alloca i32, align 4
  %40 = alloca %union.anon.6, align 4
  %41 = alloca ptr, align 2
  %42 = alloca i32, align 2
  %43 = alloca i32, align 2
  %44 = alloca i64, align 8
  %45 = alloca %union.anon.7, align 8
  %46 = alloca ptr, align 2
  %47 = alloca i32, align 2
  %48 = alloca i32, align 2
  %49 = alloca i16, align 2
  %50 = alloca %union.anon.8, align 2
  %51 = alloca ptr, align 2
  %52 = alloca i32, align 2
  %53 = alloca i32, align 2
  %54 = alloca i32, align 4
  %55 = alloca %union.anon.9, align 4
  %56 = alloca ptr, align 2
  %57 = alloca i32, align 2
  %58 = alloca i32, align 2
  %59 = alloca i64, align 8
  %60 = alloca %union.anon.10, align 8
  %61 = alloca ptr, align 2
  %62 = alloca i32, align 2
  %63 = alloca i32, align 2
  %64 = alloca i8, align 1
  %65 = alloca %union.anon.11, align 1
  %66 = alloca ptr, align 2
  %67 = alloca i32, align 2
  %68 = alloca i32, align 2
  %69 = alloca i16, align 2
  %70 = alloca %union.anon.12, align 2
  %71 = alloca ptr, align 2
  %72 = alloca i32, align 2
  %73 = alloca i32, align 2
  %74 = alloca i32, align 4
  %75 = alloca %union.anon.13, align 4
  %76 = alloca ptr, align 2
  %77 = alloca i32, align 2
  %78 = alloca i32, align 2
  %79 = alloca i16, align 2
  %80 = alloca %union.anon.14, align 2
  %81 = alloca ptr, align 2
  %82 = alloca i32, align 2
  %83 = alloca i32, align 2
  %84 = alloca i16, align 2
  %85 = alloca %union.anon.15, align 2
  %86 = alloca ptr, align 2
  %87 = alloca i32, align 2
  %88 = alloca i32, align 2
  %89 = alloca i64, align 8
  %90 = alloca %union.anon.16, align 8
  %91 = alloca ptr, align 2
  %92 = alloca i32, align 2
  %93 = alloca i32, align 2
  %94 = alloca i64, align 8
  %95 = alloca %union.anon.17, align 8
  br label %96

96:                                               ; preds = %0
  store ptr @sf, ptr %1, align 2
  store i32 0, ptr %2, align 2
  br label %97

97:                                               ; preds = %121, %96
  %98 = load i32, ptr %2, align 2
  %99 = icmp slt i32 %98, 3
  br i1 %99, label %100, label %124

100:                                              ; preds = %97
  store i32 0, ptr %3, align 2
  br label %101

101:                                              ; preds = %117, %100
  %102 = load i32, ptr %3, align 2
  %103 = icmp slt i32 %102, 3
  br i1 %103, label %104, label %120

104:                                              ; preds = %101
  %105 = load ptr, ptr %1, align 2
  %106 = load i32, ptr %2, align 2
  %107 = getelementptr inbounds i8, ptr %105, i32 %106
  %108 = load volatile i8, ptr %107, align 1
  %109 = load ptr, ptr %1, align 2
  %110 = load i32, ptr %3, align 2
  %111 = getelementptr inbounds i8, ptr %109, i32 %110
  %112 = load volatile i8, ptr %111, align 1
  %113 = call i8 @llvm.smul.fix.sat.i8(i8 %108, i8 %112, i32 7)
  store i8 %113, ptr %4, align 1
  %114 = load i8, ptr %4, align 1
  store i8 %114, ptr %5, align 1
  %115 = load i8, ptr %5, align 1
  %116 = zext i8 %115 to i32
  call void @report(i32 noundef %116) #3
  br label %117

117:                                              ; preds = %104
  %118 = load i32, ptr %3, align 2
  %119 = add nsw i32 %118, 1
  store i32 %119, ptr %3, align 2
  br label %101, !llvm.loop !4

120:                                              ; preds = %101
  br label %121

121:                                              ; preds = %120
  %122 = load i32, ptr %2, align 2
  %123 = add nsw i32 %122, 1
  store i32 %123, ptr %2, align 2
  br label %97, !llvm.loop !6

124:                                              ; preds = %97
  br label %125

125:                                              ; preds = %124
  br label %126

126:                                              ; preds = %125
  store ptr @f, ptr %6, align 2
  store i32 0, ptr %7, align 2
  br label %127

127:                                              ; preds = %151, %126
  %128 = load i32, ptr %7, align 2
  %129 = icmp slt i32 %128, 3
  br i1 %129, label %130, label %154

130:                                              ; preds = %127
  store i32 0, ptr %8, align 2
  br label %131

131:                                              ; preds = %147, %130
  %132 = load i32, ptr %8, align 2
  %133 = icmp slt i32 %132, 3
  br i1 %133, label %134, label %150

134:                                              ; preds = %131
  %135 = load ptr, ptr %6, align 2
  %136 = load i32, ptr %7, align 2
  %137 = getelementptr inbounds i16, ptr %135, i32 %136
  %138 = load volatile i16, ptr %137, align 2
  %139 = load ptr, ptr %6, align 2
  %140 = load i32, ptr %8, align 2
  %141 = getelementptr inbounds i16, ptr %139, i32 %140
  %142 = load volatile i16, ptr %141, align 2
  %143 = call i16 @llvm.smul.fix.sat.i16(i16 %138, i16 %142, i32 15)
  store i16 %143, ptr %9, align 2
  %144 = load i16, ptr %9, align 2
  store i16 %144, ptr %10, align 2
  %145 = load i16, ptr %10, align 2
  %146 = zext i16 %145 to i32
  call void @report(i32 noundef %146) #3
  br label %147

147:                                              ; preds = %134
  %148 = load i32, ptr %8, align 2
  %149 = add nsw i32 %148, 1
  store i32 %149, ptr %8, align 2
  br label %131, !llvm.loop !7

150:                                              ; preds = %131
  br label %151

151:                                              ; preds = %150
  %152 = load i32, ptr %7, align 2
  %153 = add nsw i32 %152, 1
  store i32 %153, ptr %7, align 2
  br label %127, !llvm.loop !8

154:                                              ; preds = %127
  br label %155

155:                                              ; preds = %154
  br label %156

156:                                              ; preds = %155
  store ptr @lf, ptr %11, align 2
  store i32 0, ptr %12, align 2
  br label %157

157:                                              ; preds = %180, %156
  %158 = load i32, ptr %12, align 2
  %159 = icmp slt i32 %158, 3
  br i1 %159, label %160, label %183

160:                                              ; preds = %157
  store i32 0, ptr %13, align 2
  br label %161

161:                                              ; preds = %176, %160
  %162 = load i32, ptr %13, align 2
  %163 = icmp slt i32 %162, 3
  br i1 %163, label %164, label %179

164:                                              ; preds = %161
  %165 = load ptr, ptr %11, align 2
  %166 = load i32, ptr %12, align 2
  %167 = getelementptr inbounds i32, ptr %165, i32 %166
  %168 = load volatile i32, ptr %167, align 4
  %169 = load ptr, ptr %11, align 2
  %170 = load i32, ptr %13, align 2
  %171 = getelementptr inbounds i32, ptr %169, i32 %170
  %172 = load volatile i32, ptr %171, align 4
  %173 = call i32 @llvm.smul.fix.sat.i32(i32 %168, i32 %172, i32 31)
  store i32 %173, ptr %14, align 4
  %174 = load i32, ptr %14, align 4
  store i32 %174, ptr %15, align 4
  %175 = load i32, ptr %15, align 4
  call void @report(i32 noundef %175) #3
  br label %176

176:                                              ; preds = %164
  %177 = load i32, ptr %13, align 2
  %178 = add nsw i32 %177, 1
  store i32 %178, ptr %13, align 2
  br label %161, !llvm.loop !9

179:                                              ; preds = %161
  br label %180

180:                                              ; preds = %179
  %181 = load i32, ptr %12, align 2
  %182 = add nsw i32 %181, 1
  store i32 %182, ptr %12, align 2
  br label %157, !llvm.loop !10

183:                                              ; preds = %157
  br label %184

184:                                              ; preds = %183
  br label %185

185:                                              ; preds = %184
  store ptr @usf, ptr %16, align 2
  store i32 0, ptr %17, align 2
  br label %186

186:                                              ; preds = %210, %185
  %187 = load i32, ptr %17, align 2
  %188 = icmp slt i32 %187, 3
  br i1 %188, label %189, label %213

189:                                              ; preds = %186
  store i32 0, ptr %18, align 2
  br label %190

190:                                              ; preds = %206, %189
  %191 = load i32, ptr %18, align 2
  %192 = icmp slt i32 %191, 3
  br i1 %192, label %193, label %209

193:                                              ; preds = %190
  %194 = load ptr, ptr %16, align 2
  %195 = load i32, ptr %17, align 2
  %196 = getelementptr inbounds i8, ptr %194, i32 %195
  %197 = load volatile i8, ptr %196, align 1
  %198 = load ptr, ptr %16, align 2
  %199 = load i32, ptr %18, align 2
  %200 = getelementptr inbounds i8, ptr %198, i32 %199
  %201 = load volatile i8, ptr %200, align 1
  %202 = call i8 @llvm.umul.fix.sat.i8(i8 %197, i8 %201, i32 8)
  store i8 %202, ptr %19, align 1
  %203 = load i8, ptr %19, align 1
  store i8 %203, ptr %20, align 1
  %204 = load i8, ptr %20, align 1
  %205 = zext i8 %204 to i32
  call void @report(i32 noundef %205) #3
  br label %206

206:                                              ; preds = %193
  %207 = load i32, ptr %18, align 2
  %208 = add nsw i32 %207, 1
  store i32 %208, ptr %18, align 2
  br label %190, !llvm.loop !11

209:                                              ; preds = %190
  br label %210

210:                                              ; preds = %209
  %211 = load i32, ptr %17, align 2
  %212 = add nsw i32 %211, 1
  store i32 %212, ptr %17, align 2
  br label %186, !llvm.loop !12

213:                                              ; preds = %186
  br label %214

214:                                              ; preds = %213
  br label %215

215:                                              ; preds = %214
  store ptr @uf, ptr %21, align 2
  store i32 0, ptr %22, align 2
  br label %216

216:                                              ; preds = %240, %215
  %217 = load i32, ptr %22, align 2
  %218 = icmp slt i32 %217, 3
  br i1 %218, label %219, label %243

219:                                              ; preds = %216
  store i32 0, ptr %23, align 2
  br label %220

220:                                              ; preds = %236, %219
  %221 = load i32, ptr %23, align 2
  %222 = icmp slt i32 %221, 3
  br i1 %222, label %223, label %239

223:                                              ; preds = %220
  %224 = load ptr, ptr %21, align 2
  %225 = load i32, ptr %22, align 2
  %226 = getelementptr inbounds i16, ptr %224, i32 %225
  %227 = load volatile i16, ptr %226, align 2
  %228 = load ptr, ptr %21, align 2
  %229 = load i32, ptr %23, align 2
  %230 = getelementptr inbounds i16, ptr %228, i32 %229
  %231 = load volatile i16, ptr %230, align 2
  %232 = call i16 @llvm.umul.fix.sat.i16(i16 %227, i16 %231, i32 16)
  store i16 %232, ptr %24, align 2
  %233 = load i16, ptr %24, align 2
  store i16 %233, ptr %25, align 2
  %234 = load i16, ptr %25, align 2
  %235 = zext i16 %234 to i32
  call void @report(i32 noundef %235) #3
  br label %236

236:                                              ; preds = %223
  %237 = load i32, ptr %23, align 2
  %238 = add nsw i32 %237, 1
  store i32 %238, ptr %23, align 2
  br label %220, !llvm.loop !13

239:                                              ; preds = %220
  br label %240

240:                                              ; preds = %239
  %241 = load i32, ptr %22, align 2
  %242 = add nsw i32 %241, 1
  store i32 %242, ptr %22, align 2
  br label %216, !llvm.loop !14

243:                                              ; preds = %216
  br label %244

244:                                              ; preds = %243
  br label %245

245:                                              ; preds = %244
  store ptr @ulf, ptr %26, align 2
  store i32 0, ptr %27, align 2
  br label %246

246:                                              ; preds = %269, %245
  %247 = load i32, ptr %27, align 2
  %248 = icmp slt i32 %247, 3
  br i1 %248, label %249, label %272

249:                                              ; preds = %246
  store i32 0, ptr %28, align 2
  br label %250

250:                                              ; preds = %265, %249
  %251 = load i32, ptr %28, align 2
  %252 = icmp slt i32 %251, 3
  br i1 %252, label %253, label %268

253:                                              ; preds = %250
  %254 = load ptr, ptr %26, align 2
  %255 = load i32, ptr %27, align 2
  %256 = getelementptr inbounds i32, ptr %254, i32 %255
  %257 = load volatile i32, ptr %256, align 4
  %258 = load ptr, ptr %26, align 2
  %259 = load i32, ptr %28, align 2
  %260 = getelementptr inbounds i32, ptr %258, i32 %259
  %261 = load volatile i32, ptr %260, align 4
  %262 = call i32 @llvm.umul.fix.sat.i32(i32 %257, i32 %261, i32 32)
  store i32 %262, ptr %29, align 4
  %263 = load i32, ptr %29, align 4
  store i32 %263, ptr %30, align 4
  %264 = load i32, ptr %30, align 4
  call void @report(i32 noundef %264) #3
  br label %265

265:                                              ; preds = %253
  %266 = load i32, ptr %28, align 2
  %267 = add nsw i32 %266, 1
  store i32 %267, ptr %28, align 2
  br label %250, !llvm.loop !15

268:                                              ; preds = %250
  br label %269

269:                                              ; preds = %268
  %270 = load i32, ptr %27, align 2
  %271 = add nsw i32 %270, 1
  store i32 %271, ptr %27, align 2
  br label %246, !llvm.loop !16

272:                                              ; preds = %246
  br label %273

273:                                              ; preds = %272
  br label %274

274:                                              ; preds = %273
  store ptr @sa, ptr %31, align 2
  store i32 0, ptr %32, align 2
  br label %275

275:                                              ; preds = %299, %274
  %276 = load i32, ptr %32, align 2
  %277 = icmp slt i32 %276, 3
  br i1 %277, label %278, label %302

278:                                              ; preds = %275
  store i32 0, ptr %33, align 2
  br label %279

279:                                              ; preds = %295, %278
  %280 = load i32, ptr %33, align 2
  %281 = icmp slt i32 %280, 3
  br i1 %281, label %282, label %298

282:                                              ; preds = %279
  %283 = load ptr, ptr %31, align 2
  %284 = load i32, ptr %32, align 2
  %285 = getelementptr inbounds i16, ptr %283, i32 %284
  %286 = load volatile i16, ptr %285, align 2
  %287 = load ptr, ptr %31, align 2
  %288 = load i32, ptr %33, align 2
  %289 = getelementptr inbounds i16, ptr %287, i32 %288
  %290 = load volatile i16, ptr %289, align 2
  %291 = call i16 @llvm.smul.fix.sat.i16(i16 %286, i16 %290, i32 7)
  store i16 %291, ptr %34, align 2
  %292 = load i16, ptr %34, align 2
  store i16 %292, ptr %35, align 2
  %293 = load i16, ptr %35, align 2
  %294 = zext i16 %293 to i32
  call void @report(i32 noundef %294) #3
  br label %295

295:                                              ; preds = %282
  %296 = load i32, ptr %33, align 2
  %297 = add nsw i32 %296, 1
  store i32 %297, ptr %33, align 2
  br label %279, !llvm.loop !17

298:                                              ; preds = %279
  br label %299

299:                                              ; preds = %298
  %300 = load i32, ptr %32, align 2
  %301 = add nsw i32 %300, 1
  store i32 %301, ptr %32, align 2
  br label %275, !llvm.loop !18

302:                                              ; preds = %275
  br label %303

303:                                              ; preds = %302
  br label %304

304:                                              ; preds = %303
  store ptr @a, ptr %36, align 2
  store i32 0, ptr %37, align 2
  br label %305

305:                                              ; preds = %328, %304
  %306 = load i32, ptr %37, align 2
  %307 = icmp slt i32 %306, 3
  br i1 %307, label %308, label %331

308:                                              ; preds = %305
  store i32 0, ptr %38, align 2
  br label %309

309:                                              ; preds = %324, %308
  %310 = load i32, ptr %38, align 2
  %311 = icmp slt i32 %310, 3
  br i1 %311, label %312, label %327

312:                                              ; preds = %309
  %313 = load ptr, ptr %36, align 2
  %314 = load i32, ptr %37, align 2
  %315 = getelementptr inbounds i32, ptr %313, i32 %314
  %316 = load volatile i32, ptr %315, align 4
  %317 = load ptr, ptr %36, align 2
  %318 = load i32, ptr %38, align 2
  %319 = getelementptr inbounds i32, ptr %317, i32 %318
  %320 = load volatile i32, ptr %319, align 4
  %321 = call i32 @llvm.smul.fix.sat.i32(i32 %316, i32 %320, i32 15)
  store i32 %321, ptr %39, align 4
  %322 = load i32, ptr %39, align 4
  store i32 %322, ptr %40, align 4
  %323 = load i32, ptr %40, align 4
  call void @report(i32 noundef %323) #3
  br label %324

324:                                              ; preds = %312
  %325 = load i32, ptr %38, align 2
  %326 = add nsw i32 %325, 1
  store i32 %326, ptr %38, align 2
  br label %309, !llvm.loop !19

327:                                              ; preds = %309
  br label %328

328:                                              ; preds = %327
  %329 = load i32, ptr %37, align 2
  %330 = add nsw i32 %329, 1
  store i32 %330, ptr %37, align 2
  br label %305, !llvm.loop !20

331:                                              ; preds = %305
  br label %332

332:                                              ; preds = %331
  br label %333

333:                                              ; preds = %332
  store ptr @la, ptr %41, align 2
  store i32 0, ptr %42, align 2
  br label %334

334:                                              ; preds = %358, %333
  %335 = load i32, ptr %42, align 2
  %336 = icmp slt i32 %335, 3
  br i1 %336, label %337, label %361

337:                                              ; preds = %334
  store i32 0, ptr %43, align 2
  br label %338

338:                                              ; preds = %354, %337
  %339 = load i32, ptr %43, align 2
  %340 = icmp slt i32 %339, 3
  br i1 %340, label %341, label %357

341:                                              ; preds = %338
  %342 = load ptr, ptr %41, align 2
  %343 = load i32, ptr %42, align 2
  %344 = getelementptr inbounds i64, ptr %342, i32 %343
  %345 = load volatile i64, ptr %344, align 8
  %346 = load ptr, ptr %41, align 2
  %347 = load i32, ptr %43, align 2
  %348 = getelementptr inbounds i64, ptr %346, i32 %347
  %349 = load volatile i64, ptr %348, align 8
  %350 = call i64 @llvm.smul.fix.sat.i64(i64 %345, i64 %349, i32 31)
  store i64 %350, ptr %44, align 8
  %351 = load i64, ptr %44, align 8
  store i64 %351, ptr %45, align 8
  %352 = load i64, ptr %45, align 8
  %353 = trunc i64 %352 to i32
  call void @report(i32 noundef %353) #3
  br label %354

354:                                              ; preds = %341
  %355 = load i32, ptr %43, align 2
  %356 = add nsw i32 %355, 1
  store i32 %356, ptr %43, align 2
  br label %338, !llvm.loop !21

357:                                              ; preds = %338
  br label %358

358:                                              ; preds = %357
  %359 = load i32, ptr %42, align 2
  %360 = add nsw i32 %359, 1
  store i32 %360, ptr %42, align 2
  br label %334, !llvm.loop !22

361:                                              ; preds = %334
  br label %362

362:                                              ; preds = %361
  br label %363

363:                                              ; preds = %362
  store ptr @usa, ptr %46, align 2
  store i32 0, ptr %47, align 2
  br label %364

364:                                              ; preds = %388, %363
  %365 = load i32, ptr %47, align 2
  %366 = icmp slt i32 %365, 3
  br i1 %366, label %367, label %391

367:                                              ; preds = %364
  store i32 0, ptr %48, align 2
  br label %368

368:                                              ; preds = %384, %367
  %369 = load i32, ptr %48, align 2
  %370 = icmp slt i32 %369, 3
  br i1 %370, label %371, label %387

371:                                              ; preds = %368
  %372 = load ptr, ptr %46, align 2
  %373 = load i32, ptr %47, align 2
  %374 = getelementptr inbounds i16, ptr %372, i32 %373
  %375 = load volatile i16, ptr %374, align 2
  %376 = load ptr, ptr %46, align 2
  %377 = load i32, ptr %48, align 2
  %378 = getelementptr inbounds i16, ptr %376, i32 %377
  %379 = load volatile i16, ptr %378, align 2
  %380 = call i16 @llvm.umul.fix.sat.i16(i16 %375, i16 %379, i32 8)
  store i16 %380, ptr %49, align 2
  %381 = load i16, ptr %49, align 2
  store i16 %381, ptr %50, align 2
  %382 = load i16, ptr %50, align 2
  %383 = zext i16 %382 to i32
  call void @report(i32 noundef %383) #3
  br label %384

384:                                              ; preds = %371
  %385 = load i32, ptr %48, align 2
  %386 = add nsw i32 %385, 1
  store i32 %386, ptr %48, align 2
  br label %368, !llvm.loop !23

387:                                              ; preds = %368
  br label %388

388:                                              ; preds = %387
  %389 = load i32, ptr %47, align 2
  %390 = add nsw i32 %389, 1
  store i32 %390, ptr %47, align 2
  br label %364, !llvm.loop !24

391:                                              ; preds = %364
  br label %392

392:                                              ; preds = %391
  br label %393

393:                                              ; preds = %392
  store ptr @ua, ptr %51, align 2
  store i32 0, ptr %52, align 2
  br label %394

394:                                              ; preds = %417, %393
  %395 = load i32, ptr %52, align 2
  %396 = icmp slt i32 %395, 3
  br i1 %396, label %397, label %420

397:                                              ; preds = %394
  store i32 0, ptr %53, align 2
  br label %398

398:                                              ; preds = %413, %397
  %399 = load i32, ptr %53, align 2
  %400 = icmp slt i32 %399, 3
  br i1 %400, label %401, label %416

401:                                              ; preds = %398
  %402 = load ptr, ptr %51, align 2
  %403 = load i32, ptr %52, align 2
  %404 = getelementptr inbounds i32, ptr %402, i32 %403
  %405 = load volatile i32, ptr %404, align 4
  %406 = load ptr, ptr %51, align 2
  %407 = load i32, ptr %53, align 2
  %408 = getelementptr inbounds i32, ptr %406, i32 %407
  %409 = load volatile i32, ptr %408, align 4
  %410 = call i32 @llvm.umul.fix.sat.i32(i32 %405, i32 %409, i32 16)
  store i32 %410, ptr %54, align 4
  %411 = load i32, ptr %54, align 4
  store i32 %411, ptr %55, align 4
  %412 = load i32, ptr %55, align 4
  call void @report(i32 noundef %412) #3
  br label %413

413:                                              ; preds = %401
  %414 = load i32, ptr %53, align 2
  %415 = add nsw i32 %414, 1
  store i32 %415, ptr %53, align 2
  br label %398, !llvm.loop !25

416:                                              ; preds = %398
  br label %417

417:                                              ; preds = %416
  %418 = load i32, ptr %52, align 2
  %419 = add nsw i32 %418, 1
  store i32 %419, ptr %52, align 2
  br label %394, !llvm.loop !26

420:                                              ; preds = %394
  br label %421

421:                                              ; preds = %420
  br label %422

422:                                              ; preds = %421
  store ptr @ula, ptr %56, align 2
  store i32 0, ptr %57, align 2
  br label %423

423:                                              ; preds = %447, %422
  %424 = load i32, ptr %57, align 2
  %425 = icmp slt i32 %424, 3
  br i1 %425, label %426, label %450

426:                                              ; preds = %423
  store i32 0, ptr %58, align 2
  br label %427

427:                                              ; preds = %443, %426
  %428 = load i32, ptr %58, align 2
  %429 = icmp slt i32 %428, 3
  br i1 %429, label %430, label %446

430:                                              ; preds = %427
  %431 = load ptr, ptr %56, align 2
  %432 = load i32, ptr %57, align 2
  %433 = getelementptr inbounds i64, ptr %431, i32 %432
  %434 = load volatile i64, ptr %433, align 8
  %435 = load ptr, ptr %56, align 2
  %436 = load i32, ptr %58, align 2
  %437 = getelementptr inbounds i64, ptr %435, i32 %436
  %438 = load volatile i64, ptr %437, align 8
  %439 = call i64 @llvm.umul.fix.sat.i64(i64 %434, i64 %438, i32 32)
  store i64 %439, ptr %59, align 8
  %440 = load i64, ptr %59, align 8
  store i64 %440, ptr %60, align 8
  %441 = load i64, ptr %60, align 8
  %442 = trunc i64 %441 to i32
  call void @report(i32 noundef %442) #3
  br label %443

443:                                              ; preds = %430
  %444 = load i32, ptr %58, align 2
  %445 = add nsw i32 %444, 1
  store i32 %445, ptr %58, align 2
  br label %427, !llvm.loop !27

446:                                              ; preds = %427
  br label %447

447:                                              ; preds = %446
  %448 = load i32, ptr %57, align 2
  %449 = add nsw i32 %448, 1
  store i32 %449, ptr %57, align 2
  br label %423, !llvm.loop !28

450:                                              ; preds = %423
  br label %451

451:                                              ; preds = %450
  br label %452

452:                                              ; preds = %451
  store ptr @sf, ptr %61, align 2
  store volatile i32 3, ptr %62, align 2
  store i32 0, ptr %63, align 2
  br label %453

453:                                              ; preds = %467, %452
  %454 = load i32, ptr %63, align 2
  %455 = icmp slt i32 %454, 3
  br i1 %455, label %456, label %470

456:                                              ; preds = %453
  %457 = load ptr, ptr %61, align 2
  %458 = load i32, ptr %63, align 2
  %459 = getelementptr inbounds i8, ptr %457, i32 %458
  %460 = load volatile i8, ptr %459, align 1
  %461 = load volatile i32, ptr %62, align 2
  %462 = trunc i32 %461 to i8
  %463 = call i8 @llvm.sshl.sat.i8(i8 %460, i8 %462)
  store i8 %463, ptr %64, align 1
  %464 = load i8, ptr %64, align 1
  store i8 %464, ptr %65, align 1
  %465 = load i8, ptr %65, align 1
  %466 = zext i8 %465 to i32
  call void @report(i32 noundef %466) #3
  br label %467

467:                                              ; preds = %456
  %468 = load i32, ptr %63, align 2
  %469 = add nsw i32 %468, 1
  store i32 %469, ptr %63, align 2
  br label %453, !llvm.loop !29

470:                                              ; preds = %453
  br label %471

471:                                              ; preds = %470
  br label %472

472:                                              ; preds = %471
  store ptr @f, ptr %66, align 2
  store volatile i32 3, ptr %67, align 2
  store i32 0, ptr %68, align 2
  br label %473

473:                                              ; preds = %487, %472
  %474 = load i32, ptr %68, align 2
  %475 = icmp slt i32 %474, 3
  br i1 %475, label %476, label %490

476:                                              ; preds = %473
  %477 = load ptr, ptr %66, align 2
  %478 = load i32, ptr %68, align 2
  %479 = getelementptr inbounds i16, ptr %477, i32 %478
  %480 = load volatile i16, ptr %479, align 2
  %481 = load volatile i32, ptr %67, align 2
  %482 = trunc i32 %481 to i16
  %483 = call i16 @llvm.sshl.sat.i16(i16 %480, i16 %482)
  store i16 %483, ptr %69, align 2
  %484 = load i16, ptr %69, align 2
  store i16 %484, ptr %70, align 2
  %485 = load i16, ptr %70, align 2
  %486 = zext i16 %485 to i32
  call void @report(i32 noundef %486) #3
  br label %487

487:                                              ; preds = %476
  %488 = load i32, ptr %68, align 2
  %489 = add nsw i32 %488, 1
  store i32 %489, ptr %68, align 2
  br label %473, !llvm.loop !30

490:                                              ; preds = %473
  br label %491

491:                                              ; preds = %490
  br label %492

492:                                              ; preds = %491
  store ptr @lf, ptr %71, align 2
  store volatile i32 3, ptr %72, align 2
  store i32 0, ptr %73, align 2
  br label %493

493:                                              ; preds = %505, %492
  %494 = load i32, ptr %73, align 2
  %495 = icmp slt i32 %494, 3
  br i1 %495, label %496, label %508

496:                                              ; preds = %493
  %497 = load ptr, ptr %71, align 2
  %498 = load i32, ptr %73, align 2
  %499 = getelementptr inbounds i32, ptr %497, i32 %498
  %500 = load volatile i32, ptr %499, align 4
  %501 = load volatile i32, ptr %72, align 2
  %502 = call i32 @llvm.sshl.sat.i32(i32 %500, i32 %501)
  store i32 %502, ptr %74, align 4
  %503 = load i32, ptr %74, align 4
  store i32 %503, ptr %75, align 4
  %504 = load i32, ptr %75, align 4
  call void @report(i32 noundef %504) #3
  br label %505

505:                                              ; preds = %496
  %506 = load i32, ptr %73, align 2
  %507 = add nsw i32 %506, 1
  store i32 %507, ptr %73, align 2
  br label %493, !llvm.loop !31

508:                                              ; preds = %493
  br label %509

509:                                              ; preds = %508
  br label %510

510:                                              ; preds = %509
  store ptr @uf, ptr %76, align 2
  store volatile i32 3, ptr %77, align 2
  store i32 0, ptr %78, align 2
  br label %511

511:                                              ; preds = %525, %510
  %512 = load i32, ptr %78, align 2
  %513 = icmp slt i32 %512, 3
  br i1 %513, label %514, label %528

514:                                              ; preds = %511
  %515 = load ptr, ptr %76, align 2
  %516 = load i32, ptr %78, align 2
  %517 = getelementptr inbounds i16, ptr %515, i32 %516
  %518 = load volatile i16, ptr %517, align 2
  %519 = load volatile i32, ptr %77, align 2
  %520 = trunc i32 %519 to i16
  %521 = call i16 @llvm.ushl.sat.i16(i16 %518, i16 %520)
  store i16 %521, ptr %79, align 2
  %522 = load i16, ptr %79, align 2
  store i16 %522, ptr %80, align 2
  %523 = load i16, ptr %80, align 2
  %524 = zext i16 %523 to i32
  call void @report(i32 noundef %524) #3
  br label %525

525:                                              ; preds = %514
  %526 = load i32, ptr %78, align 2
  %527 = add nsw i32 %526, 1
  store i32 %527, ptr %78, align 2
  br label %511, !llvm.loop !32

528:                                              ; preds = %511
  br label %529

529:                                              ; preds = %528
  br label %530

530:                                              ; preds = %529
  store ptr @sa, ptr %81, align 2
  store volatile i32 3, ptr %82, align 2
  store i32 0, ptr %83, align 2
  br label %531

531:                                              ; preds = %545, %530
  %532 = load i32, ptr %83, align 2
  %533 = icmp slt i32 %532, 3
  br i1 %533, label %534, label %548

534:                                              ; preds = %531
  %535 = load ptr, ptr %81, align 2
  %536 = load i32, ptr %83, align 2
  %537 = getelementptr inbounds i16, ptr %535, i32 %536
  %538 = load volatile i16, ptr %537, align 2
  %539 = load volatile i32, ptr %82, align 2
  %540 = trunc i32 %539 to i16
  %541 = call i16 @llvm.sshl.sat.i16(i16 %538, i16 %540)
  store i16 %541, ptr %84, align 2
  %542 = load i16, ptr %84, align 2
  store i16 %542, ptr %85, align 2
  %543 = load i16, ptr %85, align 2
  %544 = zext i16 %543 to i32
  call void @report(i32 noundef %544) #3
  br label %545

545:                                              ; preds = %534
  %546 = load i32, ptr %83, align 2
  %547 = add nsw i32 %546, 1
  store i32 %547, ptr %83, align 2
  br label %531, !llvm.loop !33

548:                                              ; preds = %531
  br label %549

549:                                              ; preds = %548
  br label %550

550:                                              ; preds = %549
  store ptr @la, ptr %86, align 2
  store volatile i32 3, ptr %87, align 2
  store i32 0, ptr %88, align 2
  br label %551

551:                                              ; preds = %565, %550
  %552 = load i32, ptr %88, align 2
  %553 = icmp slt i32 %552, 3
  br i1 %553, label %554, label %568

554:                                              ; preds = %551
  %555 = load ptr, ptr %86, align 2
  %556 = load i32, ptr %88, align 2
  %557 = getelementptr inbounds i64, ptr %555, i32 %556
  %558 = load volatile i64, ptr %557, align 8
  %559 = load volatile i32, ptr %87, align 2
  %560 = zext i32 %559 to i64
  %561 = call i64 @llvm.sshl.sat.i64(i64 %558, i64 %560)
  store i64 %561, ptr %89, align 8
  %562 = load i64, ptr %89, align 8
  store i64 %562, ptr %90, align 8
  %563 = load i64, ptr %90, align 8
  %564 = trunc i64 %563 to i32
  call void @report(i32 noundef %564) #3
  br label %565

565:                                              ; preds = %554
  %566 = load i32, ptr %88, align 2
  %567 = add nsw i32 %566, 1
  store i32 %567, ptr %88, align 2
  br label %551, !llvm.loop !34

568:                                              ; preds = %551
  br label %569

569:                                              ; preds = %568
  br label %570

570:                                              ; preds = %569
  store ptr @ula, ptr %91, align 2
  store volatile i32 3, ptr %92, align 2
  store i32 0, ptr %93, align 2
  br label %571

571:                                              ; preds = %585, %570
  %572 = load i32, ptr %93, align 2
  %573 = icmp slt i32 %572, 3
  br i1 %573, label %574, label %588

574:                                              ; preds = %571
  %575 = load ptr, ptr %91, align 2
  %576 = load i32, ptr %93, align 2
  %577 = getelementptr inbounds i64, ptr %575, i32 %576
  %578 = load volatile i64, ptr %577, align 8
  %579 = load volatile i32, ptr %92, align 2
  %580 = zext i32 %579 to i64
  %581 = call i64 @llvm.ushl.sat.i64(i64 %578, i64 %580)
  store i64 %581, ptr %94, align 8
  %582 = load i64, ptr %94, align 8
  store i64 %582, ptr %95, align 8
  %583 = load i64, ptr %95, align 8
  %584 = trunc i64 %583 to i32
  call void @report(i32 noundef %584) #3
  br label %585

585:                                              ; preds = %574
  %586 = load i32, ptr %93, align 2
  %587 = add nsw i32 %586, 1
  store i32 %587, ptr %93, align 2
  br label %571, !llvm.loop !35

588:                                              ; preds = %571
  br label %589

589:                                              ; preds = %588
  ret i32 0
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i8 @llvm.smul.fix.sat.i8(i8, i8, i32 immarg) #1

declare void @report(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.smul.fix.sat.i16(i16, i16, i32 immarg) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.smul.fix.sat.i32(i32, i32, i32 immarg) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i8 @llvm.umul.fix.sat.i8(i8, i8, i32 immarg) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.umul.fix.sat.i16(i16, i16, i32 immarg) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.umul.fix.sat.i32(i32, i32, i32 immarg) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.smul.fix.sat.i64(i64, i64, i32 immarg) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.umul.fix.sat.i64(i64, i64, i32 immarg) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i8 @llvm.sshl.sat.i8(i8, i8) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.sshl.sat.i16(i16, i16) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.sshl.sat.i32(i32, i32) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.ushl.sat.i16(i16, i16) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.sshl.sat.i64(i64, i64) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.ushl.sat.i64(i64, i64) #1

attributes #0 = { noinline nounwind optnone "frame-pointer"="all" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" }
attributes #1 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #2 = { "frame-pointer"="all" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68020" "target-features"="+isa-68881" }
attributes #3 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{i32 7, !"frame-pointer", i32 2}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = distinct !{!4, !5}
!5 = !{!"llvm.loop.mustprogress"}
!6 = distinct !{!6, !5}
!7 = distinct !{!7, !5}
!8 = distinct !{!8, !5}
!9 = distinct !{!9, !5}
!10 = distinct !{!10, !5}
!11 = distinct !{!11, !5}
!12 = distinct !{!12, !5}
!13 = distinct !{!13, !5}
!14 = distinct !{!14, !5}
!15 = distinct !{!15, !5}
!16 = distinct !{!16, !5}
!17 = distinct !{!17, !5}
!18 = distinct !{!18, !5}
!19 = distinct !{!19, !5}
!20 = distinct !{!20, !5}
!21 = distinct !{!21, !5}
!22 = distinct !{!22, !5}
!23 = distinct !{!23, !5}
!24 = distinct !{!24, !5}
!25 = distinct !{!25, !5}
!26 = distinct !{!26, !5}
!27 = distinct !{!27, !5}
!28 = distinct !{!28, !5}
!29 = distinct !{!29, !5}
!30 = distinct !{!30, !5}
!31 = distinct !{!31, !5}
!32 = distinct !{!32, !5}
!33 = distinct !{!33, !5}
!34 = distinct !{!34, !5}
!35 = distinct !{!35, !5}
