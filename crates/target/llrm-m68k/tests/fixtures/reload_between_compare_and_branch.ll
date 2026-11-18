; llvm-reduce of known_loads_walks `f`: a reload sat between a compare and its branch
source_filename = "tests/run/c/known_loads_walks.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k"

; Function Attrs: noinline nounwind optsize
define zeroext i8 @f(i8 noundef zeroext %0, i8 noundef zeroext %1, ptr %2, ptr %3, i8 %4, ptr %5, i8 %6, i32 %7, i32 %8, i8 %9, i32 %10, i32 %11, i1 %12, i8 %13, i8 %14, i32 %15, i32 %16, i8 %17, i32 %18, i32 %19, i1 %20, i8 %21, ptr %22, i8 %23, i32 %24, i32 %25, i8 %26, i8 %27, ptr %28, i8 %29, i32 %30, ptr %31, i8 %32, i8 %33, i32 %34, i32 %35, i8 %36, i32 %37, i32 %38, i1 %39, i8 %40, i8 %41, i8 %42, i8 %43, ptr %44, i8 %45, i32 %46, ptr %47, i8 %48, i8 %49, i32 %50, i32 %51, i8 %52, i32 %53, i32 %54, i1 %55, i8 %56, i8 %57, i8 %58, i8 %59, i8 %60, i32 %61, i32 %62, i8 %63, i8 %64, i32 %65, ptr %66, i8 %67, i8 %68, i32 %69, i32 %70, i32 %71, ptr %72, i8 %73, i8 %74, i32 %75, i32 %76, i8 %77, i32 %78, i32 %79, i1 %80, ptr %81, i8 %82, i32 %83, i1 %84, i8 %85, ptr %86, i8 %87, i8 %88, i32 %89, i32 %90, i8 %91, i32 %92, i32 %93, i1 %94, i8 %95, i8 %96, i8 %97) #0 {
  %99 = alloca i8, align 1
  %100 = alloca i8, align 1
  %101 = alloca i8, align 1
  %102 = alloca i8, align 1
  %103 = alloca i8, align 1
  %104 = alloca i8, align 1
  %105 = alloca i8, align 1
  %106 = alloca i8, align 1
  %107 = alloca i8, align 1
  %108 = alloca i8, align 1
  %109 = alloca i8, align 1
  %110 = alloca i8, align 1
  %111 = alloca i8, align 1
  %112 = alloca i8, align 1
  store i8 %0, ptr %2, align 1
  store i8 %1, ptr %101, align 1
  br label %113

113:                                              ; preds = %155, %98
  %114 = load i8, ptr %3, align 1
  %115 = zext i8 %4 to i32
  %116 = load i8, ptr %5, align 1
  %117 = zext i8 %6 to i32
  %118 = add nsw i32 %7, 0
  %119 = trunc i32 %8 to i8
  %120 = zext i8 %9 to i32
  %121 = icmp slt i32 %10, %11
  br i1 %12, label %123, label %122

122:                                              ; preds = %113
  br label %158

123:                                              ; preds = %113
  br label %124

124:                                              ; preds = %130, %123
  br i1 false, label %126, label %125

125:                                              ; preds = %124
  br label %131

126:                                              ; preds = %124
  br label %127

127:                                              ; preds = %129, %126
  br i1 false, label %129, label %128

128:                                              ; preds = %127
  br label %130

129:                                              ; preds = %127
  br label %127

130:                                              ; preds = %128
  br label %124

131:                                              ; preds = %125
  br label %132

132:                                              ; preds = %154, %131
  %133 = load i8, ptr null, align 1
  %134 = zext i8 %13 to i32
  %135 = load i8, ptr %5, align 1
  %136 = zext i8 %14 to i32
  %137 = add nsw i32 %15, 0
  %138 = trunc i32 %16 to i8
  %139 = zext i8 %17 to i32
  %140 = icmp slt i32 %18, %19
  br i1 %20, label %142, label %141

141:                                              ; preds = %132
  br label %155

142:                                              ; preds = %132
  %143 = load i8, ptr %2, align 1
  %144 = zext i8 %21 to i32
  %145 = load i8, ptr %22, align 1
  %146 = zext i8 %23 to i32
  %147 = icmp sgt i32 %24, %25
  br i1 true, label %148, label %149

148:                                              ; preds = %142
  store i8 -1, ptr %22, align 1
  br label %154

149:                                              ; preds = %142
  br label %150

150:                                              ; preds = %153, %149
  %151 = load i8, ptr null, align 1
  br i1 false, label %153, label %152

152:                                              ; preds = %150
  br label %154

153:                                              ; preds = %150
  br label %150

154:                                              ; preds = %152, %148
  br label %132

155:                                              ; preds = %141
  %156 = load i8, ptr %3, align 1
  %157 = add i8 %26, 1
  store i8 %27, ptr %3, align 1
  br label %113

158:                                              ; preds = %122
  br label %159

159:                                              ; preds = %169, %158
  br i1 false, label %161, label %160

160:                                              ; preds = %159
  br label %170

161:                                              ; preds = %159
  br i1 false, label %163, label %162

162:                                              ; preds = %161
  br label %165

163:                                              ; preds = %161
  br label %164

164:                                              ; preds = %164, %163
  br label %164

165:                                              ; preds = %162
  br i1 false, label %166, label %167

166:                                              ; preds = %165
  br label %169

167:                                              ; preds = %165
  br label %168

168:                                              ; preds = %168, %167
  br label %168

169:                                              ; preds = %166
  br label %159

170:                                              ; preds = %160
  store i8 0, ptr %28, align 1
  br label %171

171:                                              ; preds = %202, %170
  %172 = load i8, ptr %28, align 1
  %173 = zext i8 %29 to i32
  %174 = load i8, ptr %101, align 1
  %175 = zext i8 %174 to i32
  %176 = add nsw i32 %175, 0
  %177 = trunc i32 %176 to i8
  %178 = zext i8 %177 to i32
  %179 = icmp slt i32 %30, %178
  br i1 %179, label %181, label %180

180:                                              ; preds = %171
  br label %205

181:                                              ; preds = %171
  br label %182

182:                                              ; preds = %199, %181
  %183 = load i8, ptr %31, align 1
  %184 = zext i8 %32 to i32
  %185 = load i8, ptr %5, align 1
  %186 = zext i8 %33 to i32
  %187 = add nsw i32 %34, 1
  %188 = trunc i32 %35 to i8
  %189 = zext i8 %36 to i32
  %190 = icmp slt i32 %37, %38
  br i1 %39, label %192, label %191

191:                                              ; preds = %182
  br label %202

192:                                              ; preds = %182
  br label %193

193:                                              ; preds = %198, %192
  br i1 false, label %195, label %194

194:                                              ; preds = %193
  br label %199

195:                                              ; preds = %193
  br i1 false, label %196, label %197

196:                                              ; preds = %195
  br label %198

197:                                              ; preds = %195
  br label %198

198:                                              ; preds = %197, %196
  br label %193

199:                                              ; preds = %194
  %200 = load i8, ptr %31, align 1
  %201 = add i8 %40, 1
  store i8 %41, ptr %31, align 1
  br label %182

202:                                              ; preds = %191
  %203 = load i8, ptr %28, align 1
  %204 = add i8 %42, 1
  store i8 %43, ptr %28, align 1
  br label %171

205:                                              ; preds = %180
  store i8 0, ptr %44, align 1
  br label %206

206:                                              ; preds = %238, %205
  %207 = load i8, ptr %44, align 1
  %208 = zext i8 %45 to i32
  %209 = load i8, ptr %101, align 1
  %210 = zext i8 %209 to i32
  %211 = add nsw i32 %210, 1
  %212 = trunc i32 %211 to i8
  %213 = zext i8 %212 to i32
  %214 = icmp slt i32 %46, %213
  br i1 %214, label %216, label %215

215:                                              ; preds = %206
  br label %241

216:                                              ; preds = %206
  br label %217

217:                                              ; preds = %235, %216
  %218 = load i8, ptr %47, align 1
  %219 = zext i8 %48 to i32
  %220 = load i8, ptr %5, align 1
  %221 = zext i8 %49 to i32
  %222 = add nsw i32 %50, 0
  %223 = trunc i32 %51 to i8
  %224 = zext i8 %52 to i32
  %225 = icmp slt i32 %53, %54
  br i1 %55, label %227, label %226

226:                                              ; preds = %217
  br label %238

227:                                              ; preds = %217
  br label %228

228:                                              ; preds = %234, %227
  br i1 false, label %230, label %229

229:                                              ; preds = %228
  br label %235

230:                                              ; preds = %228
  br label %231

231:                                              ; preds = %233, %230
  br i1 false, label %233, label %232

232:                                              ; preds = %231
  br label %234

233:                                              ; preds = %231
  br label %231

234:                                              ; preds = %232
  br label %228

235:                                              ; preds = %229
  %236 = load i8, ptr %47, align 1
  %237 = add i8 %56, 1
  store i8 %57, ptr %47, align 1
  br label %217

238:                                              ; preds = %226
  %239 = load i8, ptr %44, align 1
  %240 = add i8 %58, 1
  store i8 %59, ptr %44, align 1
  br label %206

241:                                              ; preds = %215
  br label %242

242:                                              ; preds = %327, %241
  store i8 0, ptr %109, align 1
  br label %243

243:                                              ; preds = %273, %242
  %244 = load i8, ptr %109, align 1
  %245 = zext i8 %244 to i32
  %246 = load i8, ptr %5, align 1
  %247 = zext i8 %60 to i32
  %248 = add nsw i32 %61, 1
  %249 = trunc i32 %62 to i8
  %250 = zext i8 %63 to i32
  %251 = icmp slt i32 %245, %250
  br i1 %251, label %253, label %252

252:                                              ; preds = %243
  br label %276

253:                                              ; preds = %243
  store i8 0, ptr %110, align 1
  br label %254

254:                                              ; preds = %264, %253
  %255 = load i8, ptr %110, align 1
  %256 = zext i8 %255 to i32
  %257 = load i8, ptr %101, align 1
  %258 = zext i8 %257 to i32
  %259 = add nsw i32 %258, 1
  %260 = trunc i32 %259 to i8
  %261 = zext i8 %260 to i32
  %262 = icmp slt i32 %256, %261
  br i1 %262, label %264, label %263

263:                                              ; preds = %254
  br label %273

264:                                              ; preds = %254
  %265 = load i8, ptr %102, align 1
  %266 = zext i8 %265 to i32
  %267 = load i8, ptr %2, align 1
  %268 = zext i8 %64 to i32
  %269 = or i32 %266, %65
  %270 = trunc i32 %269 to i8
  store i8 %270, ptr %66, align 1
  store i8 5, ptr %102, align 1
  %271 = load i8, ptr %110, align 1
  %272 = add i8 %271, 1
  store i8 %272, ptr %110, align 1
  br label %254

273:                                              ; preds = %263
  %274 = load i8, ptr %109, align 1
  %275 = add i8 %274, 1
  store i8 %275, ptr %109, align 1
  br label %243

276:                                              ; preds = %252
  %277 = load i8, ptr %22, align 1
  %278 = zext i8 %67 to i32
  %279 = load i8, ptr %22, align 1
  %280 = zext i8 %68 to i32
  %281 = add nsw i32 %69, %70
  %282 = trunc i32 %71 to i8
  store i8 %282, ptr %102, align 1
  store i8 0, ptr %72, align 1
  br label %283

283:                                              ; preds = %314, %276
  %284 = load i8, ptr %72, align 1
  %285 = zext i8 %73 to i32
  %286 = load i8, ptr %5, align 1
  %287 = zext i8 %74 to i32
  %288 = add nsw i32 %75, 1
  %289 = trunc i32 %76 to i8
  %290 = zext i8 %77 to i32
  %291 = icmp slt i32 %78, %79
  br i1 %80, label %293, label %292

292:                                              ; preds = %283
  br label %315

293:                                              ; preds = %283
  %294 = load i8, ptr %81, align 1
  %295 = zext i8 %82 to i32
  %296 = icmp ne i32 %83, 1
  br i1 %84, label %297, label %313

297:                                              ; preds = %293
  %298 = trunc i32 0 to i8
  store i8 %85, ptr %22, align 1
  br label %299

299:                                              ; preds = %309, %297
  %300 = load i8, ptr %86, align 1
  %301 = zext i8 %87 to i32
  %302 = load i8, ptr %5, align 1
  %303 = zext i8 %88 to i32
  %304 = add nsw i32 %89, 0
  %305 = trunc i32 %90 to i8
  %306 = zext i8 %91 to i32
  %307 = icmp slt i32 %92, %93
  br i1 %94, label %309, label %308

308:                                              ; preds = %299
  br label %314

309:                                              ; preds = %299
  %310 = load i8, ptr %66, align 1
  store i8 %95, ptr %81, align 1
  %311 = load i8, ptr %86, align 1
  %312 = add i8 %96, 1
  store i8 %97, ptr %86, align 1
  br label %299

313:                                              ; preds = %293
  store i8 0, ptr %2, align 1
  br label %314

314:                                              ; preds = %313, %308
  store i8 1, ptr %72, align 1
  br label %283

315:                                              ; preds = %292
  br label %316

316:                                              ; preds = %326, %315
  br i1 false, label %318, label %317

317:                                              ; preds = %316
  br label %327

318:                                              ; preds = %316
  br label %319

319:                                              ; preds = %325, %318
  br i1 false, label %321, label %320

320:                                              ; preds = %319
  br label %326

321:                                              ; preds = %319
  br label %322

322:                                              ; preds = %324, %321
  br i1 false, label %324, label %323

323:                                              ; preds = %322
  br label %325

324:                                              ; preds = %322
  br label %322

325:                                              ; preds = %323
  br label %319

326:                                              ; preds = %320
  br label %316

327:                                              ; preds = %317
  br label %242
}

attributes #0 = { noinline nounwind optsize "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
