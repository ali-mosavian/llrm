target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"

@dst = internal global [1200 x i8] zeroinitializer, align 16
@src = internal global [1200 x i8] zeroinitializer, align 16
@ref = internal global [1200 x i8] zeroinitializer, align 16
@bad = internal unnamed_addr global i1 false, align 4
@check_locals.x = internal global [700 x i8] zeroinitializer, align 16
@check_locals.y = internal global [700 x i8] zeroinitializer, align 16

define range(i32 0, 2) i32 @main() nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" {
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1

1:
  %2 = phi i64 [ 0, %0 ], [ %22, %1 ]
  %3 = trunc i64 %2 to i8
  %4 = mul i8 %3, 7
  %5 = or disjoint i8 %4, 3
  %6 = getelementptr inbounds nuw i8, ptr @src, i64 %2
  store i8 %5, ptr %6, align 4, !tbaa !10
  %7 = or disjoint i64 %2, 1
  %8 = trunc i64 %7 to i8
  %9 = mul i8 %8, 7
  %10 = add i8 %9, 3
  %11 = getelementptr inbounds nuw i8, ptr @src, i64 %7
  store i8 %10, ptr %11, align 1, !tbaa !10
  %12 = or disjoint i64 %2, 2
  %13 = trunc i64 %12 to i8
  %14 = mul i8 %13, 7
  %15 = add i8 %14, 3
  %16 = getelementptr inbounds nuw i8, ptr @src, i64 %12
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %2, 3
  %18 = trunc i64 %17 to i8
  %19 = mul i8 %18, 7
  %20 = add i8 %19, 3
  %21 = getelementptr inbounds nuw i8, ptr @src, i64 %17
  store i8 %20, ptr %21, align 1, !tbaa !10
  %22 = add nuw nsw i64 %2, 4
  %23 = icmp eq i64 %22, 1200
  br i1 %23, label %24, label %1, !llvm.loop !11

24:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp1()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 1)
  br label %42

25:
  %26 = add nuw nsw i64 %43, 1
  %27 = getelementptr inbounds nuw i8, ptr @dst, i64 %26
  %28 = load i8, ptr %27, align 1, !tbaa !10
  %29 = getelementptr inbounds nuw i8, ptr @ref, i64 %26
  %30 = load i8, ptr %29, align 1, !tbaa !10
  %31 = icmp eq i8 %28, %30
  br i1 %31, label %32, label %49

32:
  %33 = add nuw nsw i64 %43, 2
  %34 = getelementptr inbounds nuw i8, ptr @dst, i64 %33
  %35 = load i8, ptr %34, align 1, !tbaa !10
  %36 = getelementptr inbounds nuw i8, ptr @ref, i64 %33
  %37 = load i8, ptr %36, align 1, !tbaa !10
  %38 = icmp eq i8 %35, %37
  br i1 %38, label %39, label %49

39:
  %40 = add nuw nsw i64 %43, 3
  %41 = icmp eq i64 %40, 1200
  br i1 %41, label %50, label %42, !llvm.loop !144

42:
  %43 = phi i64 [ 0, %24 ], [ %40, %39 ]
  %44 = getelementptr inbounds nuw i8, ptr @dst, i64 %43
  %45 = load i8, ptr %44, align 1, !tbaa !10
  %46 = getelementptr inbounds nuw i8, ptr @ref, i64 %43
  %47 = load i8, ptr %46, align 1, !tbaa !10
  %48 = icmp eq i8 %45, %47
  br i1 %48, label %25, label %49

49:
  tail call void @report(i64 noundef 1)
  store i1 true, ptr @bad, align 4
  br label %51

50:
  br label %51

51:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %52

52:
  %53 = phi i64 [ 0, %51 ], [ %73, %52 ]
  %54 = trunc i64 %53 to i8
  %55 = mul i8 %54, 7
  %56 = or disjoint i8 %55, 3
  %57 = getelementptr inbounds nuw i8, ptr @src, i64 %53
  store i8 %56, ptr %57, align 4, !tbaa !10
  %58 = or disjoint i64 %53, 1
  %59 = trunc i64 %58 to i8
  %60 = mul i8 %59, 7
  %61 = add i8 %60, 3
  %62 = getelementptr inbounds nuw i8, ptr @src, i64 %58
  store i8 %61, ptr %62, align 1, !tbaa !10
  %63 = or disjoint i64 %53, 2
  %64 = trunc i64 %63 to i8
  %65 = mul i8 %64, 7
  %66 = add i8 %65, 3
  %67 = getelementptr inbounds nuw i8, ptr @src, i64 %63
  store i8 %66, ptr %67, align 2, !tbaa !10
  %68 = or disjoint i64 %53, 3
  %69 = trunc i64 %68 to i8
  %70 = mul i8 %69, 7
  %71 = add i8 %70, 3
  %72 = getelementptr inbounds nuw i8, ptr @src, i64 %68
  store i8 %71, ptr %72, align 1, !tbaa !10
  %73 = add nuw nsw i64 %53, 4
  %74 = icmp eq i64 %73, 1200
  br i1 %74, label %75, label %52, !llvm.loop !11

75:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr1()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 1)
  br label %93

76:
  %77 = add nuw nsw i64 %94, 1
  %78 = getelementptr inbounds nuw i8, ptr @dst, i64 %77
  %79 = load i8, ptr %78, align 1, !tbaa !10
  %80 = getelementptr inbounds nuw i8, ptr @ref, i64 %77
  %81 = load i8, ptr %80, align 1, !tbaa !10
  %82 = icmp eq i8 %79, %81
  br i1 %82, label %83, label %100

83:
  %84 = add nuw nsw i64 %94, 2
  %85 = getelementptr inbounds nuw i8, ptr @dst, i64 %84
  %86 = load i8, ptr %85, align 1, !tbaa !10
  %87 = getelementptr inbounds nuw i8, ptr @ref, i64 %84
  %88 = load i8, ptr %87, align 1, !tbaa !10
  %89 = icmp eq i8 %86, %88
  br i1 %89, label %90, label %100

90:
  %91 = add nuw nsw i64 %94, 3
  %92 = icmp eq i64 %91, 1200
  br i1 %92, label %101, label %93, !llvm.loop !144

93:
  %94 = phi i64 [ 0, %75 ], [ %91, %90 ]
  %95 = getelementptr inbounds nuw i8, ptr @dst, i64 %94
  %96 = load i8, ptr %95, align 1, !tbaa !10
  %97 = getelementptr inbounds nuw i8, ptr @ref, i64 %94
  %98 = load i8, ptr %97, align 1, !tbaa !10
  %99 = icmp eq i8 %96, %98
  br i1 %99, label %76, label %100

100:
  tail call void @report(i64 noundef 1)
  store i1 true, ptr @bad, align 4
  br label %102

101:
  br label %102

102:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %103

103:
  %104 = phi i64 [ 0, %102 ], [ %124, %103 ]
  %105 = trunc i64 %104 to i8
  %106 = mul i8 %105, 7
  %107 = or disjoint i8 %106, 3
  %108 = getelementptr inbounds nuw i8, ptr @src, i64 %104
  store i8 %107, ptr %108, align 4, !tbaa !10
  %109 = or disjoint i64 %104, 1
  %110 = trunc i64 %109 to i8
  %111 = mul i8 %110, 7
  %112 = add i8 %111, 3
  %113 = getelementptr inbounds nuw i8, ptr @src, i64 %109
  store i8 %112, ptr %113, align 1, !tbaa !10
  %114 = or disjoint i64 %104, 2
  %115 = trunc i64 %114 to i8
  %116 = mul i8 %115, 7
  %117 = add i8 %116, 3
  %118 = getelementptr inbounds nuw i8, ptr @src, i64 %114
  store i8 %117, ptr %118, align 2, !tbaa !10
  %119 = or disjoint i64 %104, 3
  %120 = trunc i64 %119 to i8
  %121 = mul i8 %120, 7
  %122 = add i8 %121, 3
  %123 = getelementptr inbounds nuw i8, ptr @src, i64 %119
  store i8 %122, ptr %123, align 1, !tbaa !10
  %124 = add nuw nsw i64 %104, 4
  %125 = icmp eq i64 %124, 1200
  br i1 %125, label %126, label %103, !llvm.loop !11

126:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c71()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 1)
  br label %144

127:
  %128 = add nuw nsw i64 %145, 1
  %129 = getelementptr inbounds nuw i8, ptr @dst, i64 %128
  %130 = load i8, ptr %129, align 1, !tbaa !10
  %131 = getelementptr inbounds nuw i8, ptr @ref, i64 %128
  %132 = load i8, ptr %131, align 1, !tbaa !10
  %133 = icmp eq i8 %130, %132
  br i1 %133, label %134, label %151

134:
  %135 = add nuw nsw i64 %145, 2
  %136 = getelementptr inbounds nuw i8, ptr @dst, i64 %135
  %137 = load i8, ptr %136, align 1, !tbaa !10
  %138 = getelementptr inbounds nuw i8, ptr @ref, i64 %135
  %139 = load i8, ptr %138, align 1, !tbaa !10
  %140 = icmp eq i8 %137, %139
  br i1 %140, label %141, label %151

141:
  %142 = add nuw nsw i64 %145, 3
  %143 = icmp eq i64 %142, 1200
  br i1 %143, label %152, label %144, !llvm.loop !144

144:
  %145 = phi i64 [ 0, %126 ], [ %142, %141 ]
  %146 = getelementptr inbounds nuw i8, ptr @dst, i64 %145
  %147 = load i8, ptr %146, align 1, !tbaa !10
  %148 = getelementptr inbounds nuw i8, ptr @ref, i64 %145
  %149 = load i8, ptr %148, align 1, !tbaa !10
  %150 = icmp eq i8 %147, %149
  br i1 %150, label %127, label %151

151:
  tail call void @report(i64 noundef 1)
  store i1 true, ptr @bad, align 4
  br label %153

152:
  br label %153

153:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %154

154:
  %155 = phi i64 [ 0, %153 ], [ %175, %154 ]
  %156 = trunc i64 %155 to i8
  %157 = mul i8 %156, 7
  %158 = or disjoint i8 %157, 3
  %159 = getelementptr inbounds nuw i8, ptr @src, i64 %155
  store i8 %158, ptr %159, align 4, !tbaa !10
  %160 = or disjoint i64 %155, 1
  %161 = trunc i64 %160 to i8
  %162 = mul i8 %161, 7
  %163 = add i8 %162, 3
  %164 = getelementptr inbounds nuw i8, ptr @src, i64 %160
  store i8 %163, ptr %164, align 1, !tbaa !10
  %165 = or disjoint i64 %155, 2
  %166 = trunc i64 %165 to i8
  %167 = mul i8 %166, 7
  %168 = add i8 %167, 3
  %169 = getelementptr inbounds nuw i8, ptr @src, i64 %165
  store i8 %168, ptr %169, align 2, !tbaa !10
  %170 = or disjoint i64 %155, 3
  %171 = trunc i64 %170 to i8
  %172 = mul i8 %171, 7
  %173 = add i8 %172, 3
  %174 = getelementptr inbounds nuw i8, ptr @src, i64 %170
  store i8 %173, ptr %174, align 1, !tbaa !10
  %175 = add nuw nsw i64 %155, 4
  %176 = icmp eq i64 %175, 1200
  br i1 %176, label %177, label %154, !llvm.loop !11

177:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr1()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 1)
  br label %195

178:
  %179 = add nuw nsw i64 %196, 1
  %180 = getelementptr inbounds nuw i8, ptr @dst, i64 %179
  %181 = load i8, ptr %180, align 1, !tbaa !10
  %182 = getelementptr inbounds nuw i8, ptr @ref, i64 %179
  %183 = load i8, ptr %182, align 1, !tbaa !10
  %184 = icmp eq i8 %181, %183
  br i1 %184, label %185, label %202

185:
  %186 = add nuw nsw i64 %196, 2
  %187 = getelementptr inbounds nuw i8, ptr @dst, i64 %186
  %188 = load i8, ptr %187, align 1, !tbaa !10
  %189 = getelementptr inbounds nuw i8, ptr @ref, i64 %186
  %190 = load i8, ptr %189, align 1, !tbaa !10
  %191 = icmp eq i8 %188, %190
  br i1 %191, label %192, label %202

192:
  %193 = add nuw nsw i64 %196, 3
  %194 = icmp eq i64 %193, 1200
  br i1 %194, label %203, label %195, !llvm.loop !144

195:
  %196 = phi i64 [ 0, %177 ], [ %193, %192 ]
  %197 = getelementptr inbounds nuw i8, ptr @dst, i64 %196
  %198 = load i8, ptr %197, align 1, !tbaa !10
  %199 = getelementptr inbounds nuw i8, ptr @ref, i64 %196
  %200 = load i8, ptr %199, align 1, !tbaa !10
  %201 = icmp eq i8 %198, %200
  br i1 %201, label %178, label %202

202:
  tail call void @report(i64 noundef 1)
  store i1 true, ptr @bad, align 4
  br label %204

203:
  br label %204

204:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %205

205:
  %206 = phi i64 [ 0, %204 ], [ %226, %205 ]
  %207 = trunc i64 %206 to i8
  %208 = mul i8 %207, 7
  %209 = or disjoint i8 %208, 3
  %210 = getelementptr inbounds nuw i8, ptr @src, i64 %206
  store i8 %209, ptr %210, align 4, !tbaa !10
  %211 = or disjoint i64 %206, 1
  %212 = trunc i64 %211 to i8
  %213 = mul i8 %212, 7
  %214 = add i8 %213, 3
  %215 = getelementptr inbounds nuw i8, ptr @src, i64 %211
  store i8 %214, ptr %215, align 1, !tbaa !10
  %216 = or disjoint i64 %206, 2
  %217 = trunc i64 %216 to i8
  %218 = mul i8 %217, 7
  %219 = add i8 %218, 3
  %220 = getelementptr inbounds nuw i8, ptr @src, i64 %216
  store i8 %219, ptr %220, align 2, !tbaa !10
  %221 = or disjoint i64 %206, 3
  %222 = trunc i64 %221 to i8
  %223 = mul i8 %222, 7
  %224 = add i8 %223, 3
  %225 = getelementptr inbounds nuw i8, ptr @src, i64 %221
  store i8 %224, ptr %225, align 1, !tbaa !10
  %226 = add nuw nsw i64 %206, 4
  %227 = icmp eq i64 %226, 1200
  br i1 %227, label %228, label %205, !llvm.loop !11

228:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  %229 = load i8, ptr getelementptr inbounds nuw (i8, ptr @dst, i64 1)
  tail call void @mv1(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), i8 %229)
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 1)
  br label %247

230:
  %231 = add nuw nsw i64 %248, 1
  %232 = getelementptr inbounds nuw i8, ptr @dst, i64 %231
  %233 = load i8, ptr %232, align 1, !tbaa !10
  %234 = getelementptr inbounds nuw i8, ptr @ref, i64 %231
  %235 = load i8, ptr %234, align 1, !tbaa !10
  %236 = icmp eq i8 %233, %235
  br i1 %236, label %237, label %254

237:
  %238 = add nuw nsw i64 %248, 2
  %239 = getelementptr inbounds nuw i8, ptr @dst, i64 %238
  %240 = load i8, ptr %239, align 1, !tbaa !10
  %241 = getelementptr inbounds nuw i8, ptr @ref, i64 %238
  %242 = load i8, ptr %241, align 1, !tbaa !10
  %243 = icmp eq i8 %240, %242
  br i1 %243, label %244, label %254

244:
  %245 = add nuw nsw i64 %248, 3
  %246 = icmp eq i64 %245, 1200
  br i1 %246, label %255, label %247, !llvm.loop !144

247:
  %248 = phi i64 [ 0, %228 ], [ %245, %244 ]
  %249 = getelementptr inbounds nuw i8, ptr @dst, i64 %248
  %250 = load i8, ptr %249, align 1, !tbaa !10
  %251 = getelementptr inbounds nuw i8, ptr @ref, i64 %248
  %252 = load i8, ptr %251, align 1, !tbaa !10
  %253 = icmp eq i8 %250, %252
  br i1 %253, label %230, label %254

254:
  tail call void @report(i64 noundef 1)
  store i1 true, ptr @bad, align 4
  br label %256

255:
  br label %256

256:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %257

257:
  %258 = phi i64 [ 0, %256 ], [ %278, %257 ]
  %259 = trunc i64 %258 to i8
  %260 = mul i8 %259, 7
  %261 = or disjoint i8 %260, 3
  %262 = getelementptr inbounds nuw i8, ptr @src, i64 %258
  store i8 %261, ptr %262, align 4, !tbaa !10
  %263 = or disjoint i64 %258, 1
  %264 = trunc i64 %263 to i8
  %265 = mul i8 %264, 7
  %266 = add i8 %265, 3
  %267 = getelementptr inbounds nuw i8, ptr @src, i64 %263
  store i8 %266, ptr %267, align 1, !tbaa !10
  %268 = or disjoint i64 %258, 2
  %269 = trunc i64 %268 to i8
  %270 = mul i8 %269, 7
  %271 = add i8 %270, 3
  %272 = getelementptr inbounds nuw i8, ptr @src, i64 %268
  store i8 %271, ptr %272, align 2, !tbaa !10
  %273 = or disjoint i64 %258, 3
  %274 = trunc i64 %273 to i8
  %275 = mul i8 %274, 7
  %276 = add i8 %275, 3
  %277 = getelementptr inbounds nuw i8, ptr @src, i64 %273
  store i8 %276, ptr %277, align 1, !tbaa !10
  %278 = add nuw nsw i64 %258, 4
  %279 = icmp eq i64 %278, 1200
  br i1 %279, label %280, label %257, !llvm.loop !11

280:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  %281 = load i8, ptr getelementptr inbounds nuw (i8, ptr @dst, i64 4), align 4
  tail call void @mv1(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), i8 %281)
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 1)
  br label %299

282:
  %283 = add nuw nsw i64 %300, 1
  %284 = getelementptr inbounds nuw i8, ptr @dst, i64 %283
  %285 = load i8, ptr %284, align 1, !tbaa !10
  %286 = getelementptr inbounds nuw i8, ptr @ref, i64 %283
  %287 = load i8, ptr %286, align 1, !tbaa !10
  %288 = icmp eq i8 %285, %287
  br i1 %288, label %289, label %306

289:
  %290 = add nuw nsw i64 %300, 2
  %291 = getelementptr inbounds nuw i8, ptr @dst, i64 %290
  %292 = load i8, ptr %291, align 1, !tbaa !10
  %293 = getelementptr inbounds nuw i8, ptr @ref, i64 %290
  %294 = load i8, ptr %293, align 1, !tbaa !10
  %295 = icmp eq i8 %292, %294
  br i1 %295, label %296, label %306

296:
  %297 = add nuw nsw i64 %300, 3
  %298 = icmp eq i64 %297, 1200
  br i1 %298, label %307, label %299, !llvm.loop !144

299:
  %300 = phi i64 [ 0, %280 ], [ %297, %296 ]
  %301 = getelementptr inbounds nuw i8, ptr @dst, i64 %300
  %302 = load i8, ptr %301, align 1, !tbaa !10
  %303 = getelementptr inbounds nuw i8, ptr @ref, i64 %300
  %304 = load i8, ptr %303, align 1, !tbaa !10
  %305 = icmp eq i8 %302, %304
  br i1 %305, label %282, label %306

306:
  tail call void @report(i64 noundef 1)
  store i1 true, ptr @bad, align 4
  br label %308

307:
  br label %308

308:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %309

309:
  %310 = phi i64 [ 0, %308 ], [ %330, %309 ]
  %311 = trunc i64 %310 to i8
  %312 = mul i8 %311, 7
  %313 = or disjoint i8 %312, 3
  %314 = getelementptr inbounds nuw i8, ptr @src, i64 %310
  store i8 %313, ptr %314, align 4, !tbaa !10
  %315 = or disjoint i64 %310, 1
  %316 = trunc i64 %315 to i8
  %317 = mul i8 %316, 7
  %318 = add i8 %317, 3
  %319 = getelementptr inbounds nuw i8, ptr @src, i64 %315
  store i8 %318, ptr %319, align 1, !tbaa !10
  %320 = or disjoint i64 %310, 2
  %321 = trunc i64 %320 to i8
  %322 = mul i8 %321, 7
  %323 = add i8 %322, 3
  %324 = getelementptr inbounds nuw i8, ptr @src, i64 %320
  store i8 %323, ptr %324, align 2, !tbaa !10
  %325 = or disjoint i64 %310, 3
  %326 = trunc i64 %325 to i8
  %327 = mul i8 %326, 7
  %328 = add i8 %327, 3
  %329 = getelementptr inbounds nuw i8, ptr @src, i64 %325
  store i8 %328, ptr %329, align 1, !tbaa !10
  %330 = add nuw nsw i64 %310, 4
  %331 = icmp eq i64 %330, 1200
  br i1 %331, label %332, label %309, !llvm.loop !11

332:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp2()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 2)
  br label %350

333:
  %334 = add nuw nsw i64 %351, 1
  %335 = getelementptr inbounds nuw i8, ptr @dst, i64 %334
  %336 = load i8, ptr %335, align 1, !tbaa !10
  %337 = getelementptr inbounds nuw i8, ptr @ref, i64 %334
  %338 = load i8, ptr %337, align 1, !tbaa !10
  %339 = icmp eq i8 %336, %338
  br i1 %339, label %340, label %357

340:
  %341 = add nuw nsw i64 %351, 2
  %342 = getelementptr inbounds nuw i8, ptr @dst, i64 %341
  %343 = load i8, ptr %342, align 1, !tbaa !10
  %344 = getelementptr inbounds nuw i8, ptr @ref, i64 %341
  %345 = load i8, ptr %344, align 1, !tbaa !10
  %346 = icmp eq i8 %343, %345
  br i1 %346, label %347, label %357

347:
  %348 = add nuw nsw i64 %351, 3
  %349 = icmp eq i64 %348, 1200
  br i1 %349, label %358, label %350, !llvm.loop !144

350:
  %351 = phi i64 [ 0, %332 ], [ %348, %347 ]
  %352 = getelementptr inbounds nuw i8, ptr @dst, i64 %351
  %353 = load i8, ptr %352, align 1, !tbaa !10
  %354 = getelementptr inbounds nuw i8, ptr @ref, i64 %351
  %355 = load i8, ptr %354, align 1, !tbaa !10
  %356 = icmp eq i8 %353, %355
  br i1 %356, label %333, label %357

357:
  tail call void @report(i64 noundef 2)
  store i1 true, ptr @bad, align 4
  br label %359

358:
  br label %359

359:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %360

360:
  %361 = phi i64 [ 0, %359 ], [ %381, %360 ]
  %362 = trunc i64 %361 to i8
  %363 = mul i8 %362, 7
  %364 = or disjoint i8 %363, 3
  %365 = getelementptr inbounds nuw i8, ptr @src, i64 %361
  store i8 %364, ptr %365, align 4, !tbaa !10
  %366 = or disjoint i64 %361, 1
  %367 = trunc i64 %366 to i8
  %368 = mul i8 %367, 7
  %369 = add i8 %368, 3
  %370 = getelementptr inbounds nuw i8, ptr @src, i64 %366
  store i8 %369, ptr %370, align 1, !tbaa !10
  %371 = or disjoint i64 %361, 2
  %372 = trunc i64 %371 to i8
  %373 = mul i8 %372, 7
  %374 = add i8 %373, 3
  %375 = getelementptr inbounds nuw i8, ptr @src, i64 %371
  store i8 %374, ptr %375, align 2, !tbaa !10
  %376 = or disjoint i64 %361, 3
  %377 = trunc i64 %376 to i8
  %378 = mul i8 %377, 7
  %379 = add i8 %378, 3
  %380 = getelementptr inbounds nuw i8, ptr @src, i64 %376
  store i8 %379, ptr %380, align 1, !tbaa !10
  %381 = add nuw nsw i64 %361, 4
  %382 = icmp eq i64 %381, 1200
  br i1 %382, label %383, label %360, !llvm.loop !11

383:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr2()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 2)
  br label %401

384:
  %385 = add nuw nsw i64 %402, 1
  %386 = getelementptr inbounds nuw i8, ptr @dst, i64 %385
  %387 = load i8, ptr %386, align 1, !tbaa !10
  %388 = getelementptr inbounds nuw i8, ptr @ref, i64 %385
  %389 = load i8, ptr %388, align 1, !tbaa !10
  %390 = icmp eq i8 %387, %389
  br i1 %390, label %391, label %408

391:
  %392 = add nuw nsw i64 %402, 2
  %393 = getelementptr inbounds nuw i8, ptr @dst, i64 %392
  %394 = load i8, ptr %393, align 1, !tbaa !10
  %395 = getelementptr inbounds nuw i8, ptr @ref, i64 %392
  %396 = load i8, ptr %395, align 1, !tbaa !10
  %397 = icmp eq i8 %394, %396
  br i1 %397, label %398, label %408

398:
  %399 = add nuw nsw i64 %402, 3
  %400 = icmp eq i64 %399, 1200
  br i1 %400, label %409, label %401, !llvm.loop !144

401:
  %402 = phi i64 [ 0, %383 ], [ %399, %398 ]
  %403 = getelementptr inbounds nuw i8, ptr @dst, i64 %402
  %404 = load i8, ptr %403, align 1, !tbaa !10
  %405 = getelementptr inbounds nuw i8, ptr @ref, i64 %402
  %406 = load i8, ptr %405, align 1, !tbaa !10
  %407 = icmp eq i8 %404, %406
  br i1 %407, label %384, label %408

408:
  tail call void @report(i64 noundef 2)
  store i1 true, ptr @bad, align 4
  br label %410

409:
  br label %410

410:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %411

411:
  %412 = phi i64 [ 0, %410 ], [ %432, %411 ]
  %413 = trunc i64 %412 to i8
  %414 = mul i8 %413, 7
  %415 = or disjoint i8 %414, 3
  %416 = getelementptr inbounds nuw i8, ptr @src, i64 %412
  store i8 %415, ptr %416, align 4, !tbaa !10
  %417 = or disjoint i64 %412, 1
  %418 = trunc i64 %417 to i8
  %419 = mul i8 %418, 7
  %420 = add i8 %419, 3
  %421 = getelementptr inbounds nuw i8, ptr @src, i64 %417
  store i8 %420, ptr %421, align 1, !tbaa !10
  %422 = or disjoint i64 %412, 2
  %423 = trunc i64 %422 to i8
  %424 = mul i8 %423, 7
  %425 = add i8 %424, 3
  %426 = getelementptr inbounds nuw i8, ptr @src, i64 %422
  store i8 %425, ptr %426, align 2, !tbaa !10
  %427 = or disjoint i64 %412, 3
  %428 = trunc i64 %427 to i8
  %429 = mul i8 %428, 7
  %430 = add i8 %429, 3
  %431 = getelementptr inbounds nuw i8, ptr @src, i64 %427
  store i8 %430, ptr %431, align 1, !tbaa !10
  %432 = add nuw nsw i64 %412, 4
  %433 = icmp eq i64 %432, 1200
  br i1 %433, label %434, label %411, !llvm.loop !11

434:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c72()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 2)
  br label %452

435:
  %436 = add nuw nsw i64 %453, 1
  %437 = getelementptr inbounds nuw i8, ptr @dst, i64 %436
  %438 = load i8, ptr %437, align 1, !tbaa !10
  %439 = getelementptr inbounds nuw i8, ptr @ref, i64 %436
  %440 = load i8, ptr %439, align 1, !tbaa !10
  %441 = icmp eq i8 %438, %440
  br i1 %441, label %442, label %459

442:
  %443 = add nuw nsw i64 %453, 2
  %444 = getelementptr inbounds nuw i8, ptr @dst, i64 %443
  %445 = load i8, ptr %444, align 1, !tbaa !10
  %446 = getelementptr inbounds nuw i8, ptr @ref, i64 %443
  %447 = load i8, ptr %446, align 1, !tbaa !10
  %448 = icmp eq i8 %445, %447
  br i1 %448, label %449, label %459

449:
  %450 = add nuw nsw i64 %453, 3
  %451 = icmp eq i64 %450, 1200
  br i1 %451, label %460, label %452, !llvm.loop !144

452:
  %453 = phi i64 [ 0, %434 ], [ %450, %449 ]
  %454 = getelementptr inbounds nuw i8, ptr @dst, i64 %453
  %455 = load i8, ptr %454, align 1, !tbaa !10
  %456 = getelementptr inbounds nuw i8, ptr @ref, i64 %453
  %457 = load i8, ptr %456, align 1, !tbaa !10
  %458 = icmp eq i8 %455, %457
  br i1 %458, label %435, label %459

459:
  tail call void @report(i64 noundef 2)
  store i1 true, ptr @bad, align 4
  br label %461

460:
  br label %461

461:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %462

462:
  %463 = phi i64 [ 0, %461 ], [ %483, %462 ]
  %464 = trunc i64 %463 to i8
  %465 = mul i8 %464, 7
  %466 = or disjoint i8 %465, 3
  %467 = getelementptr inbounds nuw i8, ptr @src, i64 %463
  store i8 %466, ptr %467, align 4, !tbaa !10
  %468 = or disjoint i64 %463, 1
  %469 = trunc i64 %468 to i8
  %470 = mul i8 %469, 7
  %471 = add i8 %470, 3
  %472 = getelementptr inbounds nuw i8, ptr @src, i64 %468
  store i8 %471, ptr %472, align 1, !tbaa !10
  %473 = or disjoint i64 %463, 2
  %474 = trunc i64 %473 to i8
  %475 = mul i8 %474, 7
  %476 = add i8 %475, 3
  %477 = getelementptr inbounds nuw i8, ptr @src, i64 %473
  store i8 %476, ptr %477, align 2, !tbaa !10
  %478 = or disjoint i64 %463, 3
  %479 = trunc i64 %478 to i8
  %480 = mul i8 %479, 7
  %481 = add i8 %480, 3
  %482 = getelementptr inbounds nuw i8, ptr @src, i64 %478
  store i8 %481, ptr %482, align 1, !tbaa !10
  %483 = add nuw nsw i64 %463, 4
  %484 = icmp eq i64 %483, 1200
  br i1 %484, label %485, label %462, !llvm.loop !11

485:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr2()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 2)
  br label %503

486:
  %487 = add nuw nsw i64 %504, 1
  %488 = getelementptr inbounds nuw i8, ptr @dst, i64 %487
  %489 = load i8, ptr %488, align 1, !tbaa !10
  %490 = getelementptr inbounds nuw i8, ptr @ref, i64 %487
  %491 = load i8, ptr %490, align 1, !tbaa !10
  %492 = icmp eq i8 %489, %491
  br i1 %492, label %493, label %510

493:
  %494 = add nuw nsw i64 %504, 2
  %495 = getelementptr inbounds nuw i8, ptr @dst, i64 %494
  %496 = load i8, ptr %495, align 1, !tbaa !10
  %497 = getelementptr inbounds nuw i8, ptr @ref, i64 %494
  %498 = load i8, ptr %497, align 1, !tbaa !10
  %499 = icmp eq i8 %496, %498
  br i1 %499, label %500, label %510

500:
  %501 = add nuw nsw i64 %504, 3
  %502 = icmp eq i64 %501, 1200
  br i1 %502, label %511, label %503, !llvm.loop !144

503:
  %504 = phi i64 [ 0, %485 ], [ %501, %500 ]
  %505 = getelementptr inbounds nuw i8, ptr @dst, i64 %504
  %506 = load i8, ptr %505, align 1, !tbaa !10
  %507 = getelementptr inbounds nuw i8, ptr @ref, i64 %504
  %508 = load i8, ptr %507, align 1, !tbaa !10
  %509 = icmp eq i8 %506, %508
  br i1 %509, label %486, label %510

510:
  tail call void @report(i64 noundef 2)
  store i1 true, ptr @bad, align 4
  br label %512

511:
  br label %512

512:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %513

513:
  %514 = phi i64 [ 0, %512 ], [ %534, %513 ]
  %515 = trunc i64 %514 to i8
  %516 = mul i8 %515, 7
  %517 = or disjoint i8 %516, 3
  %518 = getelementptr inbounds nuw i8, ptr @src, i64 %514
  store i8 %517, ptr %518, align 4, !tbaa !10
  %519 = or disjoint i64 %514, 1
  %520 = trunc i64 %519 to i8
  %521 = mul i8 %520, 7
  %522 = add i8 %521, 3
  %523 = getelementptr inbounds nuw i8, ptr @src, i64 %519
  store i8 %522, ptr %523, align 1, !tbaa !10
  %524 = or disjoint i64 %514, 2
  %525 = trunc i64 %524 to i8
  %526 = mul i8 %525, 7
  %527 = add i8 %526, 3
  %528 = getelementptr inbounds nuw i8, ptr @src, i64 %524
  store i8 %527, ptr %528, align 2, !tbaa !10
  %529 = or disjoint i64 %514, 3
  %530 = trunc i64 %529 to i8
  %531 = mul i8 %530, 7
  %532 = add i8 %531, 3
  %533 = getelementptr inbounds nuw i8, ptr @src, i64 %529
  store i8 %532, ptr %533, align 1, !tbaa !10
  %534 = add nuw nsw i64 %514, 4
  %535 = icmp eq i64 %534, 1200
  br i1 %535, label %536, label %513, !llvm.loop !11

536:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  %537 = load i16, ptr getelementptr inbounds nuw (i8, ptr @dst, i64 1)
  tail call void @mv2(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), i16 %537)
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 2)
  br label %555

538:
  %539 = add nuw nsw i64 %556, 1
  %540 = getelementptr inbounds nuw i8, ptr @dst, i64 %539
  %541 = load i8, ptr %540, align 1, !tbaa !10
  %542 = getelementptr inbounds nuw i8, ptr @ref, i64 %539
  %543 = load i8, ptr %542, align 1, !tbaa !10
  %544 = icmp eq i8 %541, %543
  br i1 %544, label %545, label %562

545:
  %546 = add nuw nsw i64 %556, 2
  %547 = getelementptr inbounds nuw i8, ptr @dst, i64 %546
  %548 = load i8, ptr %547, align 1, !tbaa !10
  %549 = getelementptr inbounds nuw i8, ptr @ref, i64 %546
  %550 = load i8, ptr %549, align 1, !tbaa !10
  %551 = icmp eq i8 %548, %550
  br i1 %551, label %552, label %562

552:
  %553 = add nuw nsw i64 %556, 3
  %554 = icmp eq i64 %553, 1200
  br i1 %554, label %563, label %555, !llvm.loop !144

555:
  %556 = phi i64 [ 0, %536 ], [ %553, %552 ]
  %557 = getelementptr inbounds nuw i8, ptr @dst, i64 %556
  %558 = load i8, ptr %557, align 1, !tbaa !10
  %559 = getelementptr inbounds nuw i8, ptr @ref, i64 %556
  %560 = load i8, ptr %559, align 1, !tbaa !10
  %561 = icmp eq i8 %558, %560
  br i1 %561, label %538, label %562

562:
  tail call void @report(i64 noundef 2)
  store i1 true, ptr @bad, align 4
  br label %564

563:
  br label %564

564:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %565

565:
  %566 = phi i64 [ 0, %564 ], [ %586, %565 ]
  %567 = trunc i64 %566 to i8
  %568 = mul i8 %567, 7
  %569 = or disjoint i8 %568, 3
  %570 = getelementptr inbounds nuw i8, ptr @src, i64 %566
  store i8 %569, ptr %570, align 4, !tbaa !10
  %571 = or disjoint i64 %566, 1
  %572 = trunc i64 %571 to i8
  %573 = mul i8 %572, 7
  %574 = add i8 %573, 3
  %575 = getelementptr inbounds nuw i8, ptr @src, i64 %571
  store i8 %574, ptr %575, align 1, !tbaa !10
  %576 = or disjoint i64 %566, 2
  %577 = trunc i64 %576 to i8
  %578 = mul i8 %577, 7
  %579 = add i8 %578, 3
  %580 = getelementptr inbounds nuw i8, ptr @src, i64 %576
  store i8 %579, ptr %580, align 2, !tbaa !10
  %581 = or disjoint i64 %566, 3
  %582 = trunc i64 %581 to i8
  %583 = mul i8 %582, 7
  %584 = add i8 %583, 3
  %585 = getelementptr inbounds nuw i8, ptr @src, i64 %581
  store i8 %584, ptr %585, align 1, !tbaa !10
  %586 = add nuw nsw i64 %566, 4
  %587 = icmp eq i64 %586, 1200
  br i1 %587, label %588, label %565, !llvm.loop !11

588:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  %589 = load i16, ptr getelementptr inbounds nuw (i8, ptr @dst, i64 4), align 4
  tail call void @mv2(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), i16 %589)
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 2)
  br label %607

590:
  %591 = add nuw nsw i64 %608, 1
  %592 = getelementptr inbounds nuw i8, ptr @dst, i64 %591
  %593 = load i8, ptr %592, align 1, !tbaa !10
  %594 = getelementptr inbounds nuw i8, ptr @ref, i64 %591
  %595 = load i8, ptr %594, align 1, !tbaa !10
  %596 = icmp eq i8 %593, %595
  br i1 %596, label %597, label %614

597:
  %598 = add nuw nsw i64 %608, 2
  %599 = getelementptr inbounds nuw i8, ptr @dst, i64 %598
  %600 = load i8, ptr %599, align 1, !tbaa !10
  %601 = getelementptr inbounds nuw i8, ptr @ref, i64 %598
  %602 = load i8, ptr %601, align 1, !tbaa !10
  %603 = icmp eq i8 %600, %602
  br i1 %603, label %604, label %614

604:
  %605 = add nuw nsw i64 %608, 3
  %606 = icmp eq i64 %605, 1200
  br i1 %606, label %615, label %607, !llvm.loop !144

607:
  %608 = phi i64 [ 0, %588 ], [ %605, %604 ]
  %609 = getelementptr inbounds nuw i8, ptr @dst, i64 %608
  %610 = load i8, ptr %609, align 1, !tbaa !10
  %611 = getelementptr inbounds nuw i8, ptr @ref, i64 %608
  %612 = load i8, ptr %611, align 1, !tbaa !10
  %613 = icmp eq i8 %610, %612
  br i1 %613, label %590, label %614

614:
  tail call void @report(i64 noundef 2)
  store i1 true, ptr @bad, align 4
  br label %616

615:
  br label %616

616:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %617

617:
  %618 = phi i64 [ 0, %616 ], [ %638, %617 ]
  %619 = trunc i64 %618 to i8
  %620 = mul i8 %619, 7
  %621 = or disjoint i8 %620, 3
  %622 = getelementptr inbounds nuw i8, ptr @src, i64 %618
  store i8 %621, ptr %622, align 4, !tbaa !10
  %623 = or disjoint i64 %618, 1
  %624 = trunc i64 %623 to i8
  %625 = mul i8 %624, 7
  %626 = add i8 %625, 3
  %627 = getelementptr inbounds nuw i8, ptr @src, i64 %623
  store i8 %626, ptr %627, align 1, !tbaa !10
  %628 = or disjoint i64 %618, 2
  %629 = trunc i64 %628 to i8
  %630 = mul i8 %629, 7
  %631 = add i8 %630, 3
  %632 = getelementptr inbounds nuw i8, ptr @src, i64 %628
  store i8 %631, ptr %632, align 2, !tbaa !10
  %633 = or disjoint i64 %618, 3
  %634 = trunc i64 %633 to i8
  %635 = mul i8 %634, 7
  %636 = add i8 %635, 3
  %637 = getelementptr inbounds nuw i8, ptr @src, i64 %633
  store i8 %636, ptr %637, align 1, !tbaa !10
  %638 = add nuw nsw i64 %618, 4
  %639 = icmp eq i64 %638, 1200
  br i1 %639, label %640, label %617, !llvm.loop !11

640:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp3()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 3)
  br label %658

641:
  %642 = add nuw nsw i64 %659, 1
  %643 = getelementptr inbounds nuw i8, ptr @dst, i64 %642
  %644 = load i8, ptr %643, align 1, !tbaa !10
  %645 = getelementptr inbounds nuw i8, ptr @ref, i64 %642
  %646 = load i8, ptr %645, align 1, !tbaa !10
  %647 = icmp eq i8 %644, %646
  br i1 %647, label %648, label %665

648:
  %649 = add nuw nsw i64 %659, 2
  %650 = getelementptr inbounds nuw i8, ptr @dst, i64 %649
  %651 = load i8, ptr %650, align 1, !tbaa !10
  %652 = getelementptr inbounds nuw i8, ptr @ref, i64 %649
  %653 = load i8, ptr %652, align 1, !tbaa !10
  %654 = icmp eq i8 %651, %653
  br i1 %654, label %655, label %665

655:
  %656 = add nuw nsw i64 %659, 3
  %657 = icmp eq i64 %656, 1200
  br i1 %657, label %666, label %658, !llvm.loop !144

658:
  %659 = phi i64 [ 0, %640 ], [ %656, %655 ]
  %660 = getelementptr inbounds nuw i8, ptr @dst, i64 %659
  %661 = load i8, ptr %660, align 1, !tbaa !10
  %662 = getelementptr inbounds nuw i8, ptr @ref, i64 %659
  %663 = load i8, ptr %662, align 1, !tbaa !10
  %664 = icmp eq i8 %661, %663
  br i1 %664, label %641, label %665

665:
  tail call void @report(i64 noundef 3)
  store i1 true, ptr @bad, align 4
  br label %667

666:
  br label %667

667:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %668

668:
  %669 = phi i64 [ 0, %667 ], [ %689, %668 ]
  %670 = trunc i64 %669 to i8
  %671 = mul i8 %670, 7
  %672 = or disjoint i8 %671, 3
  %673 = getelementptr inbounds nuw i8, ptr @src, i64 %669
  store i8 %672, ptr %673, align 4, !tbaa !10
  %674 = or disjoint i64 %669, 1
  %675 = trunc i64 %674 to i8
  %676 = mul i8 %675, 7
  %677 = add i8 %676, 3
  %678 = getelementptr inbounds nuw i8, ptr @src, i64 %674
  store i8 %677, ptr %678, align 1, !tbaa !10
  %679 = or disjoint i64 %669, 2
  %680 = trunc i64 %679 to i8
  %681 = mul i8 %680, 7
  %682 = add i8 %681, 3
  %683 = getelementptr inbounds nuw i8, ptr @src, i64 %679
  store i8 %682, ptr %683, align 2, !tbaa !10
  %684 = or disjoint i64 %669, 3
  %685 = trunc i64 %684 to i8
  %686 = mul i8 %685, 7
  %687 = add i8 %686, 3
  %688 = getelementptr inbounds nuw i8, ptr @src, i64 %684
  store i8 %687, ptr %688, align 1, !tbaa !10
  %689 = add nuw nsw i64 %669, 4
  %690 = icmp eq i64 %689, 1200
  br i1 %690, label %691, label %668, !llvm.loop !11

691:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr3()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 3)
  br label %709

692:
  %693 = add nuw nsw i64 %710, 1
  %694 = getelementptr inbounds nuw i8, ptr @dst, i64 %693
  %695 = load i8, ptr %694, align 1, !tbaa !10
  %696 = getelementptr inbounds nuw i8, ptr @ref, i64 %693
  %697 = load i8, ptr %696, align 1, !tbaa !10
  %698 = icmp eq i8 %695, %697
  br i1 %698, label %699, label %716

699:
  %700 = add nuw nsw i64 %710, 2
  %701 = getelementptr inbounds nuw i8, ptr @dst, i64 %700
  %702 = load i8, ptr %701, align 1, !tbaa !10
  %703 = getelementptr inbounds nuw i8, ptr @ref, i64 %700
  %704 = load i8, ptr %703, align 1, !tbaa !10
  %705 = icmp eq i8 %702, %704
  br i1 %705, label %706, label %716

706:
  %707 = add nuw nsw i64 %710, 3
  %708 = icmp eq i64 %707, 1200
  br i1 %708, label %717, label %709, !llvm.loop !144

709:
  %710 = phi i64 [ 0, %691 ], [ %707, %706 ]
  %711 = getelementptr inbounds nuw i8, ptr @dst, i64 %710
  %712 = load i8, ptr %711, align 1, !tbaa !10
  %713 = getelementptr inbounds nuw i8, ptr @ref, i64 %710
  %714 = load i8, ptr %713, align 1, !tbaa !10
  %715 = icmp eq i8 %712, %714
  br i1 %715, label %692, label %716

716:
  tail call void @report(i64 noundef 3)
  store i1 true, ptr @bad, align 4
  br label %718

717:
  br label %718

718:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %719

719:
  %720 = phi i64 [ 0, %718 ], [ %740, %719 ]
  %721 = trunc i64 %720 to i8
  %722 = mul i8 %721, 7
  %723 = or disjoint i8 %722, 3
  %724 = getelementptr inbounds nuw i8, ptr @src, i64 %720
  store i8 %723, ptr %724, align 4, !tbaa !10
  %725 = or disjoint i64 %720, 1
  %726 = trunc i64 %725 to i8
  %727 = mul i8 %726, 7
  %728 = add i8 %727, 3
  %729 = getelementptr inbounds nuw i8, ptr @src, i64 %725
  store i8 %728, ptr %729, align 1, !tbaa !10
  %730 = or disjoint i64 %720, 2
  %731 = trunc i64 %730 to i8
  %732 = mul i8 %731, 7
  %733 = add i8 %732, 3
  %734 = getelementptr inbounds nuw i8, ptr @src, i64 %730
  store i8 %733, ptr %734, align 2, !tbaa !10
  %735 = or disjoint i64 %720, 3
  %736 = trunc i64 %735 to i8
  %737 = mul i8 %736, 7
  %738 = add i8 %737, 3
  %739 = getelementptr inbounds nuw i8, ptr @src, i64 %735
  store i8 %738, ptr %739, align 1, !tbaa !10
  %740 = add nuw nsw i64 %720, 4
  %741 = icmp eq i64 %740, 1200
  br i1 %741, label %742, label %719, !llvm.loop !11

742:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c73()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 3)
  br label %760

743:
  %744 = add nuw nsw i64 %761, 1
  %745 = getelementptr inbounds nuw i8, ptr @dst, i64 %744
  %746 = load i8, ptr %745, align 1, !tbaa !10
  %747 = getelementptr inbounds nuw i8, ptr @ref, i64 %744
  %748 = load i8, ptr %747, align 1, !tbaa !10
  %749 = icmp eq i8 %746, %748
  br i1 %749, label %750, label %767

750:
  %751 = add nuw nsw i64 %761, 2
  %752 = getelementptr inbounds nuw i8, ptr @dst, i64 %751
  %753 = load i8, ptr %752, align 1, !tbaa !10
  %754 = getelementptr inbounds nuw i8, ptr @ref, i64 %751
  %755 = load i8, ptr %754, align 1, !tbaa !10
  %756 = icmp eq i8 %753, %755
  br i1 %756, label %757, label %767

757:
  %758 = add nuw nsw i64 %761, 3
  %759 = icmp eq i64 %758, 1200
  br i1 %759, label %768, label %760, !llvm.loop !144

760:
  %761 = phi i64 [ 0, %742 ], [ %758, %757 ]
  %762 = getelementptr inbounds nuw i8, ptr @dst, i64 %761
  %763 = load i8, ptr %762, align 1, !tbaa !10
  %764 = getelementptr inbounds nuw i8, ptr @ref, i64 %761
  %765 = load i8, ptr %764, align 1, !tbaa !10
  %766 = icmp eq i8 %763, %765
  br i1 %766, label %743, label %767

767:
  tail call void @report(i64 noundef 3)
  store i1 true, ptr @bad, align 4
  br label %769

768:
  br label %769

769:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %770

770:
  %771 = phi i64 [ 0, %769 ], [ %791, %770 ]
  %772 = trunc i64 %771 to i8
  %773 = mul i8 %772, 7
  %774 = or disjoint i8 %773, 3
  %775 = getelementptr inbounds nuw i8, ptr @src, i64 %771
  store i8 %774, ptr %775, align 4, !tbaa !10
  %776 = or disjoint i64 %771, 1
  %777 = trunc i64 %776 to i8
  %778 = mul i8 %777, 7
  %779 = add i8 %778, 3
  %780 = getelementptr inbounds nuw i8, ptr @src, i64 %776
  store i8 %779, ptr %780, align 1, !tbaa !10
  %781 = or disjoint i64 %771, 2
  %782 = trunc i64 %781 to i8
  %783 = mul i8 %782, 7
  %784 = add i8 %783, 3
  %785 = getelementptr inbounds nuw i8, ptr @src, i64 %781
  store i8 %784, ptr %785, align 2, !tbaa !10
  %786 = or disjoint i64 %771, 3
  %787 = trunc i64 %786 to i8
  %788 = mul i8 %787, 7
  %789 = add i8 %788, 3
  %790 = getelementptr inbounds nuw i8, ptr @src, i64 %786
  store i8 %789, ptr %790, align 1, !tbaa !10
  %791 = add nuw nsw i64 %771, 4
  %792 = icmp eq i64 %791, 1200
  br i1 %792, label %793, label %770, !llvm.loop !11

793:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr3()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 3)
  br label %811

794:
  %795 = add nuw nsw i64 %812, 1
  %796 = getelementptr inbounds nuw i8, ptr @dst, i64 %795
  %797 = load i8, ptr %796, align 1, !tbaa !10
  %798 = getelementptr inbounds nuw i8, ptr @ref, i64 %795
  %799 = load i8, ptr %798, align 1, !tbaa !10
  %800 = icmp eq i8 %797, %799
  br i1 %800, label %801, label %818

801:
  %802 = add nuw nsw i64 %812, 2
  %803 = getelementptr inbounds nuw i8, ptr @dst, i64 %802
  %804 = load i8, ptr %803, align 1, !tbaa !10
  %805 = getelementptr inbounds nuw i8, ptr @ref, i64 %802
  %806 = load i8, ptr %805, align 1, !tbaa !10
  %807 = icmp eq i8 %804, %806
  br i1 %807, label %808, label %818

808:
  %809 = add nuw nsw i64 %812, 3
  %810 = icmp eq i64 %809, 1200
  br i1 %810, label %819, label %811, !llvm.loop !144

811:
  %812 = phi i64 [ 0, %793 ], [ %809, %808 ]
  %813 = getelementptr inbounds nuw i8, ptr @dst, i64 %812
  %814 = load i8, ptr %813, align 1, !tbaa !10
  %815 = getelementptr inbounds nuw i8, ptr @ref, i64 %812
  %816 = load i8, ptr %815, align 1, !tbaa !10
  %817 = icmp eq i8 %814, %816
  br i1 %817, label %794, label %818

818:
  tail call void @report(i64 noundef 3)
  store i1 true, ptr @bad, align 4
  br label %820

819:
  br label %820

820:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %821

821:
  %822 = phi i64 [ 0, %820 ], [ %842, %821 ]
  %823 = trunc i64 %822 to i8
  %824 = mul i8 %823, 7
  %825 = or disjoint i8 %824, 3
  %826 = getelementptr inbounds nuw i8, ptr @src, i64 %822
  store i8 %825, ptr %826, align 4, !tbaa !10
  %827 = or disjoint i64 %822, 1
  %828 = trunc i64 %827 to i8
  %829 = mul i8 %828, 7
  %830 = add i8 %829, 3
  %831 = getelementptr inbounds nuw i8, ptr @src, i64 %827
  store i8 %830, ptr %831, align 1, !tbaa !10
  %832 = or disjoint i64 %822, 2
  %833 = trunc i64 %832 to i8
  %834 = mul i8 %833, 7
  %835 = add i8 %834, 3
  %836 = getelementptr inbounds nuw i8, ptr @src, i64 %832
  store i8 %835, ptr %836, align 2, !tbaa !10
  %837 = or disjoint i64 %822, 3
  %838 = trunc i64 %837 to i8
  %839 = mul i8 %838, 7
  %840 = add i8 %839, 3
  %841 = getelementptr inbounds nuw i8, ptr @src, i64 %837
  store i8 %840, ptr %841, align 1, !tbaa !10
  %842 = add nuw nsw i64 %822, 4
  %843 = icmp eq i64 %842, 1200
  br i1 %843, label %844, label %821, !llvm.loop !11

844:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv3(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 3)
  br label %862

845:
  %846 = add nuw nsw i64 %863, 1
  %847 = getelementptr inbounds nuw i8, ptr @dst, i64 %846
  %848 = load i8, ptr %847, align 1, !tbaa !10
  %849 = getelementptr inbounds nuw i8, ptr @ref, i64 %846
  %850 = load i8, ptr %849, align 1, !tbaa !10
  %851 = icmp eq i8 %848, %850
  br i1 %851, label %852, label %869

852:
  %853 = add nuw nsw i64 %863, 2
  %854 = getelementptr inbounds nuw i8, ptr @dst, i64 %853
  %855 = load i8, ptr %854, align 1, !tbaa !10
  %856 = getelementptr inbounds nuw i8, ptr @ref, i64 %853
  %857 = load i8, ptr %856, align 1, !tbaa !10
  %858 = icmp eq i8 %855, %857
  br i1 %858, label %859, label %869

859:
  %860 = add nuw nsw i64 %863, 3
  %861 = icmp eq i64 %860, 1200
  br i1 %861, label %870, label %862, !llvm.loop !144

862:
  %863 = phi i64 [ 0, %844 ], [ %860, %859 ]
  %864 = getelementptr inbounds nuw i8, ptr @dst, i64 %863
  %865 = load i8, ptr %864, align 1, !tbaa !10
  %866 = getelementptr inbounds nuw i8, ptr @ref, i64 %863
  %867 = load i8, ptr %866, align 1, !tbaa !10
  %868 = icmp eq i8 %865, %867
  br i1 %868, label %845, label %869

869:
  tail call void @report(i64 noundef 3)
  store i1 true, ptr @bad, align 4
  br label %871

870:
  br label %871

871:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %872

872:
  %873 = phi i64 [ 0, %871 ], [ %893, %872 ]
  %874 = trunc i64 %873 to i8
  %875 = mul i8 %874, 7
  %876 = or disjoint i8 %875, 3
  %877 = getelementptr inbounds nuw i8, ptr @src, i64 %873
  store i8 %876, ptr %877, align 4, !tbaa !10
  %878 = or disjoint i64 %873, 1
  %879 = trunc i64 %878 to i8
  %880 = mul i8 %879, 7
  %881 = add i8 %880, 3
  %882 = getelementptr inbounds nuw i8, ptr @src, i64 %878
  store i8 %881, ptr %882, align 1, !tbaa !10
  %883 = or disjoint i64 %873, 2
  %884 = trunc i64 %883 to i8
  %885 = mul i8 %884, 7
  %886 = add i8 %885, 3
  %887 = getelementptr inbounds nuw i8, ptr @src, i64 %883
  store i8 %886, ptr %887, align 2, !tbaa !10
  %888 = or disjoint i64 %873, 3
  %889 = trunc i64 %888 to i8
  %890 = mul i8 %889, 7
  %891 = add i8 %890, 3
  %892 = getelementptr inbounds nuw i8, ptr @src, i64 %888
  store i8 %891, ptr %892, align 1, !tbaa !10
  %893 = add nuw nsw i64 %873, 4
  %894 = icmp eq i64 %893, 1200
  br i1 %894, label %895, label %872, !llvm.loop !11

895:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv3(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 3)
  br label %913

896:
  %897 = add nuw nsw i64 %914, 1
  %898 = getelementptr inbounds nuw i8, ptr @dst, i64 %897
  %899 = load i8, ptr %898, align 1, !tbaa !10
  %900 = getelementptr inbounds nuw i8, ptr @ref, i64 %897
  %901 = load i8, ptr %900, align 1, !tbaa !10
  %902 = icmp eq i8 %899, %901
  br i1 %902, label %903, label %920

903:
  %904 = add nuw nsw i64 %914, 2
  %905 = getelementptr inbounds nuw i8, ptr @dst, i64 %904
  %906 = load i8, ptr %905, align 1, !tbaa !10
  %907 = getelementptr inbounds nuw i8, ptr @ref, i64 %904
  %908 = load i8, ptr %907, align 1, !tbaa !10
  %909 = icmp eq i8 %906, %908
  br i1 %909, label %910, label %920

910:
  %911 = add nuw nsw i64 %914, 3
  %912 = icmp eq i64 %911, 1200
  br i1 %912, label %921, label %913, !llvm.loop !144

913:
  %914 = phi i64 [ 0, %895 ], [ %911, %910 ]
  %915 = getelementptr inbounds nuw i8, ptr @dst, i64 %914
  %916 = load i8, ptr %915, align 1, !tbaa !10
  %917 = getelementptr inbounds nuw i8, ptr @ref, i64 %914
  %918 = load i8, ptr %917, align 1, !tbaa !10
  %919 = icmp eq i8 %916, %918
  br i1 %919, label %896, label %920

920:
  tail call void @report(i64 noundef 3)
  store i1 true, ptr @bad, align 4
  br label %922

921:
  br label %922

922:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %923

923:
  %924 = phi i64 [ 0, %922 ], [ %944, %923 ]
  %925 = trunc i64 %924 to i8
  %926 = mul i8 %925, 7
  %927 = or disjoint i8 %926, 3
  %928 = getelementptr inbounds nuw i8, ptr @src, i64 %924
  store i8 %927, ptr %928, align 4, !tbaa !10
  %929 = or disjoint i64 %924, 1
  %930 = trunc i64 %929 to i8
  %931 = mul i8 %930, 7
  %932 = add i8 %931, 3
  %933 = getelementptr inbounds nuw i8, ptr @src, i64 %929
  store i8 %932, ptr %933, align 1, !tbaa !10
  %934 = or disjoint i64 %924, 2
  %935 = trunc i64 %934 to i8
  %936 = mul i8 %935, 7
  %937 = add i8 %936, 3
  %938 = getelementptr inbounds nuw i8, ptr @src, i64 %934
  store i8 %937, ptr %938, align 2, !tbaa !10
  %939 = or disjoint i64 %924, 3
  %940 = trunc i64 %939 to i8
  %941 = mul i8 %940, 7
  %942 = add i8 %941, 3
  %943 = getelementptr inbounds nuw i8, ptr @src, i64 %939
  store i8 %942, ptr %943, align 1, !tbaa !10
  %944 = add nuw nsw i64 %924, 4
  %945 = icmp eq i64 %944, 1200
  br i1 %945, label %946, label %923, !llvm.loop !11

946:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp5()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 5)
  br label %964

947:
  %948 = add nuw nsw i64 %965, 1
  %949 = getelementptr inbounds nuw i8, ptr @dst, i64 %948
  %950 = load i8, ptr %949, align 1, !tbaa !10
  %951 = getelementptr inbounds nuw i8, ptr @ref, i64 %948
  %952 = load i8, ptr %951, align 1, !tbaa !10
  %953 = icmp eq i8 %950, %952
  br i1 %953, label %954, label %971

954:
  %955 = add nuw nsw i64 %965, 2
  %956 = getelementptr inbounds nuw i8, ptr @dst, i64 %955
  %957 = load i8, ptr %956, align 1, !tbaa !10
  %958 = getelementptr inbounds nuw i8, ptr @ref, i64 %955
  %959 = load i8, ptr %958, align 1, !tbaa !10
  %960 = icmp eq i8 %957, %959
  br i1 %960, label %961, label %971

961:
  %962 = add nuw nsw i64 %965, 3
  %963 = icmp eq i64 %962, 1200
  br i1 %963, label %972, label %964, !llvm.loop !144

964:
  %965 = phi i64 [ 0, %946 ], [ %962, %961 ]
  %966 = getelementptr inbounds nuw i8, ptr @dst, i64 %965
  %967 = load i8, ptr %966, align 1, !tbaa !10
  %968 = getelementptr inbounds nuw i8, ptr @ref, i64 %965
  %969 = load i8, ptr %968, align 1, !tbaa !10
  %970 = icmp eq i8 %967, %969
  br i1 %970, label %947, label %971

971:
  tail call void @report(i64 noundef 5)
  store i1 true, ptr @bad, align 4
  br label %973

972:
  br label %973

973:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %974

974:
  %975 = phi i64 [ 0, %973 ], [ %995, %974 ]
  %976 = trunc i64 %975 to i8
  %977 = mul i8 %976, 7
  %978 = or disjoint i8 %977, 3
  %979 = getelementptr inbounds nuw i8, ptr @src, i64 %975
  store i8 %978, ptr %979, align 4, !tbaa !10
  %980 = or disjoint i64 %975, 1
  %981 = trunc i64 %980 to i8
  %982 = mul i8 %981, 7
  %983 = add i8 %982, 3
  %984 = getelementptr inbounds nuw i8, ptr @src, i64 %980
  store i8 %983, ptr %984, align 1, !tbaa !10
  %985 = or disjoint i64 %975, 2
  %986 = trunc i64 %985 to i8
  %987 = mul i8 %986, 7
  %988 = add i8 %987, 3
  %989 = getelementptr inbounds nuw i8, ptr @src, i64 %985
  store i8 %988, ptr %989, align 2, !tbaa !10
  %990 = or disjoint i64 %975, 3
  %991 = trunc i64 %990 to i8
  %992 = mul i8 %991, 7
  %993 = add i8 %992, 3
  %994 = getelementptr inbounds nuw i8, ptr @src, i64 %990
  store i8 %993, ptr %994, align 1, !tbaa !10
  %995 = add nuw nsw i64 %975, 4
  %996 = icmp eq i64 %995, 1200
  br i1 %996, label %997, label %974, !llvm.loop !11

997:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr5()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 5)
  br label %1015

998:
  %999 = add nuw nsw i64 %1016, 1
  %1000 = getelementptr inbounds nuw i8, ptr @dst, i64 %999
  %1001 = load i8, ptr %1000, align 1, !tbaa !10
  %1002 = getelementptr inbounds nuw i8, ptr @ref, i64 %999
  %1003 = load i8, ptr %1002, align 1, !tbaa !10
  %1004 = icmp eq i8 %1001, %1003
  br i1 %1004, label %1005, label %1022

1005:
  %1006 = add nuw nsw i64 %1016, 2
  %1007 = getelementptr inbounds nuw i8, ptr @dst, i64 %1006
  %1008 = load i8, ptr %1007, align 1, !tbaa !10
  %1009 = getelementptr inbounds nuw i8, ptr @ref, i64 %1006
  %1010 = load i8, ptr %1009, align 1, !tbaa !10
  %1011 = icmp eq i8 %1008, %1010
  br i1 %1011, label %1012, label %1022

1012:
  %1013 = add nuw nsw i64 %1016, 3
  %1014 = icmp eq i64 %1013, 1200
  br i1 %1014, label %1023, label %1015, !llvm.loop !144

1015:
  %1016 = phi i64 [ 0, %997 ], [ %1013, %1012 ]
  %1017 = getelementptr inbounds nuw i8, ptr @dst, i64 %1016
  %1018 = load i8, ptr %1017, align 1, !tbaa !10
  %1019 = getelementptr inbounds nuw i8, ptr @ref, i64 %1016
  %1020 = load i8, ptr %1019, align 1, !tbaa !10
  %1021 = icmp eq i8 %1018, %1020
  br i1 %1021, label %998, label %1022

1022:
  tail call void @report(i64 noundef 5)
  store i1 true, ptr @bad, align 4
  br label %1024

1023:
  br label %1024

1024:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1025

1025:
  %1026 = phi i64 [ 0, %1024 ], [ %1046, %1025 ]
  %1027 = trunc i64 %1026 to i8
  %1028 = mul i8 %1027, 7
  %1029 = or disjoint i8 %1028, 3
  %1030 = getelementptr inbounds nuw i8, ptr @src, i64 %1026
  store i8 %1029, ptr %1030, align 4, !tbaa !10
  %1031 = or disjoint i64 %1026, 1
  %1032 = trunc i64 %1031 to i8
  %1033 = mul i8 %1032, 7
  %1034 = add i8 %1033, 3
  %1035 = getelementptr inbounds nuw i8, ptr @src, i64 %1031
  store i8 %1034, ptr %1035, align 1, !tbaa !10
  %1036 = or disjoint i64 %1026, 2
  %1037 = trunc i64 %1036 to i8
  %1038 = mul i8 %1037, 7
  %1039 = add i8 %1038, 3
  %1040 = getelementptr inbounds nuw i8, ptr @src, i64 %1036
  store i8 %1039, ptr %1040, align 2, !tbaa !10
  %1041 = or disjoint i64 %1026, 3
  %1042 = trunc i64 %1041 to i8
  %1043 = mul i8 %1042, 7
  %1044 = add i8 %1043, 3
  %1045 = getelementptr inbounds nuw i8, ptr @src, i64 %1041
  store i8 %1044, ptr %1045, align 1, !tbaa !10
  %1046 = add nuw nsw i64 %1026, 4
  %1047 = icmp eq i64 %1046, 1200
  br i1 %1047, label %1048, label %1025, !llvm.loop !11

1048:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c75()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 5)
  br label %1066

1049:
  %1050 = add nuw nsw i64 %1067, 1
  %1051 = getelementptr inbounds nuw i8, ptr @dst, i64 %1050
  %1052 = load i8, ptr %1051, align 1, !tbaa !10
  %1053 = getelementptr inbounds nuw i8, ptr @ref, i64 %1050
  %1054 = load i8, ptr %1053, align 1, !tbaa !10
  %1055 = icmp eq i8 %1052, %1054
  br i1 %1055, label %1056, label %1073

1056:
  %1057 = add nuw nsw i64 %1067, 2
  %1058 = getelementptr inbounds nuw i8, ptr @dst, i64 %1057
  %1059 = load i8, ptr %1058, align 1, !tbaa !10
  %1060 = getelementptr inbounds nuw i8, ptr @ref, i64 %1057
  %1061 = load i8, ptr %1060, align 1, !tbaa !10
  %1062 = icmp eq i8 %1059, %1061
  br i1 %1062, label %1063, label %1073

1063:
  %1064 = add nuw nsw i64 %1067, 3
  %1065 = icmp eq i64 %1064, 1200
  br i1 %1065, label %1074, label %1066, !llvm.loop !144

1066:
  %1067 = phi i64 [ 0, %1048 ], [ %1064, %1063 ]
  %1068 = getelementptr inbounds nuw i8, ptr @dst, i64 %1067
  %1069 = load i8, ptr %1068, align 1, !tbaa !10
  %1070 = getelementptr inbounds nuw i8, ptr @ref, i64 %1067
  %1071 = load i8, ptr %1070, align 1, !tbaa !10
  %1072 = icmp eq i8 %1069, %1071
  br i1 %1072, label %1049, label %1073

1073:
  tail call void @report(i64 noundef 5)
  store i1 true, ptr @bad, align 4
  br label %1075

1074:
  br label %1075

1075:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1076

1076:
  %1077 = phi i64 [ 0, %1075 ], [ %1097, %1076 ]
  %1078 = trunc i64 %1077 to i8
  %1079 = mul i8 %1078, 7
  %1080 = or disjoint i8 %1079, 3
  %1081 = getelementptr inbounds nuw i8, ptr @src, i64 %1077
  store i8 %1080, ptr %1081, align 4, !tbaa !10
  %1082 = or disjoint i64 %1077, 1
  %1083 = trunc i64 %1082 to i8
  %1084 = mul i8 %1083, 7
  %1085 = add i8 %1084, 3
  %1086 = getelementptr inbounds nuw i8, ptr @src, i64 %1082
  store i8 %1085, ptr %1086, align 1, !tbaa !10
  %1087 = or disjoint i64 %1077, 2
  %1088 = trunc i64 %1087 to i8
  %1089 = mul i8 %1088, 7
  %1090 = add i8 %1089, 3
  %1091 = getelementptr inbounds nuw i8, ptr @src, i64 %1087
  store i8 %1090, ptr %1091, align 2, !tbaa !10
  %1092 = or disjoint i64 %1077, 3
  %1093 = trunc i64 %1092 to i8
  %1094 = mul i8 %1093, 7
  %1095 = add i8 %1094, 3
  %1096 = getelementptr inbounds nuw i8, ptr @src, i64 %1092
  store i8 %1095, ptr %1096, align 1, !tbaa !10
  %1097 = add nuw nsw i64 %1077, 4
  %1098 = icmp eq i64 %1097, 1200
  br i1 %1098, label %1099, label %1076, !llvm.loop !11

1099:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr5()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 5)
  br label %1117

1100:
  %1101 = add nuw nsw i64 %1118, 1
  %1102 = getelementptr inbounds nuw i8, ptr @dst, i64 %1101
  %1103 = load i8, ptr %1102, align 1, !tbaa !10
  %1104 = getelementptr inbounds nuw i8, ptr @ref, i64 %1101
  %1105 = load i8, ptr %1104, align 1, !tbaa !10
  %1106 = icmp eq i8 %1103, %1105
  br i1 %1106, label %1107, label %1124

1107:
  %1108 = add nuw nsw i64 %1118, 2
  %1109 = getelementptr inbounds nuw i8, ptr @dst, i64 %1108
  %1110 = load i8, ptr %1109, align 1, !tbaa !10
  %1111 = getelementptr inbounds nuw i8, ptr @ref, i64 %1108
  %1112 = load i8, ptr %1111, align 1, !tbaa !10
  %1113 = icmp eq i8 %1110, %1112
  br i1 %1113, label %1114, label %1124

1114:
  %1115 = add nuw nsw i64 %1118, 3
  %1116 = icmp eq i64 %1115, 1200
  br i1 %1116, label %1125, label %1117, !llvm.loop !144

1117:
  %1118 = phi i64 [ 0, %1099 ], [ %1115, %1114 ]
  %1119 = getelementptr inbounds nuw i8, ptr @dst, i64 %1118
  %1120 = load i8, ptr %1119, align 1, !tbaa !10
  %1121 = getelementptr inbounds nuw i8, ptr @ref, i64 %1118
  %1122 = load i8, ptr %1121, align 1, !tbaa !10
  %1123 = icmp eq i8 %1120, %1122
  br i1 %1123, label %1100, label %1124

1124:
  tail call void @report(i64 noundef 5)
  store i1 true, ptr @bad, align 4
  br label %1126

1125:
  br label %1126

1126:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1127

1127:
  %1128 = phi i64 [ 0, %1126 ], [ %1148, %1127 ]
  %1129 = trunc i64 %1128 to i8
  %1130 = mul i8 %1129, 7
  %1131 = or disjoint i8 %1130, 3
  %1132 = getelementptr inbounds nuw i8, ptr @src, i64 %1128
  store i8 %1131, ptr %1132, align 4, !tbaa !10
  %1133 = or disjoint i64 %1128, 1
  %1134 = trunc i64 %1133 to i8
  %1135 = mul i8 %1134, 7
  %1136 = add i8 %1135, 3
  %1137 = getelementptr inbounds nuw i8, ptr @src, i64 %1133
  store i8 %1136, ptr %1137, align 1, !tbaa !10
  %1138 = or disjoint i64 %1128, 2
  %1139 = trunc i64 %1138 to i8
  %1140 = mul i8 %1139, 7
  %1141 = add i8 %1140, 3
  %1142 = getelementptr inbounds nuw i8, ptr @src, i64 %1138
  store i8 %1141, ptr %1142, align 2, !tbaa !10
  %1143 = or disjoint i64 %1128, 3
  %1144 = trunc i64 %1143 to i8
  %1145 = mul i8 %1144, 7
  %1146 = add i8 %1145, 3
  %1147 = getelementptr inbounds nuw i8, ptr @src, i64 %1143
  store i8 %1146, ptr %1147, align 1, !tbaa !10
  %1148 = add nuw nsw i64 %1128, 4
  %1149 = icmp eq i64 %1148, 1200
  br i1 %1149, label %1150, label %1127, !llvm.loop !11

1150:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv5(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 5)
  br label %1168

1151:
  %1152 = add nuw nsw i64 %1169, 1
  %1153 = getelementptr inbounds nuw i8, ptr @dst, i64 %1152
  %1154 = load i8, ptr %1153, align 1, !tbaa !10
  %1155 = getelementptr inbounds nuw i8, ptr @ref, i64 %1152
  %1156 = load i8, ptr %1155, align 1, !tbaa !10
  %1157 = icmp eq i8 %1154, %1156
  br i1 %1157, label %1158, label %1175

1158:
  %1159 = add nuw nsw i64 %1169, 2
  %1160 = getelementptr inbounds nuw i8, ptr @dst, i64 %1159
  %1161 = load i8, ptr %1160, align 1, !tbaa !10
  %1162 = getelementptr inbounds nuw i8, ptr @ref, i64 %1159
  %1163 = load i8, ptr %1162, align 1, !tbaa !10
  %1164 = icmp eq i8 %1161, %1163
  br i1 %1164, label %1165, label %1175

1165:
  %1166 = add nuw nsw i64 %1169, 3
  %1167 = icmp eq i64 %1166, 1200
  br i1 %1167, label %1176, label %1168, !llvm.loop !144

1168:
  %1169 = phi i64 [ 0, %1150 ], [ %1166, %1165 ]
  %1170 = getelementptr inbounds nuw i8, ptr @dst, i64 %1169
  %1171 = load i8, ptr %1170, align 1, !tbaa !10
  %1172 = getelementptr inbounds nuw i8, ptr @ref, i64 %1169
  %1173 = load i8, ptr %1172, align 1, !tbaa !10
  %1174 = icmp eq i8 %1171, %1173
  br i1 %1174, label %1151, label %1175

1175:
  tail call void @report(i64 noundef 5)
  store i1 true, ptr @bad, align 4
  br label %1177

1176:
  br label %1177

1177:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1178

1178:
  %1179 = phi i64 [ 0, %1177 ], [ %1199, %1178 ]
  %1180 = trunc i64 %1179 to i8
  %1181 = mul i8 %1180, 7
  %1182 = or disjoint i8 %1181, 3
  %1183 = getelementptr inbounds nuw i8, ptr @src, i64 %1179
  store i8 %1182, ptr %1183, align 4, !tbaa !10
  %1184 = or disjoint i64 %1179, 1
  %1185 = trunc i64 %1184 to i8
  %1186 = mul i8 %1185, 7
  %1187 = add i8 %1186, 3
  %1188 = getelementptr inbounds nuw i8, ptr @src, i64 %1184
  store i8 %1187, ptr %1188, align 1, !tbaa !10
  %1189 = or disjoint i64 %1179, 2
  %1190 = trunc i64 %1189 to i8
  %1191 = mul i8 %1190, 7
  %1192 = add i8 %1191, 3
  %1193 = getelementptr inbounds nuw i8, ptr @src, i64 %1189
  store i8 %1192, ptr %1193, align 2, !tbaa !10
  %1194 = or disjoint i64 %1179, 3
  %1195 = trunc i64 %1194 to i8
  %1196 = mul i8 %1195, 7
  %1197 = add i8 %1196, 3
  %1198 = getelementptr inbounds nuw i8, ptr @src, i64 %1194
  store i8 %1197, ptr %1198, align 1, !tbaa !10
  %1199 = add nuw nsw i64 %1179, 4
  %1200 = icmp eq i64 %1199, 1200
  br i1 %1200, label %1201, label %1178, !llvm.loop !11

1201:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv5(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 5)
  br label %1219

1202:
  %1203 = add nuw nsw i64 %1220, 1
  %1204 = getelementptr inbounds nuw i8, ptr @dst, i64 %1203
  %1205 = load i8, ptr %1204, align 1, !tbaa !10
  %1206 = getelementptr inbounds nuw i8, ptr @ref, i64 %1203
  %1207 = load i8, ptr %1206, align 1, !tbaa !10
  %1208 = icmp eq i8 %1205, %1207
  br i1 %1208, label %1209, label %1226

1209:
  %1210 = add nuw nsw i64 %1220, 2
  %1211 = getelementptr inbounds nuw i8, ptr @dst, i64 %1210
  %1212 = load i8, ptr %1211, align 1, !tbaa !10
  %1213 = getelementptr inbounds nuw i8, ptr @ref, i64 %1210
  %1214 = load i8, ptr %1213, align 1, !tbaa !10
  %1215 = icmp eq i8 %1212, %1214
  br i1 %1215, label %1216, label %1226

1216:
  %1217 = add nuw nsw i64 %1220, 3
  %1218 = icmp eq i64 %1217, 1200
  br i1 %1218, label %1227, label %1219, !llvm.loop !144

1219:
  %1220 = phi i64 [ 0, %1201 ], [ %1217, %1216 ]
  %1221 = getelementptr inbounds nuw i8, ptr @dst, i64 %1220
  %1222 = load i8, ptr %1221, align 1, !tbaa !10
  %1223 = getelementptr inbounds nuw i8, ptr @ref, i64 %1220
  %1224 = load i8, ptr %1223, align 1, !tbaa !10
  %1225 = icmp eq i8 %1222, %1224
  br i1 %1225, label %1202, label %1226

1226:
  tail call void @report(i64 noundef 5)
  store i1 true, ptr @bad, align 4
  br label %1228

1227:
  br label %1228

1228:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1229

1229:
  %1230 = phi i64 [ 0, %1228 ], [ %1250, %1229 ]
  %1231 = trunc i64 %1230 to i8
  %1232 = mul i8 %1231, 7
  %1233 = or disjoint i8 %1232, 3
  %1234 = getelementptr inbounds nuw i8, ptr @src, i64 %1230
  store i8 %1233, ptr %1234, align 4, !tbaa !10
  %1235 = or disjoint i64 %1230, 1
  %1236 = trunc i64 %1235 to i8
  %1237 = mul i8 %1236, 7
  %1238 = add i8 %1237, 3
  %1239 = getelementptr inbounds nuw i8, ptr @src, i64 %1235
  store i8 %1238, ptr %1239, align 1, !tbaa !10
  %1240 = or disjoint i64 %1230, 2
  %1241 = trunc i64 %1240 to i8
  %1242 = mul i8 %1241, 7
  %1243 = add i8 %1242, 3
  %1244 = getelementptr inbounds nuw i8, ptr @src, i64 %1240
  store i8 %1243, ptr %1244, align 2, !tbaa !10
  %1245 = or disjoint i64 %1230, 3
  %1246 = trunc i64 %1245 to i8
  %1247 = mul i8 %1246, 7
  %1248 = add i8 %1247, 3
  %1249 = getelementptr inbounds nuw i8, ptr @src, i64 %1245
  store i8 %1248, ptr %1249, align 1, !tbaa !10
  %1250 = add nuw nsw i64 %1230, 4
  %1251 = icmp eq i64 %1250, 1200
  br i1 %1251, label %1252, label %1229, !llvm.loop !11

1252:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp7()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 7)
  br label %1270

1253:
  %1254 = add nuw nsw i64 %1271, 1
  %1255 = getelementptr inbounds nuw i8, ptr @dst, i64 %1254
  %1256 = load i8, ptr %1255, align 1, !tbaa !10
  %1257 = getelementptr inbounds nuw i8, ptr @ref, i64 %1254
  %1258 = load i8, ptr %1257, align 1, !tbaa !10
  %1259 = icmp eq i8 %1256, %1258
  br i1 %1259, label %1260, label %1277

1260:
  %1261 = add nuw nsw i64 %1271, 2
  %1262 = getelementptr inbounds nuw i8, ptr @dst, i64 %1261
  %1263 = load i8, ptr %1262, align 1, !tbaa !10
  %1264 = getelementptr inbounds nuw i8, ptr @ref, i64 %1261
  %1265 = load i8, ptr %1264, align 1, !tbaa !10
  %1266 = icmp eq i8 %1263, %1265
  br i1 %1266, label %1267, label %1277

1267:
  %1268 = add nuw nsw i64 %1271, 3
  %1269 = icmp eq i64 %1268, 1200
  br i1 %1269, label %1278, label %1270, !llvm.loop !144

1270:
  %1271 = phi i64 [ 0, %1252 ], [ %1268, %1267 ]
  %1272 = getelementptr inbounds nuw i8, ptr @dst, i64 %1271
  %1273 = load i8, ptr %1272, align 1, !tbaa !10
  %1274 = getelementptr inbounds nuw i8, ptr @ref, i64 %1271
  %1275 = load i8, ptr %1274, align 1, !tbaa !10
  %1276 = icmp eq i8 %1273, %1275
  br i1 %1276, label %1253, label %1277

1277:
  tail call void @report(i64 noundef 7)
  store i1 true, ptr @bad, align 4
  br label %1279

1278:
  br label %1279

1279:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1280

1280:
  %1281 = phi i64 [ 0, %1279 ], [ %1301, %1280 ]
  %1282 = trunc i64 %1281 to i8
  %1283 = mul i8 %1282, 7
  %1284 = or disjoint i8 %1283, 3
  %1285 = getelementptr inbounds nuw i8, ptr @src, i64 %1281
  store i8 %1284, ptr %1285, align 4, !tbaa !10
  %1286 = or disjoint i64 %1281, 1
  %1287 = trunc i64 %1286 to i8
  %1288 = mul i8 %1287, 7
  %1289 = add i8 %1288, 3
  %1290 = getelementptr inbounds nuw i8, ptr @src, i64 %1286
  store i8 %1289, ptr %1290, align 1, !tbaa !10
  %1291 = or disjoint i64 %1281, 2
  %1292 = trunc i64 %1291 to i8
  %1293 = mul i8 %1292, 7
  %1294 = add i8 %1293, 3
  %1295 = getelementptr inbounds nuw i8, ptr @src, i64 %1291
  store i8 %1294, ptr %1295, align 2, !tbaa !10
  %1296 = or disjoint i64 %1281, 3
  %1297 = trunc i64 %1296 to i8
  %1298 = mul i8 %1297, 7
  %1299 = add i8 %1298, 3
  %1300 = getelementptr inbounds nuw i8, ptr @src, i64 %1296
  store i8 %1299, ptr %1300, align 1, !tbaa !10
  %1301 = add nuw nsw i64 %1281, 4
  %1302 = icmp eq i64 %1301, 1200
  br i1 %1302, label %1303, label %1280, !llvm.loop !11

1303:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr7()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 7)
  br label %1321

1304:
  %1305 = add nuw nsw i64 %1322, 1
  %1306 = getelementptr inbounds nuw i8, ptr @dst, i64 %1305
  %1307 = load i8, ptr %1306, align 1, !tbaa !10
  %1308 = getelementptr inbounds nuw i8, ptr @ref, i64 %1305
  %1309 = load i8, ptr %1308, align 1, !tbaa !10
  %1310 = icmp eq i8 %1307, %1309
  br i1 %1310, label %1311, label %1328

1311:
  %1312 = add nuw nsw i64 %1322, 2
  %1313 = getelementptr inbounds nuw i8, ptr @dst, i64 %1312
  %1314 = load i8, ptr %1313, align 1, !tbaa !10
  %1315 = getelementptr inbounds nuw i8, ptr @ref, i64 %1312
  %1316 = load i8, ptr %1315, align 1, !tbaa !10
  %1317 = icmp eq i8 %1314, %1316
  br i1 %1317, label %1318, label %1328

1318:
  %1319 = add nuw nsw i64 %1322, 3
  %1320 = icmp eq i64 %1319, 1200
  br i1 %1320, label %1329, label %1321, !llvm.loop !144

1321:
  %1322 = phi i64 [ 0, %1303 ], [ %1319, %1318 ]
  %1323 = getelementptr inbounds nuw i8, ptr @dst, i64 %1322
  %1324 = load i8, ptr %1323, align 1, !tbaa !10
  %1325 = getelementptr inbounds nuw i8, ptr @ref, i64 %1322
  %1326 = load i8, ptr %1325, align 1, !tbaa !10
  %1327 = icmp eq i8 %1324, %1326
  br i1 %1327, label %1304, label %1328

1328:
  tail call void @report(i64 noundef 7)
  store i1 true, ptr @bad, align 4
  br label %1330

1329:
  br label %1330

1330:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1331

1331:
  %1332 = phi i64 [ 0, %1330 ], [ %1352, %1331 ]
  %1333 = trunc i64 %1332 to i8
  %1334 = mul i8 %1333, 7
  %1335 = or disjoint i8 %1334, 3
  %1336 = getelementptr inbounds nuw i8, ptr @src, i64 %1332
  store i8 %1335, ptr %1336, align 4, !tbaa !10
  %1337 = or disjoint i64 %1332, 1
  %1338 = trunc i64 %1337 to i8
  %1339 = mul i8 %1338, 7
  %1340 = add i8 %1339, 3
  %1341 = getelementptr inbounds nuw i8, ptr @src, i64 %1337
  store i8 %1340, ptr %1341, align 1, !tbaa !10
  %1342 = or disjoint i64 %1332, 2
  %1343 = trunc i64 %1342 to i8
  %1344 = mul i8 %1343, 7
  %1345 = add i8 %1344, 3
  %1346 = getelementptr inbounds nuw i8, ptr @src, i64 %1342
  store i8 %1345, ptr %1346, align 2, !tbaa !10
  %1347 = or disjoint i64 %1332, 3
  %1348 = trunc i64 %1347 to i8
  %1349 = mul i8 %1348, 7
  %1350 = add i8 %1349, 3
  %1351 = getelementptr inbounds nuw i8, ptr @src, i64 %1347
  store i8 %1350, ptr %1351, align 1, !tbaa !10
  %1352 = add nuw nsw i64 %1332, 4
  %1353 = icmp eq i64 %1352, 1200
  br i1 %1353, label %1354, label %1331, !llvm.loop !11

1354:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c77()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 7)
  br label %1372

1355:
  %1356 = add nuw nsw i64 %1373, 1
  %1357 = getelementptr inbounds nuw i8, ptr @dst, i64 %1356
  %1358 = load i8, ptr %1357, align 1, !tbaa !10
  %1359 = getelementptr inbounds nuw i8, ptr @ref, i64 %1356
  %1360 = load i8, ptr %1359, align 1, !tbaa !10
  %1361 = icmp eq i8 %1358, %1360
  br i1 %1361, label %1362, label %1379

1362:
  %1363 = add nuw nsw i64 %1373, 2
  %1364 = getelementptr inbounds nuw i8, ptr @dst, i64 %1363
  %1365 = load i8, ptr %1364, align 1, !tbaa !10
  %1366 = getelementptr inbounds nuw i8, ptr @ref, i64 %1363
  %1367 = load i8, ptr %1366, align 1, !tbaa !10
  %1368 = icmp eq i8 %1365, %1367
  br i1 %1368, label %1369, label %1379

1369:
  %1370 = add nuw nsw i64 %1373, 3
  %1371 = icmp eq i64 %1370, 1200
  br i1 %1371, label %1380, label %1372, !llvm.loop !144

1372:
  %1373 = phi i64 [ 0, %1354 ], [ %1370, %1369 ]
  %1374 = getelementptr inbounds nuw i8, ptr @dst, i64 %1373
  %1375 = load i8, ptr %1374, align 1, !tbaa !10
  %1376 = getelementptr inbounds nuw i8, ptr @ref, i64 %1373
  %1377 = load i8, ptr %1376, align 1, !tbaa !10
  %1378 = icmp eq i8 %1375, %1377
  br i1 %1378, label %1355, label %1379

1379:
  tail call void @report(i64 noundef 7)
  store i1 true, ptr @bad, align 4
  br label %1381

1380:
  br label %1381

1381:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1382

1382:
  %1383 = phi i64 [ 0, %1381 ], [ %1403, %1382 ]
  %1384 = trunc i64 %1383 to i8
  %1385 = mul i8 %1384, 7
  %1386 = or disjoint i8 %1385, 3
  %1387 = getelementptr inbounds nuw i8, ptr @src, i64 %1383
  store i8 %1386, ptr %1387, align 4, !tbaa !10
  %1388 = or disjoint i64 %1383, 1
  %1389 = trunc i64 %1388 to i8
  %1390 = mul i8 %1389, 7
  %1391 = add i8 %1390, 3
  %1392 = getelementptr inbounds nuw i8, ptr @src, i64 %1388
  store i8 %1391, ptr %1392, align 1, !tbaa !10
  %1393 = or disjoint i64 %1383, 2
  %1394 = trunc i64 %1393 to i8
  %1395 = mul i8 %1394, 7
  %1396 = add i8 %1395, 3
  %1397 = getelementptr inbounds nuw i8, ptr @src, i64 %1393
  store i8 %1396, ptr %1397, align 2, !tbaa !10
  %1398 = or disjoint i64 %1383, 3
  %1399 = trunc i64 %1398 to i8
  %1400 = mul i8 %1399, 7
  %1401 = add i8 %1400, 3
  %1402 = getelementptr inbounds nuw i8, ptr @src, i64 %1398
  store i8 %1401, ptr %1402, align 1, !tbaa !10
  %1403 = add nuw nsw i64 %1383, 4
  %1404 = icmp eq i64 %1403, 1200
  br i1 %1404, label %1405, label %1382, !llvm.loop !11

1405:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr7()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 7)
  br label %1423

1406:
  %1407 = add nuw nsw i64 %1424, 1
  %1408 = getelementptr inbounds nuw i8, ptr @dst, i64 %1407
  %1409 = load i8, ptr %1408, align 1, !tbaa !10
  %1410 = getelementptr inbounds nuw i8, ptr @ref, i64 %1407
  %1411 = load i8, ptr %1410, align 1, !tbaa !10
  %1412 = icmp eq i8 %1409, %1411
  br i1 %1412, label %1413, label %1430

1413:
  %1414 = add nuw nsw i64 %1424, 2
  %1415 = getelementptr inbounds nuw i8, ptr @dst, i64 %1414
  %1416 = load i8, ptr %1415, align 1, !tbaa !10
  %1417 = getelementptr inbounds nuw i8, ptr @ref, i64 %1414
  %1418 = load i8, ptr %1417, align 1, !tbaa !10
  %1419 = icmp eq i8 %1416, %1418
  br i1 %1419, label %1420, label %1430

1420:
  %1421 = add nuw nsw i64 %1424, 3
  %1422 = icmp eq i64 %1421, 1200
  br i1 %1422, label %1431, label %1423, !llvm.loop !144

1423:
  %1424 = phi i64 [ 0, %1405 ], [ %1421, %1420 ]
  %1425 = getelementptr inbounds nuw i8, ptr @dst, i64 %1424
  %1426 = load i8, ptr %1425, align 1, !tbaa !10
  %1427 = getelementptr inbounds nuw i8, ptr @ref, i64 %1424
  %1428 = load i8, ptr %1427, align 1, !tbaa !10
  %1429 = icmp eq i8 %1426, %1428
  br i1 %1429, label %1406, label %1430

1430:
  tail call void @report(i64 noundef 7)
  store i1 true, ptr @bad, align 4
  br label %1432

1431:
  br label %1432

1432:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1433

1433:
  %1434 = phi i64 [ 0, %1432 ], [ %1454, %1433 ]
  %1435 = trunc i64 %1434 to i8
  %1436 = mul i8 %1435, 7
  %1437 = or disjoint i8 %1436, 3
  %1438 = getelementptr inbounds nuw i8, ptr @src, i64 %1434
  store i8 %1437, ptr %1438, align 4, !tbaa !10
  %1439 = or disjoint i64 %1434, 1
  %1440 = trunc i64 %1439 to i8
  %1441 = mul i8 %1440, 7
  %1442 = add i8 %1441, 3
  %1443 = getelementptr inbounds nuw i8, ptr @src, i64 %1439
  store i8 %1442, ptr %1443, align 1, !tbaa !10
  %1444 = or disjoint i64 %1434, 2
  %1445 = trunc i64 %1444 to i8
  %1446 = mul i8 %1445, 7
  %1447 = add i8 %1446, 3
  %1448 = getelementptr inbounds nuw i8, ptr @src, i64 %1444
  store i8 %1447, ptr %1448, align 2, !tbaa !10
  %1449 = or disjoint i64 %1434, 3
  %1450 = trunc i64 %1449 to i8
  %1451 = mul i8 %1450, 7
  %1452 = add i8 %1451, 3
  %1453 = getelementptr inbounds nuw i8, ptr @src, i64 %1449
  store i8 %1452, ptr %1453, align 1, !tbaa !10
  %1454 = add nuw nsw i64 %1434, 4
  %1455 = icmp eq i64 %1454, 1200
  br i1 %1455, label %1456, label %1433, !llvm.loop !11

1456:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv7(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 7)
  br label %1474

1457:
  %1458 = add nuw nsw i64 %1475, 1
  %1459 = getelementptr inbounds nuw i8, ptr @dst, i64 %1458
  %1460 = load i8, ptr %1459, align 1, !tbaa !10
  %1461 = getelementptr inbounds nuw i8, ptr @ref, i64 %1458
  %1462 = load i8, ptr %1461, align 1, !tbaa !10
  %1463 = icmp eq i8 %1460, %1462
  br i1 %1463, label %1464, label %1481

1464:
  %1465 = add nuw nsw i64 %1475, 2
  %1466 = getelementptr inbounds nuw i8, ptr @dst, i64 %1465
  %1467 = load i8, ptr %1466, align 1, !tbaa !10
  %1468 = getelementptr inbounds nuw i8, ptr @ref, i64 %1465
  %1469 = load i8, ptr %1468, align 1, !tbaa !10
  %1470 = icmp eq i8 %1467, %1469
  br i1 %1470, label %1471, label %1481

1471:
  %1472 = add nuw nsw i64 %1475, 3
  %1473 = icmp eq i64 %1472, 1200
  br i1 %1473, label %1482, label %1474, !llvm.loop !144

1474:
  %1475 = phi i64 [ 0, %1456 ], [ %1472, %1471 ]
  %1476 = getelementptr inbounds nuw i8, ptr @dst, i64 %1475
  %1477 = load i8, ptr %1476, align 1, !tbaa !10
  %1478 = getelementptr inbounds nuw i8, ptr @ref, i64 %1475
  %1479 = load i8, ptr %1478, align 1, !tbaa !10
  %1480 = icmp eq i8 %1477, %1479
  br i1 %1480, label %1457, label %1481

1481:
  tail call void @report(i64 noundef 7)
  store i1 true, ptr @bad, align 4
  br label %1483

1482:
  br label %1483

1483:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1484

1484:
  %1485 = phi i64 [ 0, %1483 ], [ %1505, %1484 ]
  %1486 = trunc i64 %1485 to i8
  %1487 = mul i8 %1486, 7
  %1488 = or disjoint i8 %1487, 3
  %1489 = getelementptr inbounds nuw i8, ptr @src, i64 %1485
  store i8 %1488, ptr %1489, align 4, !tbaa !10
  %1490 = or disjoint i64 %1485, 1
  %1491 = trunc i64 %1490 to i8
  %1492 = mul i8 %1491, 7
  %1493 = add i8 %1492, 3
  %1494 = getelementptr inbounds nuw i8, ptr @src, i64 %1490
  store i8 %1493, ptr %1494, align 1, !tbaa !10
  %1495 = or disjoint i64 %1485, 2
  %1496 = trunc i64 %1495 to i8
  %1497 = mul i8 %1496, 7
  %1498 = add i8 %1497, 3
  %1499 = getelementptr inbounds nuw i8, ptr @src, i64 %1495
  store i8 %1498, ptr %1499, align 2, !tbaa !10
  %1500 = or disjoint i64 %1485, 3
  %1501 = trunc i64 %1500 to i8
  %1502 = mul i8 %1501, 7
  %1503 = add i8 %1502, 3
  %1504 = getelementptr inbounds nuw i8, ptr @src, i64 %1500
  store i8 %1503, ptr %1504, align 1, !tbaa !10
  %1505 = add nuw nsw i64 %1485, 4
  %1506 = icmp eq i64 %1505, 1200
  br i1 %1506, label %1507, label %1484, !llvm.loop !11

1507:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv7(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 7)
  br label %1525

1508:
  %1509 = add nuw nsw i64 %1526, 1
  %1510 = getelementptr inbounds nuw i8, ptr @dst, i64 %1509
  %1511 = load i8, ptr %1510, align 1, !tbaa !10
  %1512 = getelementptr inbounds nuw i8, ptr @ref, i64 %1509
  %1513 = load i8, ptr %1512, align 1, !tbaa !10
  %1514 = icmp eq i8 %1511, %1513
  br i1 %1514, label %1515, label %1532

1515:
  %1516 = add nuw nsw i64 %1526, 2
  %1517 = getelementptr inbounds nuw i8, ptr @dst, i64 %1516
  %1518 = load i8, ptr %1517, align 1, !tbaa !10
  %1519 = getelementptr inbounds nuw i8, ptr @ref, i64 %1516
  %1520 = load i8, ptr %1519, align 1, !tbaa !10
  %1521 = icmp eq i8 %1518, %1520
  br i1 %1521, label %1522, label %1532

1522:
  %1523 = add nuw nsw i64 %1526, 3
  %1524 = icmp eq i64 %1523, 1200
  br i1 %1524, label %1533, label %1525, !llvm.loop !144

1525:
  %1526 = phi i64 [ 0, %1507 ], [ %1523, %1522 ]
  %1527 = getelementptr inbounds nuw i8, ptr @dst, i64 %1526
  %1528 = load i8, ptr %1527, align 1, !tbaa !10
  %1529 = getelementptr inbounds nuw i8, ptr @ref, i64 %1526
  %1530 = load i8, ptr %1529, align 1, !tbaa !10
  %1531 = icmp eq i8 %1528, %1530
  br i1 %1531, label %1508, label %1532

1532:
  tail call void @report(i64 noundef 7)
  store i1 true, ptr @bad, align 4
  br label %1534

1533:
  br label %1534

1534:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1535

1535:
  %1536 = phi i64 [ 0, %1534 ], [ %1556, %1535 ]
  %1537 = trunc i64 %1536 to i8
  %1538 = mul i8 %1537, 7
  %1539 = or disjoint i8 %1538, 3
  %1540 = getelementptr inbounds nuw i8, ptr @src, i64 %1536
  store i8 %1539, ptr %1540, align 4, !tbaa !10
  %1541 = or disjoint i64 %1536, 1
  %1542 = trunc i64 %1541 to i8
  %1543 = mul i8 %1542, 7
  %1544 = add i8 %1543, 3
  %1545 = getelementptr inbounds nuw i8, ptr @src, i64 %1541
  store i8 %1544, ptr %1545, align 1, !tbaa !10
  %1546 = or disjoint i64 %1536, 2
  %1547 = trunc i64 %1546 to i8
  %1548 = mul i8 %1547, 7
  %1549 = add i8 %1548, 3
  %1550 = getelementptr inbounds nuw i8, ptr @src, i64 %1546
  store i8 %1549, ptr %1550, align 2, !tbaa !10
  %1551 = or disjoint i64 %1536, 3
  %1552 = trunc i64 %1551 to i8
  %1553 = mul i8 %1552, 7
  %1554 = add i8 %1553, 3
  %1555 = getelementptr inbounds nuw i8, ptr @src, i64 %1551
  store i8 %1554, ptr %1555, align 1, !tbaa !10
  %1556 = add nuw nsw i64 %1536, 4
  %1557 = icmp eq i64 %1556, 1200
  br i1 %1557, label %1558, label %1535, !llvm.loop !11

1558:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp8()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 8)
  br label %1576

1559:
  %1560 = add nuw nsw i64 %1577, 1
  %1561 = getelementptr inbounds nuw i8, ptr @dst, i64 %1560
  %1562 = load i8, ptr %1561, align 1, !tbaa !10
  %1563 = getelementptr inbounds nuw i8, ptr @ref, i64 %1560
  %1564 = load i8, ptr %1563, align 1, !tbaa !10
  %1565 = icmp eq i8 %1562, %1564
  br i1 %1565, label %1566, label %1583

1566:
  %1567 = add nuw nsw i64 %1577, 2
  %1568 = getelementptr inbounds nuw i8, ptr @dst, i64 %1567
  %1569 = load i8, ptr %1568, align 1, !tbaa !10
  %1570 = getelementptr inbounds nuw i8, ptr @ref, i64 %1567
  %1571 = load i8, ptr %1570, align 1, !tbaa !10
  %1572 = icmp eq i8 %1569, %1571
  br i1 %1572, label %1573, label %1583

1573:
  %1574 = add nuw nsw i64 %1577, 3
  %1575 = icmp eq i64 %1574, 1200
  br i1 %1575, label %1584, label %1576, !llvm.loop !144

1576:
  %1577 = phi i64 [ 0, %1558 ], [ %1574, %1573 ]
  %1578 = getelementptr inbounds nuw i8, ptr @dst, i64 %1577
  %1579 = load i8, ptr %1578, align 1, !tbaa !10
  %1580 = getelementptr inbounds nuw i8, ptr @ref, i64 %1577
  %1581 = load i8, ptr %1580, align 1, !tbaa !10
  %1582 = icmp eq i8 %1579, %1581
  br i1 %1582, label %1559, label %1583

1583:
  tail call void @report(i64 noundef 8)
  store i1 true, ptr @bad, align 4
  br label %1585

1584:
  br label %1585

1585:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1586

1586:
  %1587 = phi i64 [ 0, %1585 ], [ %1607, %1586 ]
  %1588 = trunc i64 %1587 to i8
  %1589 = mul i8 %1588, 7
  %1590 = or disjoint i8 %1589, 3
  %1591 = getelementptr inbounds nuw i8, ptr @src, i64 %1587
  store i8 %1590, ptr %1591, align 4, !tbaa !10
  %1592 = or disjoint i64 %1587, 1
  %1593 = trunc i64 %1592 to i8
  %1594 = mul i8 %1593, 7
  %1595 = add i8 %1594, 3
  %1596 = getelementptr inbounds nuw i8, ptr @src, i64 %1592
  store i8 %1595, ptr %1596, align 1, !tbaa !10
  %1597 = or disjoint i64 %1587, 2
  %1598 = trunc i64 %1597 to i8
  %1599 = mul i8 %1598, 7
  %1600 = add i8 %1599, 3
  %1601 = getelementptr inbounds nuw i8, ptr @src, i64 %1597
  store i8 %1600, ptr %1601, align 2, !tbaa !10
  %1602 = or disjoint i64 %1587, 3
  %1603 = trunc i64 %1602 to i8
  %1604 = mul i8 %1603, 7
  %1605 = add i8 %1604, 3
  %1606 = getelementptr inbounds nuw i8, ptr @src, i64 %1602
  store i8 %1605, ptr %1606, align 1, !tbaa !10
  %1607 = add nuw nsw i64 %1587, 4
  %1608 = icmp eq i64 %1607, 1200
  br i1 %1608, label %1609, label %1586, !llvm.loop !11

1609:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr8()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 8)
  br label %1627

1610:
  %1611 = add nuw nsw i64 %1628, 1
  %1612 = getelementptr inbounds nuw i8, ptr @dst, i64 %1611
  %1613 = load i8, ptr %1612, align 1, !tbaa !10
  %1614 = getelementptr inbounds nuw i8, ptr @ref, i64 %1611
  %1615 = load i8, ptr %1614, align 1, !tbaa !10
  %1616 = icmp eq i8 %1613, %1615
  br i1 %1616, label %1617, label %1634

1617:
  %1618 = add nuw nsw i64 %1628, 2
  %1619 = getelementptr inbounds nuw i8, ptr @dst, i64 %1618
  %1620 = load i8, ptr %1619, align 1, !tbaa !10
  %1621 = getelementptr inbounds nuw i8, ptr @ref, i64 %1618
  %1622 = load i8, ptr %1621, align 1, !tbaa !10
  %1623 = icmp eq i8 %1620, %1622
  br i1 %1623, label %1624, label %1634

1624:
  %1625 = add nuw nsw i64 %1628, 3
  %1626 = icmp eq i64 %1625, 1200
  br i1 %1626, label %1635, label %1627, !llvm.loop !144

1627:
  %1628 = phi i64 [ 0, %1609 ], [ %1625, %1624 ]
  %1629 = getelementptr inbounds nuw i8, ptr @dst, i64 %1628
  %1630 = load i8, ptr %1629, align 1, !tbaa !10
  %1631 = getelementptr inbounds nuw i8, ptr @ref, i64 %1628
  %1632 = load i8, ptr %1631, align 1, !tbaa !10
  %1633 = icmp eq i8 %1630, %1632
  br i1 %1633, label %1610, label %1634

1634:
  tail call void @report(i64 noundef 8)
  store i1 true, ptr @bad, align 4
  br label %1636

1635:
  br label %1636

1636:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1637

1637:
  %1638 = phi i64 [ 0, %1636 ], [ %1658, %1637 ]
  %1639 = trunc i64 %1638 to i8
  %1640 = mul i8 %1639, 7
  %1641 = or disjoint i8 %1640, 3
  %1642 = getelementptr inbounds nuw i8, ptr @src, i64 %1638
  store i8 %1641, ptr %1642, align 4, !tbaa !10
  %1643 = or disjoint i64 %1638, 1
  %1644 = trunc i64 %1643 to i8
  %1645 = mul i8 %1644, 7
  %1646 = add i8 %1645, 3
  %1647 = getelementptr inbounds nuw i8, ptr @src, i64 %1643
  store i8 %1646, ptr %1647, align 1, !tbaa !10
  %1648 = or disjoint i64 %1638, 2
  %1649 = trunc i64 %1648 to i8
  %1650 = mul i8 %1649, 7
  %1651 = add i8 %1650, 3
  %1652 = getelementptr inbounds nuw i8, ptr @src, i64 %1648
  store i8 %1651, ptr %1652, align 2, !tbaa !10
  %1653 = or disjoint i64 %1638, 3
  %1654 = trunc i64 %1653 to i8
  %1655 = mul i8 %1654, 7
  %1656 = add i8 %1655, 3
  %1657 = getelementptr inbounds nuw i8, ptr @src, i64 %1653
  store i8 %1656, ptr %1657, align 1, !tbaa !10
  %1658 = add nuw nsw i64 %1638, 4
  %1659 = icmp eq i64 %1658, 1200
  br i1 %1659, label %1660, label %1637, !llvm.loop !11

1660:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c78()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 8)
  br label %1678

1661:
  %1662 = add nuw nsw i64 %1679, 1
  %1663 = getelementptr inbounds nuw i8, ptr @dst, i64 %1662
  %1664 = load i8, ptr %1663, align 1, !tbaa !10
  %1665 = getelementptr inbounds nuw i8, ptr @ref, i64 %1662
  %1666 = load i8, ptr %1665, align 1, !tbaa !10
  %1667 = icmp eq i8 %1664, %1666
  br i1 %1667, label %1668, label %1685

1668:
  %1669 = add nuw nsw i64 %1679, 2
  %1670 = getelementptr inbounds nuw i8, ptr @dst, i64 %1669
  %1671 = load i8, ptr %1670, align 1, !tbaa !10
  %1672 = getelementptr inbounds nuw i8, ptr @ref, i64 %1669
  %1673 = load i8, ptr %1672, align 1, !tbaa !10
  %1674 = icmp eq i8 %1671, %1673
  br i1 %1674, label %1675, label %1685

1675:
  %1676 = add nuw nsw i64 %1679, 3
  %1677 = icmp eq i64 %1676, 1200
  br i1 %1677, label %1686, label %1678, !llvm.loop !144

1678:
  %1679 = phi i64 [ 0, %1660 ], [ %1676, %1675 ]
  %1680 = getelementptr inbounds nuw i8, ptr @dst, i64 %1679
  %1681 = load i8, ptr %1680, align 1, !tbaa !10
  %1682 = getelementptr inbounds nuw i8, ptr @ref, i64 %1679
  %1683 = load i8, ptr %1682, align 1, !tbaa !10
  %1684 = icmp eq i8 %1681, %1683
  br i1 %1684, label %1661, label %1685

1685:
  tail call void @report(i64 noundef 8)
  store i1 true, ptr @bad, align 4
  br label %1687

1686:
  br label %1687

1687:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1688

1688:
  %1689 = phi i64 [ 0, %1687 ], [ %1709, %1688 ]
  %1690 = trunc i64 %1689 to i8
  %1691 = mul i8 %1690, 7
  %1692 = or disjoint i8 %1691, 3
  %1693 = getelementptr inbounds nuw i8, ptr @src, i64 %1689
  store i8 %1692, ptr %1693, align 4, !tbaa !10
  %1694 = or disjoint i64 %1689, 1
  %1695 = trunc i64 %1694 to i8
  %1696 = mul i8 %1695, 7
  %1697 = add i8 %1696, 3
  %1698 = getelementptr inbounds nuw i8, ptr @src, i64 %1694
  store i8 %1697, ptr %1698, align 1, !tbaa !10
  %1699 = or disjoint i64 %1689, 2
  %1700 = trunc i64 %1699 to i8
  %1701 = mul i8 %1700, 7
  %1702 = add i8 %1701, 3
  %1703 = getelementptr inbounds nuw i8, ptr @src, i64 %1699
  store i8 %1702, ptr %1703, align 2, !tbaa !10
  %1704 = or disjoint i64 %1689, 3
  %1705 = trunc i64 %1704 to i8
  %1706 = mul i8 %1705, 7
  %1707 = add i8 %1706, 3
  %1708 = getelementptr inbounds nuw i8, ptr @src, i64 %1704
  store i8 %1707, ptr %1708, align 1, !tbaa !10
  %1709 = add nuw nsw i64 %1689, 4
  %1710 = icmp eq i64 %1709, 1200
  br i1 %1710, label %1711, label %1688, !llvm.loop !11

1711:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr8()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 8)
  br label %1729

1712:
  %1713 = add nuw nsw i64 %1730, 1
  %1714 = getelementptr inbounds nuw i8, ptr @dst, i64 %1713
  %1715 = load i8, ptr %1714, align 1, !tbaa !10
  %1716 = getelementptr inbounds nuw i8, ptr @ref, i64 %1713
  %1717 = load i8, ptr %1716, align 1, !tbaa !10
  %1718 = icmp eq i8 %1715, %1717
  br i1 %1718, label %1719, label %1736

1719:
  %1720 = add nuw nsw i64 %1730, 2
  %1721 = getelementptr inbounds nuw i8, ptr @dst, i64 %1720
  %1722 = load i8, ptr %1721, align 1, !tbaa !10
  %1723 = getelementptr inbounds nuw i8, ptr @ref, i64 %1720
  %1724 = load i8, ptr %1723, align 1, !tbaa !10
  %1725 = icmp eq i8 %1722, %1724
  br i1 %1725, label %1726, label %1736

1726:
  %1727 = add nuw nsw i64 %1730, 3
  %1728 = icmp eq i64 %1727, 1200
  br i1 %1728, label %1737, label %1729, !llvm.loop !144

1729:
  %1730 = phi i64 [ 0, %1711 ], [ %1727, %1726 ]
  %1731 = getelementptr inbounds nuw i8, ptr @dst, i64 %1730
  %1732 = load i8, ptr %1731, align 1, !tbaa !10
  %1733 = getelementptr inbounds nuw i8, ptr @ref, i64 %1730
  %1734 = load i8, ptr %1733, align 1, !tbaa !10
  %1735 = icmp eq i8 %1732, %1734
  br i1 %1735, label %1712, label %1736

1736:
  tail call void @report(i64 noundef 8)
  store i1 true, ptr @bad, align 4
  br label %1738

1737:
  br label %1738

1738:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1739

1739:
  %1740 = phi i64 [ 0, %1738 ], [ %1760, %1739 ]
  %1741 = trunc i64 %1740 to i8
  %1742 = mul i8 %1741, 7
  %1743 = or disjoint i8 %1742, 3
  %1744 = getelementptr inbounds nuw i8, ptr @src, i64 %1740
  store i8 %1743, ptr %1744, align 4, !tbaa !10
  %1745 = or disjoint i64 %1740, 1
  %1746 = trunc i64 %1745 to i8
  %1747 = mul i8 %1746, 7
  %1748 = add i8 %1747, 3
  %1749 = getelementptr inbounds nuw i8, ptr @src, i64 %1745
  store i8 %1748, ptr %1749, align 1, !tbaa !10
  %1750 = or disjoint i64 %1740, 2
  %1751 = trunc i64 %1750 to i8
  %1752 = mul i8 %1751, 7
  %1753 = add i8 %1752, 3
  %1754 = getelementptr inbounds nuw i8, ptr @src, i64 %1750
  store i8 %1753, ptr %1754, align 2, !tbaa !10
  %1755 = or disjoint i64 %1740, 3
  %1756 = trunc i64 %1755 to i8
  %1757 = mul i8 %1756, 7
  %1758 = add i8 %1757, 3
  %1759 = getelementptr inbounds nuw i8, ptr @src, i64 %1755
  store i8 %1758, ptr %1759, align 1, !tbaa !10
  %1760 = add nuw nsw i64 %1740, 4
  %1761 = icmp eq i64 %1760, 1200
  br i1 %1761, label %1762, label %1739, !llvm.loop !11

1762:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  %1763 = load i64, ptr getelementptr inbounds nuw (i8, ptr @dst, i64 1)
  tail call void @mv8(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), i64 %1763)
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 8)
  br label %1781

1764:
  %1765 = add nuw nsw i64 %1782, 1
  %1766 = getelementptr inbounds nuw i8, ptr @dst, i64 %1765
  %1767 = load i8, ptr %1766, align 1, !tbaa !10
  %1768 = getelementptr inbounds nuw i8, ptr @ref, i64 %1765
  %1769 = load i8, ptr %1768, align 1, !tbaa !10
  %1770 = icmp eq i8 %1767, %1769
  br i1 %1770, label %1771, label %1788

1771:
  %1772 = add nuw nsw i64 %1782, 2
  %1773 = getelementptr inbounds nuw i8, ptr @dst, i64 %1772
  %1774 = load i8, ptr %1773, align 1, !tbaa !10
  %1775 = getelementptr inbounds nuw i8, ptr @ref, i64 %1772
  %1776 = load i8, ptr %1775, align 1, !tbaa !10
  %1777 = icmp eq i8 %1774, %1776
  br i1 %1777, label %1778, label %1788

1778:
  %1779 = add nuw nsw i64 %1782, 3
  %1780 = icmp eq i64 %1779, 1200
  br i1 %1780, label %1789, label %1781, !llvm.loop !144

1781:
  %1782 = phi i64 [ 0, %1762 ], [ %1779, %1778 ]
  %1783 = getelementptr inbounds nuw i8, ptr @dst, i64 %1782
  %1784 = load i8, ptr %1783, align 1, !tbaa !10
  %1785 = getelementptr inbounds nuw i8, ptr @ref, i64 %1782
  %1786 = load i8, ptr %1785, align 1, !tbaa !10
  %1787 = icmp eq i8 %1784, %1786
  br i1 %1787, label %1764, label %1788

1788:
  tail call void @report(i64 noundef 8)
  store i1 true, ptr @bad, align 4
  br label %1790

1789:
  br label %1790

1790:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1791

1791:
  %1792 = phi i64 [ 0, %1790 ], [ %1812, %1791 ]
  %1793 = trunc i64 %1792 to i8
  %1794 = mul i8 %1793, 7
  %1795 = or disjoint i8 %1794, 3
  %1796 = getelementptr inbounds nuw i8, ptr @src, i64 %1792
  store i8 %1795, ptr %1796, align 4, !tbaa !10
  %1797 = or disjoint i64 %1792, 1
  %1798 = trunc i64 %1797 to i8
  %1799 = mul i8 %1798, 7
  %1800 = add i8 %1799, 3
  %1801 = getelementptr inbounds nuw i8, ptr @src, i64 %1797
  store i8 %1800, ptr %1801, align 1, !tbaa !10
  %1802 = or disjoint i64 %1792, 2
  %1803 = trunc i64 %1802 to i8
  %1804 = mul i8 %1803, 7
  %1805 = add i8 %1804, 3
  %1806 = getelementptr inbounds nuw i8, ptr @src, i64 %1802
  store i8 %1805, ptr %1806, align 2, !tbaa !10
  %1807 = or disjoint i64 %1792, 3
  %1808 = trunc i64 %1807 to i8
  %1809 = mul i8 %1808, 7
  %1810 = add i8 %1809, 3
  %1811 = getelementptr inbounds nuw i8, ptr @src, i64 %1807
  store i8 %1810, ptr %1811, align 1, !tbaa !10
  %1812 = add nuw nsw i64 %1792, 4
  %1813 = icmp eq i64 %1812, 1200
  br i1 %1813, label %1814, label %1791, !llvm.loop !11

1814:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  %1815 = load i64, ptr getelementptr inbounds nuw (i8, ptr @dst, i64 4)
  tail call void @mv8(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), i64 %1815)
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 8)
  br label %1833

1816:
  %1817 = add nuw nsw i64 %1834, 1
  %1818 = getelementptr inbounds nuw i8, ptr @dst, i64 %1817
  %1819 = load i8, ptr %1818, align 1, !tbaa !10
  %1820 = getelementptr inbounds nuw i8, ptr @ref, i64 %1817
  %1821 = load i8, ptr %1820, align 1, !tbaa !10
  %1822 = icmp eq i8 %1819, %1821
  br i1 %1822, label %1823, label %1840

1823:
  %1824 = add nuw nsw i64 %1834, 2
  %1825 = getelementptr inbounds nuw i8, ptr @dst, i64 %1824
  %1826 = load i8, ptr %1825, align 1, !tbaa !10
  %1827 = getelementptr inbounds nuw i8, ptr @ref, i64 %1824
  %1828 = load i8, ptr %1827, align 1, !tbaa !10
  %1829 = icmp eq i8 %1826, %1828
  br i1 %1829, label %1830, label %1840

1830:
  %1831 = add nuw nsw i64 %1834, 3
  %1832 = icmp eq i64 %1831, 1200
  br i1 %1832, label %1841, label %1833, !llvm.loop !144

1833:
  %1834 = phi i64 [ 0, %1814 ], [ %1831, %1830 ]
  %1835 = getelementptr inbounds nuw i8, ptr @dst, i64 %1834
  %1836 = load i8, ptr %1835, align 1, !tbaa !10
  %1837 = getelementptr inbounds nuw i8, ptr @ref, i64 %1834
  %1838 = load i8, ptr %1837, align 1, !tbaa !10
  %1839 = icmp eq i8 %1836, %1838
  br i1 %1839, label %1816, label %1840

1840:
  tail call void @report(i64 noundef 8)
  store i1 true, ptr @bad, align 4
  br label %1842

1841:
  br label %1842

1842:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1843

1843:
  %1844 = phi i64 [ 0, %1842 ], [ %1864, %1843 ]
  %1845 = trunc i64 %1844 to i8
  %1846 = mul i8 %1845, 7
  %1847 = or disjoint i8 %1846, 3
  %1848 = getelementptr inbounds nuw i8, ptr @src, i64 %1844
  store i8 %1847, ptr %1848, align 4, !tbaa !10
  %1849 = or disjoint i64 %1844, 1
  %1850 = trunc i64 %1849 to i8
  %1851 = mul i8 %1850, 7
  %1852 = add i8 %1851, 3
  %1853 = getelementptr inbounds nuw i8, ptr @src, i64 %1849
  store i8 %1852, ptr %1853, align 1, !tbaa !10
  %1854 = or disjoint i64 %1844, 2
  %1855 = trunc i64 %1854 to i8
  %1856 = mul i8 %1855, 7
  %1857 = add i8 %1856, 3
  %1858 = getelementptr inbounds nuw i8, ptr @src, i64 %1854
  store i8 %1857, ptr %1858, align 2, !tbaa !10
  %1859 = or disjoint i64 %1844, 3
  %1860 = trunc i64 %1859 to i8
  %1861 = mul i8 %1860, 7
  %1862 = add i8 %1861, 3
  %1863 = getelementptr inbounds nuw i8, ptr @src, i64 %1859
  store i8 %1862, ptr %1863, align 1, !tbaa !10
  %1864 = add nuw nsw i64 %1844, 4
  %1865 = icmp eq i64 %1864, 1200
  br i1 %1865, label %1866, label %1843, !llvm.loop !11

1866:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp9()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 9)
  br label %1884

1867:
  %1868 = add nuw nsw i64 %1885, 1
  %1869 = getelementptr inbounds nuw i8, ptr @dst, i64 %1868
  %1870 = load i8, ptr %1869, align 1, !tbaa !10
  %1871 = getelementptr inbounds nuw i8, ptr @ref, i64 %1868
  %1872 = load i8, ptr %1871, align 1, !tbaa !10
  %1873 = icmp eq i8 %1870, %1872
  br i1 %1873, label %1874, label %1891

1874:
  %1875 = add nuw nsw i64 %1885, 2
  %1876 = getelementptr inbounds nuw i8, ptr @dst, i64 %1875
  %1877 = load i8, ptr %1876, align 1, !tbaa !10
  %1878 = getelementptr inbounds nuw i8, ptr @ref, i64 %1875
  %1879 = load i8, ptr %1878, align 1, !tbaa !10
  %1880 = icmp eq i8 %1877, %1879
  br i1 %1880, label %1881, label %1891

1881:
  %1882 = add nuw nsw i64 %1885, 3
  %1883 = icmp eq i64 %1882, 1200
  br i1 %1883, label %1892, label %1884, !llvm.loop !144

1884:
  %1885 = phi i64 [ 0, %1866 ], [ %1882, %1881 ]
  %1886 = getelementptr inbounds nuw i8, ptr @dst, i64 %1885
  %1887 = load i8, ptr %1886, align 1, !tbaa !10
  %1888 = getelementptr inbounds nuw i8, ptr @ref, i64 %1885
  %1889 = load i8, ptr %1888, align 1, !tbaa !10
  %1890 = icmp eq i8 %1887, %1889
  br i1 %1890, label %1867, label %1891

1891:
  tail call void @report(i64 noundef 9)
  store i1 true, ptr @bad, align 4
  br label %1893

1892:
  br label %1893

1893:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1894

1894:
  %1895 = phi i64 [ 0, %1893 ], [ %1915, %1894 ]
  %1896 = trunc i64 %1895 to i8
  %1897 = mul i8 %1896, 7
  %1898 = or disjoint i8 %1897, 3
  %1899 = getelementptr inbounds nuw i8, ptr @src, i64 %1895
  store i8 %1898, ptr %1899, align 4, !tbaa !10
  %1900 = or disjoint i64 %1895, 1
  %1901 = trunc i64 %1900 to i8
  %1902 = mul i8 %1901, 7
  %1903 = add i8 %1902, 3
  %1904 = getelementptr inbounds nuw i8, ptr @src, i64 %1900
  store i8 %1903, ptr %1904, align 1, !tbaa !10
  %1905 = or disjoint i64 %1895, 2
  %1906 = trunc i64 %1905 to i8
  %1907 = mul i8 %1906, 7
  %1908 = add i8 %1907, 3
  %1909 = getelementptr inbounds nuw i8, ptr @src, i64 %1905
  store i8 %1908, ptr %1909, align 2, !tbaa !10
  %1910 = or disjoint i64 %1895, 3
  %1911 = trunc i64 %1910 to i8
  %1912 = mul i8 %1911, 7
  %1913 = add i8 %1912, 3
  %1914 = getelementptr inbounds nuw i8, ptr @src, i64 %1910
  store i8 %1913, ptr %1914, align 1, !tbaa !10
  %1915 = add nuw nsw i64 %1895, 4
  %1916 = icmp eq i64 %1915, 1200
  br i1 %1916, label %1917, label %1894, !llvm.loop !11

1917:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr9()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 9)
  br label %1935

1918:
  %1919 = add nuw nsw i64 %1936, 1
  %1920 = getelementptr inbounds nuw i8, ptr @dst, i64 %1919
  %1921 = load i8, ptr %1920, align 1, !tbaa !10
  %1922 = getelementptr inbounds nuw i8, ptr @ref, i64 %1919
  %1923 = load i8, ptr %1922, align 1, !tbaa !10
  %1924 = icmp eq i8 %1921, %1923
  br i1 %1924, label %1925, label %1942

1925:
  %1926 = add nuw nsw i64 %1936, 2
  %1927 = getelementptr inbounds nuw i8, ptr @dst, i64 %1926
  %1928 = load i8, ptr %1927, align 1, !tbaa !10
  %1929 = getelementptr inbounds nuw i8, ptr @ref, i64 %1926
  %1930 = load i8, ptr %1929, align 1, !tbaa !10
  %1931 = icmp eq i8 %1928, %1930
  br i1 %1931, label %1932, label %1942

1932:
  %1933 = add nuw nsw i64 %1936, 3
  %1934 = icmp eq i64 %1933, 1200
  br i1 %1934, label %1943, label %1935, !llvm.loop !144

1935:
  %1936 = phi i64 [ 0, %1917 ], [ %1933, %1932 ]
  %1937 = getelementptr inbounds nuw i8, ptr @dst, i64 %1936
  %1938 = load i8, ptr %1937, align 1, !tbaa !10
  %1939 = getelementptr inbounds nuw i8, ptr @ref, i64 %1936
  %1940 = load i8, ptr %1939, align 1, !tbaa !10
  %1941 = icmp eq i8 %1938, %1940
  br i1 %1941, label %1918, label %1942

1942:
  tail call void @report(i64 noundef 9)
  store i1 true, ptr @bad, align 4
  br label %1944

1943:
  br label %1944

1944:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1945

1945:
  %1946 = phi i64 [ 0, %1944 ], [ %1966, %1945 ]
  %1947 = trunc i64 %1946 to i8
  %1948 = mul i8 %1947, 7
  %1949 = or disjoint i8 %1948, 3
  %1950 = getelementptr inbounds nuw i8, ptr @src, i64 %1946
  store i8 %1949, ptr %1950, align 4, !tbaa !10
  %1951 = or disjoint i64 %1946, 1
  %1952 = trunc i64 %1951 to i8
  %1953 = mul i8 %1952, 7
  %1954 = add i8 %1953, 3
  %1955 = getelementptr inbounds nuw i8, ptr @src, i64 %1951
  store i8 %1954, ptr %1955, align 1, !tbaa !10
  %1956 = or disjoint i64 %1946, 2
  %1957 = trunc i64 %1956 to i8
  %1958 = mul i8 %1957, 7
  %1959 = add i8 %1958, 3
  %1960 = getelementptr inbounds nuw i8, ptr @src, i64 %1956
  store i8 %1959, ptr %1960, align 2, !tbaa !10
  %1961 = or disjoint i64 %1946, 3
  %1962 = trunc i64 %1961 to i8
  %1963 = mul i8 %1962, 7
  %1964 = add i8 %1963, 3
  %1965 = getelementptr inbounds nuw i8, ptr @src, i64 %1961
  store i8 %1964, ptr %1965, align 1, !tbaa !10
  %1966 = add nuw nsw i64 %1946, 4
  %1967 = icmp eq i64 %1966, 1200
  br i1 %1967, label %1968, label %1945, !llvm.loop !11

1968:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c79()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 9)
  br label %1986

1969:
  %1970 = add nuw nsw i64 %1987, 1
  %1971 = getelementptr inbounds nuw i8, ptr @dst, i64 %1970
  %1972 = load i8, ptr %1971, align 1, !tbaa !10
  %1973 = getelementptr inbounds nuw i8, ptr @ref, i64 %1970
  %1974 = load i8, ptr %1973, align 1, !tbaa !10
  %1975 = icmp eq i8 %1972, %1974
  br i1 %1975, label %1976, label %1993

1976:
  %1977 = add nuw nsw i64 %1987, 2
  %1978 = getelementptr inbounds nuw i8, ptr @dst, i64 %1977
  %1979 = load i8, ptr %1978, align 1, !tbaa !10
  %1980 = getelementptr inbounds nuw i8, ptr @ref, i64 %1977
  %1981 = load i8, ptr %1980, align 1, !tbaa !10
  %1982 = icmp eq i8 %1979, %1981
  br i1 %1982, label %1983, label %1993

1983:
  %1984 = add nuw nsw i64 %1987, 3
  %1985 = icmp eq i64 %1984, 1200
  br i1 %1985, label %1994, label %1986, !llvm.loop !144

1986:
  %1987 = phi i64 [ 0, %1968 ], [ %1984, %1983 ]
  %1988 = getelementptr inbounds nuw i8, ptr @dst, i64 %1987
  %1989 = load i8, ptr %1988, align 1, !tbaa !10
  %1990 = getelementptr inbounds nuw i8, ptr @ref, i64 %1987
  %1991 = load i8, ptr %1990, align 1, !tbaa !10
  %1992 = icmp eq i8 %1989, %1991
  br i1 %1992, label %1969, label %1993

1993:
  tail call void @report(i64 noundef 9)
  store i1 true, ptr @bad, align 4
  br label %1995

1994:
  br label %1995

1995:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %1996

1996:
  %1997 = phi i64 [ 0, %1995 ], [ %2017, %1996 ]
  %1998 = trunc i64 %1997 to i8
  %1999 = mul i8 %1998, 7
  %2000 = or disjoint i8 %1999, 3
  %2001 = getelementptr inbounds nuw i8, ptr @src, i64 %1997
  store i8 %2000, ptr %2001, align 4, !tbaa !10
  %2002 = or disjoint i64 %1997, 1
  %2003 = trunc i64 %2002 to i8
  %2004 = mul i8 %2003, 7
  %2005 = add i8 %2004, 3
  %2006 = getelementptr inbounds nuw i8, ptr @src, i64 %2002
  store i8 %2005, ptr %2006, align 1, !tbaa !10
  %2007 = or disjoint i64 %1997, 2
  %2008 = trunc i64 %2007 to i8
  %2009 = mul i8 %2008, 7
  %2010 = add i8 %2009, 3
  %2011 = getelementptr inbounds nuw i8, ptr @src, i64 %2007
  store i8 %2010, ptr %2011, align 2, !tbaa !10
  %2012 = or disjoint i64 %1997, 3
  %2013 = trunc i64 %2012 to i8
  %2014 = mul i8 %2013, 7
  %2015 = add i8 %2014, 3
  %2016 = getelementptr inbounds nuw i8, ptr @src, i64 %2012
  store i8 %2015, ptr %2016, align 1, !tbaa !10
  %2017 = add nuw nsw i64 %1997, 4
  %2018 = icmp eq i64 %2017, 1200
  br i1 %2018, label %2019, label %1996, !llvm.loop !11

2019:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr9()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 9)
  br label %2037

2020:
  %2021 = add nuw nsw i64 %2038, 1
  %2022 = getelementptr inbounds nuw i8, ptr @dst, i64 %2021
  %2023 = load i8, ptr %2022, align 1, !tbaa !10
  %2024 = getelementptr inbounds nuw i8, ptr @ref, i64 %2021
  %2025 = load i8, ptr %2024, align 1, !tbaa !10
  %2026 = icmp eq i8 %2023, %2025
  br i1 %2026, label %2027, label %2044

2027:
  %2028 = add nuw nsw i64 %2038, 2
  %2029 = getelementptr inbounds nuw i8, ptr @dst, i64 %2028
  %2030 = load i8, ptr %2029, align 1, !tbaa !10
  %2031 = getelementptr inbounds nuw i8, ptr @ref, i64 %2028
  %2032 = load i8, ptr %2031, align 1, !tbaa !10
  %2033 = icmp eq i8 %2030, %2032
  br i1 %2033, label %2034, label %2044

2034:
  %2035 = add nuw nsw i64 %2038, 3
  %2036 = icmp eq i64 %2035, 1200
  br i1 %2036, label %2045, label %2037, !llvm.loop !144

2037:
  %2038 = phi i64 [ 0, %2019 ], [ %2035, %2034 ]
  %2039 = getelementptr inbounds nuw i8, ptr @dst, i64 %2038
  %2040 = load i8, ptr %2039, align 1, !tbaa !10
  %2041 = getelementptr inbounds nuw i8, ptr @ref, i64 %2038
  %2042 = load i8, ptr %2041, align 1, !tbaa !10
  %2043 = icmp eq i8 %2040, %2042
  br i1 %2043, label %2020, label %2044

2044:
  tail call void @report(i64 noundef 9)
  store i1 true, ptr @bad, align 4
  br label %2046

2045:
  br label %2046

2046:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2047

2047:
  %2048 = phi i64 [ 0, %2046 ], [ %2068, %2047 ]
  %2049 = trunc i64 %2048 to i8
  %2050 = mul i8 %2049, 7
  %2051 = or disjoint i8 %2050, 3
  %2052 = getelementptr inbounds nuw i8, ptr @src, i64 %2048
  store i8 %2051, ptr %2052, align 4, !tbaa !10
  %2053 = or disjoint i64 %2048, 1
  %2054 = trunc i64 %2053 to i8
  %2055 = mul i8 %2054, 7
  %2056 = add i8 %2055, 3
  %2057 = getelementptr inbounds nuw i8, ptr @src, i64 %2053
  store i8 %2056, ptr %2057, align 1, !tbaa !10
  %2058 = or disjoint i64 %2048, 2
  %2059 = trunc i64 %2058 to i8
  %2060 = mul i8 %2059, 7
  %2061 = add i8 %2060, 3
  %2062 = getelementptr inbounds nuw i8, ptr @src, i64 %2058
  store i8 %2061, ptr %2062, align 2, !tbaa !10
  %2063 = or disjoint i64 %2048, 3
  %2064 = trunc i64 %2063 to i8
  %2065 = mul i8 %2064, 7
  %2066 = add i8 %2065, 3
  %2067 = getelementptr inbounds nuw i8, ptr @src, i64 %2063
  store i8 %2066, ptr %2067, align 1, !tbaa !10
  %2068 = add nuw nsw i64 %2048, 4
  %2069 = icmp eq i64 %2068, 1200
  br i1 %2069, label %2070, label %2047, !llvm.loop !11

2070:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv9(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 9)
  br label %2088

2071:
  %2072 = add nuw nsw i64 %2089, 1
  %2073 = getelementptr inbounds nuw i8, ptr @dst, i64 %2072
  %2074 = load i8, ptr %2073, align 1, !tbaa !10
  %2075 = getelementptr inbounds nuw i8, ptr @ref, i64 %2072
  %2076 = load i8, ptr %2075, align 1, !tbaa !10
  %2077 = icmp eq i8 %2074, %2076
  br i1 %2077, label %2078, label %2095

2078:
  %2079 = add nuw nsw i64 %2089, 2
  %2080 = getelementptr inbounds nuw i8, ptr @dst, i64 %2079
  %2081 = load i8, ptr %2080, align 1, !tbaa !10
  %2082 = getelementptr inbounds nuw i8, ptr @ref, i64 %2079
  %2083 = load i8, ptr %2082, align 1, !tbaa !10
  %2084 = icmp eq i8 %2081, %2083
  br i1 %2084, label %2085, label %2095

2085:
  %2086 = add nuw nsw i64 %2089, 3
  %2087 = icmp eq i64 %2086, 1200
  br i1 %2087, label %2096, label %2088, !llvm.loop !144

2088:
  %2089 = phi i64 [ 0, %2070 ], [ %2086, %2085 ]
  %2090 = getelementptr inbounds nuw i8, ptr @dst, i64 %2089
  %2091 = load i8, ptr %2090, align 1, !tbaa !10
  %2092 = getelementptr inbounds nuw i8, ptr @ref, i64 %2089
  %2093 = load i8, ptr %2092, align 1, !tbaa !10
  %2094 = icmp eq i8 %2091, %2093
  br i1 %2094, label %2071, label %2095

2095:
  tail call void @report(i64 noundef 9)
  store i1 true, ptr @bad, align 4
  br label %2097

2096:
  br label %2097

2097:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2098

2098:
  %2099 = phi i64 [ 0, %2097 ], [ %2119, %2098 ]
  %2100 = trunc i64 %2099 to i8
  %2101 = mul i8 %2100, 7
  %2102 = or disjoint i8 %2101, 3
  %2103 = getelementptr inbounds nuw i8, ptr @src, i64 %2099
  store i8 %2102, ptr %2103, align 4, !tbaa !10
  %2104 = or disjoint i64 %2099, 1
  %2105 = trunc i64 %2104 to i8
  %2106 = mul i8 %2105, 7
  %2107 = add i8 %2106, 3
  %2108 = getelementptr inbounds nuw i8, ptr @src, i64 %2104
  store i8 %2107, ptr %2108, align 1, !tbaa !10
  %2109 = or disjoint i64 %2099, 2
  %2110 = trunc i64 %2109 to i8
  %2111 = mul i8 %2110, 7
  %2112 = add i8 %2111, 3
  %2113 = getelementptr inbounds nuw i8, ptr @src, i64 %2109
  store i8 %2112, ptr %2113, align 2, !tbaa !10
  %2114 = or disjoint i64 %2099, 3
  %2115 = trunc i64 %2114 to i8
  %2116 = mul i8 %2115, 7
  %2117 = add i8 %2116, 3
  %2118 = getelementptr inbounds nuw i8, ptr @src, i64 %2114
  store i8 %2117, ptr %2118, align 1, !tbaa !10
  %2119 = add nuw nsw i64 %2099, 4
  %2120 = icmp eq i64 %2119, 1200
  br i1 %2120, label %2121, label %2098, !llvm.loop !11

2121:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv9(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 9)
  br label %2139

2122:
  %2123 = add nuw nsw i64 %2140, 1
  %2124 = getelementptr inbounds nuw i8, ptr @dst, i64 %2123
  %2125 = load i8, ptr %2124, align 1, !tbaa !10
  %2126 = getelementptr inbounds nuw i8, ptr @ref, i64 %2123
  %2127 = load i8, ptr %2126, align 1, !tbaa !10
  %2128 = icmp eq i8 %2125, %2127
  br i1 %2128, label %2129, label %2146

2129:
  %2130 = add nuw nsw i64 %2140, 2
  %2131 = getelementptr inbounds nuw i8, ptr @dst, i64 %2130
  %2132 = load i8, ptr %2131, align 1, !tbaa !10
  %2133 = getelementptr inbounds nuw i8, ptr @ref, i64 %2130
  %2134 = load i8, ptr %2133, align 1, !tbaa !10
  %2135 = icmp eq i8 %2132, %2134
  br i1 %2135, label %2136, label %2146

2136:
  %2137 = add nuw nsw i64 %2140, 3
  %2138 = icmp eq i64 %2137, 1200
  br i1 %2138, label %2147, label %2139, !llvm.loop !144

2139:
  %2140 = phi i64 [ 0, %2121 ], [ %2137, %2136 ]
  %2141 = getelementptr inbounds nuw i8, ptr @dst, i64 %2140
  %2142 = load i8, ptr %2141, align 1, !tbaa !10
  %2143 = getelementptr inbounds nuw i8, ptr @ref, i64 %2140
  %2144 = load i8, ptr %2143, align 1, !tbaa !10
  %2145 = icmp eq i8 %2142, %2144
  br i1 %2145, label %2122, label %2146

2146:
  tail call void @report(i64 noundef 9)
  store i1 true, ptr @bad, align 4
  br label %2148

2147:
  br label %2148

2148:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2149

2149:
  %2150 = phi i64 [ 0, %2148 ], [ %2170, %2149 ]
  %2151 = trunc i64 %2150 to i8
  %2152 = mul i8 %2151, 7
  %2153 = or disjoint i8 %2152, 3
  %2154 = getelementptr inbounds nuw i8, ptr @src, i64 %2150
  store i8 %2153, ptr %2154, align 4, !tbaa !10
  %2155 = or disjoint i64 %2150, 1
  %2156 = trunc i64 %2155 to i8
  %2157 = mul i8 %2156, 7
  %2158 = add i8 %2157, 3
  %2159 = getelementptr inbounds nuw i8, ptr @src, i64 %2155
  store i8 %2158, ptr %2159, align 1, !tbaa !10
  %2160 = or disjoint i64 %2150, 2
  %2161 = trunc i64 %2160 to i8
  %2162 = mul i8 %2161, 7
  %2163 = add i8 %2162, 3
  %2164 = getelementptr inbounds nuw i8, ptr @src, i64 %2160
  store i8 %2163, ptr %2164, align 2, !tbaa !10
  %2165 = or disjoint i64 %2150, 3
  %2166 = trunc i64 %2165 to i8
  %2167 = mul i8 %2166, 7
  %2168 = add i8 %2167, 3
  %2169 = getelementptr inbounds nuw i8, ptr @src, i64 %2165
  store i8 %2168, ptr %2169, align 1, !tbaa !10
  %2170 = add nuw nsw i64 %2150, 4
  %2171 = icmp eq i64 %2170, 1200
  br i1 %2171, label %2172, label %2149, !llvm.loop !11

2172:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp15()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 15)
  br label %2190

2173:
  %2174 = add nuw nsw i64 %2191, 1
  %2175 = getelementptr inbounds nuw i8, ptr @dst, i64 %2174
  %2176 = load i8, ptr %2175, align 1, !tbaa !10
  %2177 = getelementptr inbounds nuw i8, ptr @ref, i64 %2174
  %2178 = load i8, ptr %2177, align 1, !tbaa !10
  %2179 = icmp eq i8 %2176, %2178
  br i1 %2179, label %2180, label %2197

2180:
  %2181 = add nuw nsw i64 %2191, 2
  %2182 = getelementptr inbounds nuw i8, ptr @dst, i64 %2181
  %2183 = load i8, ptr %2182, align 1, !tbaa !10
  %2184 = getelementptr inbounds nuw i8, ptr @ref, i64 %2181
  %2185 = load i8, ptr %2184, align 1, !tbaa !10
  %2186 = icmp eq i8 %2183, %2185
  br i1 %2186, label %2187, label %2197

2187:
  %2188 = add nuw nsw i64 %2191, 3
  %2189 = icmp eq i64 %2188, 1200
  br i1 %2189, label %2198, label %2190, !llvm.loop !144

2190:
  %2191 = phi i64 [ 0, %2172 ], [ %2188, %2187 ]
  %2192 = getelementptr inbounds nuw i8, ptr @dst, i64 %2191
  %2193 = load i8, ptr %2192, align 1, !tbaa !10
  %2194 = getelementptr inbounds nuw i8, ptr @ref, i64 %2191
  %2195 = load i8, ptr %2194, align 1, !tbaa !10
  %2196 = icmp eq i8 %2193, %2195
  br i1 %2196, label %2173, label %2197

2197:
  tail call void @report(i64 noundef 15)
  store i1 true, ptr @bad, align 4
  br label %2199

2198:
  br label %2199

2199:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2200

2200:
  %2201 = phi i64 [ 0, %2199 ], [ %2221, %2200 ]
  %2202 = trunc i64 %2201 to i8
  %2203 = mul i8 %2202, 7
  %2204 = or disjoint i8 %2203, 3
  %2205 = getelementptr inbounds nuw i8, ptr @src, i64 %2201
  store i8 %2204, ptr %2205, align 4, !tbaa !10
  %2206 = or disjoint i64 %2201, 1
  %2207 = trunc i64 %2206 to i8
  %2208 = mul i8 %2207, 7
  %2209 = add i8 %2208, 3
  %2210 = getelementptr inbounds nuw i8, ptr @src, i64 %2206
  store i8 %2209, ptr %2210, align 1, !tbaa !10
  %2211 = or disjoint i64 %2201, 2
  %2212 = trunc i64 %2211 to i8
  %2213 = mul i8 %2212, 7
  %2214 = add i8 %2213, 3
  %2215 = getelementptr inbounds nuw i8, ptr @src, i64 %2211
  store i8 %2214, ptr %2215, align 2, !tbaa !10
  %2216 = or disjoint i64 %2201, 3
  %2217 = trunc i64 %2216 to i8
  %2218 = mul i8 %2217, 7
  %2219 = add i8 %2218, 3
  %2220 = getelementptr inbounds nuw i8, ptr @src, i64 %2216
  store i8 %2219, ptr %2220, align 1, !tbaa !10
  %2221 = add nuw nsw i64 %2201, 4
  %2222 = icmp eq i64 %2221, 1200
  br i1 %2222, label %2223, label %2200, !llvm.loop !11

2223:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr15()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 15)
  br label %2241

2224:
  %2225 = add nuw nsw i64 %2242, 1
  %2226 = getelementptr inbounds nuw i8, ptr @dst, i64 %2225
  %2227 = load i8, ptr %2226, align 1, !tbaa !10
  %2228 = getelementptr inbounds nuw i8, ptr @ref, i64 %2225
  %2229 = load i8, ptr %2228, align 1, !tbaa !10
  %2230 = icmp eq i8 %2227, %2229
  br i1 %2230, label %2231, label %2248

2231:
  %2232 = add nuw nsw i64 %2242, 2
  %2233 = getelementptr inbounds nuw i8, ptr @dst, i64 %2232
  %2234 = load i8, ptr %2233, align 1, !tbaa !10
  %2235 = getelementptr inbounds nuw i8, ptr @ref, i64 %2232
  %2236 = load i8, ptr %2235, align 1, !tbaa !10
  %2237 = icmp eq i8 %2234, %2236
  br i1 %2237, label %2238, label %2248

2238:
  %2239 = add nuw nsw i64 %2242, 3
  %2240 = icmp eq i64 %2239, 1200
  br i1 %2240, label %2249, label %2241, !llvm.loop !144

2241:
  %2242 = phi i64 [ 0, %2223 ], [ %2239, %2238 ]
  %2243 = getelementptr inbounds nuw i8, ptr @dst, i64 %2242
  %2244 = load i8, ptr %2243, align 1, !tbaa !10
  %2245 = getelementptr inbounds nuw i8, ptr @ref, i64 %2242
  %2246 = load i8, ptr %2245, align 1, !tbaa !10
  %2247 = icmp eq i8 %2244, %2246
  br i1 %2247, label %2224, label %2248

2248:
  tail call void @report(i64 noundef 15)
  store i1 true, ptr @bad, align 4
  br label %2250

2249:
  br label %2250

2250:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2251

2251:
  %2252 = phi i64 [ 0, %2250 ], [ %2272, %2251 ]
  %2253 = trunc i64 %2252 to i8
  %2254 = mul i8 %2253, 7
  %2255 = or disjoint i8 %2254, 3
  %2256 = getelementptr inbounds nuw i8, ptr @src, i64 %2252
  store i8 %2255, ptr %2256, align 4, !tbaa !10
  %2257 = or disjoint i64 %2252, 1
  %2258 = trunc i64 %2257 to i8
  %2259 = mul i8 %2258, 7
  %2260 = add i8 %2259, 3
  %2261 = getelementptr inbounds nuw i8, ptr @src, i64 %2257
  store i8 %2260, ptr %2261, align 1, !tbaa !10
  %2262 = or disjoint i64 %2252, 2
  %2263 = trunc i64 %2262 to i8
  %2264 = mul i8 %2263, 7
  %2265 = add i8 %2264, 3
  %2266 = getelementptr inbounds nuw i8, ptr @src, i64 %2262
  store i8 %2265, ptr %2266, align 2, !tbaa !10
  %2267 = or disjoint i64 %2252, 3
  %2268 = trunc i64 %2267 to i8
  %2269 = mul i8 %2268, 7
  %2270 = add i8 %2269, 3
  %2271 = getelementptr inbounds nuw i8, ptr @src, i64 %2267
  store i8 %2270, ptr %2271, align 1, !tbaa !10
  %2272 = add nuw nsw i64 %2252, 4
  %2273 = icmp eq i64 %2272, 1200
  br i1 %2273, label %2274, label %2251, !llvm.loop !11

2274:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c715()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 15)
  br label %2292

2275:
  %2276 = add nuw nsw i64 %2293, 1
  %2277 = getelementptr inbounds nuw i8, ptr @dst, i64 %2276
  %2278 = load i8, ptr %2277, align 1, !tbaa !10
  %2279 = getelementptr inbounds nuw i8, ptr @ref, i64 %2276
  %2280 = load i8, ptr %2279, align 1, !tbaa !10
  %2281 = icmp eq i8 %2278, %2280
  br i1 %2281, label %2282, label %2299

2282:
  %2283 = add nuw nsw i64 %2293, 2
  %2284 = getelementptr inbounds nuw i8, ptr @dst, i64 %2283
  %2285 = load i8, ptr %2284, align 1, !tbaa !10
  %2286 = getelementptr inbounds nuw i8, ptr @ref, i64 %2283
  %2287 = load i8, ptr %2286, align 1, !tbaa !10
  %2288 = icmp eq i8 %2285, %2287
  br i1 %2288, label %2289, label %2299

2289:
  %2290 = add nuw nsw i64 %2293, 3
  %2291 = icmp eq i64 %2290, 1200
  br i1 %2291, label %2300, label %2292, !llvm.loop !144

2292:
  %2293 = phi i64 [ 0, %2274 ], [ %2290, %2289 ]
  %2294 = getelementptr inbounds nuw i8, ptr @dst, i64 %2293
  %2295 = load i8, ptr %2294, align 1, !tbaa !10
  %2296 = getelementptr inbounds nuw i8, ptr @ref, i64 %2293
  %2297 = load i8, ptr %2296, align 1, !tbaa !10
  %2298 = icmp eq i8 %2295, %2297
  br i1 %2298, label %2275, label %2299

2299:
  tail call void @report(i64 noundef 15)
  store i1 true, ptr @bad, align 4
  br label %2301

2300:
  br label %2301

2301:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2302

2302:
  %2303 = phi i64 [ 0, %2301 ], [ %2323, %2302 ]
  %2304 = trunc i64 %2303 to i8
  %2305 = mul i8 %2304, 7
  %2306 = or disjoint i8 %2305, 3
  %2307 = getelementptr inbounds nuw i8, ptr @src, i64 %2303
  store i8 %2306, ptr %2307, align 4, !tbaa !10
  %2308 = or disjoint i64 %2303, 1
  %2309 = trunc i64 %2308 to i8
  %2310 = mul i8 %2309, 7
  %2311 = add i8 %2310, 3
  %2312 = getelementptr inbounds nuw i8, ptr @src, i64 %2308
  store i8 %2311, ptr %2312, align 1, !tbaa !10
  %2313 = or disjoint i64 %2303, 2
  %2314 = trunc i64 %2313 to i8
  %2315 = mul i8 %2314, 7
  %2316 = add i8 %2315, 3
  %2317 = getelementptr inbounds nuw i8, ptr @src, i64 %2313
  store i8 %2316, ptr %2317, align 2, !tbaa !10
  %2318 = or disjoint i64 %2303, 3
  %2319 = trunc i64 %2318 to i8
  %2320 = mul i8 %2319, 7
  %2321 = add i8 %2320, 3
  %2322 = getelementptr inbounds nuw i8, ptr @src, i64 %2318
  store i8 %2321, ptr %2322, align 1, !tbaa !10
  %2323 = add nuw nsw i64 %2303, 4
  %2324 = icmp eq i64 %2323, 1200
  br i1 %2324, label %2325, label %2302, !llvm.loop !11

2325:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr15()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 15)
  br label %2343

2326:
  %2327 = add nuw nsw i64 %2344, 1
  %2328 = getelementptr inbounds nuw i8, ptr @dst, i64 %2327
  %2329 = load i8, ptr %2328, align 1, !tbaa !10
  %2330 = getelementptr inbounds nuw i8, ptr @ref, i64 %2327
  %2331 = load i8, ptr %2330, align 1, !tbaa !10
  %2332 = icmp eq i8 %2329, %2331
  br i1 %2332, label %2333, label %2350

2333:
  %2334 = add nuw nsw i64 %2344, 2
  %2335 = getelementptr inbounds nuw i8, ptr @dst, i64 %2334
  %2336 = load i8, ptr %2335, align 1, !tbaa !10
  %2337 = getelementptr inbounds nuw i8, ptr @ref, i64 %2334
  %2338 = load i8, ptr %2337, align 1, !tbaa !10
  %2339 = icmp eq i8 %2336, %2338
  br i1 %2339, label %2340, label %2350

2340:
  %2341 = add nuw nsw i64 %2344, 3
  %2342 = icmp eq i64 %2341, 1200
  br i1 %2342, label %2351, label %2343, !llvm.loop !144

2343:
  %2344 = phi i64 [ 0, %2325 ], [ %2341, %2340 ]
  %2345 = getelementptr inbounds nuw i8, ptr @dst, i64 %2344
  %2346 = load i8, ptr %2345, align 1, !tbaa !10
  %2347 = getelementptr inbounds nuw i8, ptr @ref, i64 %2344
  %2348 = load i8, ptr %2347, align 1, !tbaa !10
  %2349 = icmp eq i8 %2346, %2348
  br i1 %2349, label %2326, label %2350

2350:
  tail call void @report(i64 noundef 15)
  store i1 true, ptr @bad, align 4
  br label %2352

2351:
  br label %2352

2352:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2353

2353:
  %2354 = phi i64 [ 0, %2352 ], [ %2374, %2353 ]
  %2355 = trunc i64 %2354 to i8
  %2356 = mul i8 %2355, 7
  %2357 = or disjoint i8 %2356, 3
  %2358 = getelementptr inbounds nuw i8, ptr @src, i64 %2354
  store i8 %2357, ptr %2358, align 4, !tbaa !10
  %2359 = or disjoint i64 %2354, 1
  %2360 = trunc i64 %2359 to i8
  %2361 = mul i8 %2360, 7
  %2362 = add i8 %2361, 3
  %2363 = getelementptr inbounds nuw i8, ptr @src, i64 %2359
  store i8 %2362, ptr %2363, align 1, !tbaa !10
  %2364 = or disjoint i64 %2354, 2
  %2365 = trunc i64 %2364 to i8
  %2366 = mul i8 %2365, 7
  %2367 = add i8 %2366, 3
  %2368 = getelementptr inbounds nuw i8, ptr @src, i64 %2364
  store i8 %2367, ptr %2368, align 2, !tbaa !10
  %2369 = or disjoint i64 %2354, 3
  %2370 = trunc i64 %2369 to i8
  %2371 = mul i8 %2370, 7
  %2372 = add i8 %2371, 3
  %2373 = getelementptr inbounds nuw i8, ptr @src, i64 %2369
  store i8 %2372, ptr %2373, align 1, !tbaa !10
  %2374 = add nuw nsw i64 %2354, 4
  %2375 = icmp eq i64 %2374, 1200
  br i1 %2375, label %2376, label %2353, !llvm.loop !11

2376:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv15(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 15)
  br label %2394

2377:
  %2378 = add nuw nsw i64 %2395, 1
  %2379 = getelementptr inbounds nuw i8, ptr @dst, i64 %2378
  %2380 = load i8, ptr %2379, align 1, !tbaa !10
  %2381 = getelementptr inbounds nuw i8, ptr @ref, i64 %2378
  %2382 = load i8, ptr %2381, align 1, !tbaa !10
  %2383 = icmp eq i8 %2380, %2382
  br i1 %2383, label %2384, label %2401

2384:
  %2385 = add nuw nsw i64 %2395, 2
  %2386 = getelementptr inbounds nuw i8, ptr @dst, i64 %2385
  %2387 = load i8, ptr %2386, align 1, !tbaa !10
  %2388 = getelementptr inbounds nuw i8, ptr @ref, i64 %2385
  %2389 = load i8, ptr %2388, align 1, !tbaa !10
  %2390 = icmp eq i8 %2387, %2389
  br i1 %2390, label %2391, label %2401

2391:
  %2392 = add nuw nsw i64 %2395, 3
  %2393 = icmp eq i64 %2392, 1200
  br i1 %2393, label %2402, label %2394, !llvm.loop !144

2394:
  %2395 = phi i64 [ 0, %2376 ], [ %2392, %2391 ]
  %2396 = getelementptr inbounds nuw i8, ptr @dst, i64 %2395
  %2397 = load i8, ptr %2396, align 1, !tbaa !10
  %2398 = getelementptr inbounds nuw i8, ptr @ref, i64 %2395
  %2399 = load i8, ptr %2398, align 1, !tbaa !10
  %2400 = icmp eq i8 %2397, %2399
  br i1 %2400, label %2377, label %2401

2401:
  tail call void @report(i64 noundef 15)
  store i1 true, ptr @bad, align 4
  br label %2403

2402:
  br label %2403

2403:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2404

2404:
  %2405 = phi i64 [ 0, %2403 ], [ %2425, %2404 ]
  %2406 = trunc i64 %2405 to i8
  %2407 = mul i8 %2406, 7
  %2408 = or disjoint i8 %2407, 3
  %2409 = getelementptr inbounds nuw i8, ptr @src, i64 %2405
  store i8 %2408, ptr %2409, align 4, !tbaa !10
  %2410 = or disjoint i64 %2405, 1
  %2411 = trunc i64 %2410 to i8
  %2412 = mul i8 %2411, 7
  %2413 = add i8 %2412, 3
  %2414 = getelementptr inbounds nuw i8, ptr @src, i64 %2410
  store i8 %2413, ptr %2414, align 1, !tbaa !10
  %2415 = or disjoint i64 %2405, 2
  %2416 = trunc i64 %2415 to i8
  %2417 = mul i8 %2416, 7
  %2418 = add i8 %2417, 3
  %2419 = getelementptr inbounds nuw i8, ptr @src, i64 %2415
  store i8 %2418, ptr %2419, align 2, !tbaa !10
  %2420 = or disjoint i64 %2405, 3
  %2421 = trunc i64 %2420 to i8
  %2422 = mul i8 %2421, 7
  %2423 = add i8 %2422, 3
  %2424 = getelementptr inbounds nuw i8, ptr @src, i64 %2420
  store i8 %2423, ptr %2424, align 1, !tbaa !10
  %2425 = add nuw nsw i64 %2405, 4
  %2426 = icmp eq i64 %2425, 1200
  br i1 %2426, label %2427, label %2404, !llvm.loop !11

2427:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv15(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 15)
  br label %2445

2428:
  %2429 = add nuw nsw i64 %2446, 1
  %2430 = getelementptr inbounds nuw i8, ptr @dst, i64 %2429
  %2431 = load i8, ptr %2430, align 1, !tbaa !10
  %2432 = getelementptr inbounds nuw i8, ptr @ref, i64 %2429
  %2433 = load i8, ptr %2432, align 1, !tbaa !10
  %2434 = icmp eq i8 %2431, %2433
  br i1 %2434, label %2435, label %2452

2435:
  %2436 = add nuw nsw i64 %2446, 2
  %2437 = getelementptr inbounds nuw i8, ptr @dst, i64 %2436
  %2438 = load i8, ptr %2437, align 1, !tbaa !10
  %2439 = getelementptr inbounds nuw i8, ptr @ref, i64 %2436
  %2440 = load i8, ptr %2439, align 1, !tbaa !10
  %2441 = icmp eq i8 %2438, %2440
  br i1 %2441, label %2442, label %2452

2442:
  %2443 = add nuw nsw i64 %2446, 3
  %2444 = icmp eq i64 %2443, 1200
  br i1 %2444, label %2453, label %2445, !llvm.loop !144

2445:
  %2446 = phi i64 [ 0, %2427 ], [ %2443, %2442 ]
  %2447 = getelementptr inbounds nuw i8, ptr @dst, i64 %2446
  %2448 = load i8, ptr %2447, align 1, !tbaa !10
  %2449 = getelementptr inbounds nuw i8, ptr @ref, i64 %2446
  %2450 = load i8, ptr %2449, align 1, !tbaa !10
  %2451 = icmp eq i8 %2448, %2450
  br i1 %2451, label %2428, label %2452

2452:
  tail call void @report(i64 noundef 15)
  store i1 true, ptr @bad, align 4
  br label %2454

2453:
  br label %2454

2454:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2455

2455:
  %2456 = phi i64 [ 0, %2454 ], [ %2476, %2455 ]
  %2457 = trunc i64 %2456 to i8
  %2458 = mul i8 %2457, 7
  %2459 = or disjoint i8 %2458, 3
  %2460 = getelementptr inbounds nuw i8, ptr @src, i64 %2456
  store i8 %2459, ptr %2460, align 4, !tbaa !10
  %2461 = or disjoint i64 %2456, 1
  %2462 = trunc i64 %2461 to i8
  %2463 = mul i8 %2462, 7
  %2464 = add i8 %2463, 3
  %2465 = getelementptr inbounds nuw i8, ptr @src, i64 %2461
  store i8 %2464, ptr %2465, align 1, !tbaa !10
  %2466 = or disjoint i64 %2456, 2
  %2467 = trunc i64 %2466 to i8
  %2468 = mul i8 %2467, 7
  %2469 = add i8 %2468, 3
  %2470 = getelementptr inbounds nuw i8, ptr @src, i64 %2466
  store i8 %2469, ptr %2470, align 2, !tbaa !10
  %2471 = or disjoint i64 %2456, 3
  %2472 = trunc i64 %2471 to i8
  %2473 = mul i8 %2472, 7
  %2474 = add i8 %2473, 3
  %2475 = getelementptr inbounds nuw i8, ptr @src, i64 %2471
  store i8 %2474, ptr %2475, align 1, !tbaa !10
  %2476 = add nuw nsw i64 %2456, 4
  %2477 = icmp eq i64 %2476, 1200
  br i1 %2477, label %2478, label %2455, !llvm.loop !11

2478:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp16()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 16)
  br label %2496

2479:
  %2480 = add nuw nsw i64 %2497, 1
  %2481 = getelementptr inbounds nuw i8, ptr @dst, i64 %2480
  %2482 = load i8, ptr %2481, align 1, !tbaa !10
  %2483 = getelementptr inbounds nuw i8, ptr @ref, i64 %2480
  %2484 = load i8, ptr %2483, align 1, !tbaa !10
  %2485 = icmp eq i8 %2482, %2484
  br i1 %2485, label %2486, label %2503

2486:
  %2487 = add nuw nsw i64 %2497, 2
  %2488 = getelementptr inbounds nuw i8, ptr @dst, i64 %2487
  %2489 = load i8, ptr %2488, align 1, !tbaa !10
  %2490 = getelementptr inbounds nuw i8, ptr @ref, i64 %2487
  %2491 = load i8, ptr %2490, align 1, !tbaa !10
  %2492 = icmp eq i8 %2489, %2491
  br i1 %2492, label %2493, label %2503

2493:
  %2494 = add nuw nsw i64 %2497, 3
  %2495 = icmp eq i64 %2494, 1200
  br i1 %2495, label %2504, label %2496, !llvm.loop !144

2496:
  %2497 = phi i64 [ 0, %2478 ], [ %2494, %2493 ]
  %2498 = getelementptr inbounds nuw i8, ptr @dst, i64 %2497
  %2499 = load i8, ptr %2498, align 1, !tbaa !10
  %2500 = getelementptr inbounds nuw i8, ptr @ref, i64 %2497
  %2501 = load i8, ptr %2500, align 1, !tbaa !10
  %2502 = icmp eq i8 %2499, %2501
  br i1 %2502, label %2479, label %2503

2503:
  tail call void @report(i64 noundef 16)
  store i1 true, ptr @bad, align 4
  br label %2505

2504:
  br label %2505

2505:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2506

2506:
  %2507 = phi i64 [ 0, %2505 ], [ %2527, %2506 ]
  %2508 = trunc i64 %2507 to i8
  %2509 = mul i8 %2508, 7
  %2510 = or disjoint i8 %2509, 3
  %2511 = getelementptr inbounds nuw i8, ptr @src, i64 %2507
  store i8 %2510, ptr %2511, align 4, !tbaa !10
  %2512 = or disjoint i64 %2507, 1
  %2513 = trunc i64 %2512 to i8
  %2514 = mul i8 %2513, 7
  %2515 = add i8 %2514, 3
  %2516 = getelementptr inbounds nuw i8, ptr @src, i64 %2512
  store i8 %2515, ptr %2516, align 1, !tbaa !10
  %2517 = or disjoint i64 %2507, 2
  %2518 = trunc i64 %2517 to i8
  %2519 = mul i8 %2518, 7
  %2520 = add i8 %2519, 3
  %2521 = getelementptr inbounds nuw i8, ptr @src, i64 %2517
  store i8 %2520, ptr %2521, align 2, !tbaa !10
  %2522 = or disjoint i64 %2507, 3
  %2523 = trunc i64 %2522 to i8
  %2524 = mul i8 %2523, 7
  %2525 = add i8 %2524, 3
  %2526 = getelementptr inbounds nuw i8, ptr @src, i64 %2522
  store i8 %2525, ptr %2526, align 1, !tbaa !10
  %2527 = add nuw nsw i64 %2507, 4
  %2528 = icmp eq i64 %2527, 1200
  br i1 %2528, label %2529, label %2506, !llvm.loop !11

2529:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr16()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 16)
  br label %2547

2530:
  %2531 = add nuw nsw i64 %2548, 1
  %2532 = getelementptr inbounds nuw i8, ptr @dst, i64 %2531
  %2533 = load i8, ptr %2532, align 1, !tbaa !10
  %2534 = getelementptr inbounds nuw i8, ptr @ref, i64 %2531
  %2535 = load i8, ptr %2534, align 1, !tbaa !10
  %2536 = icmp eq i8 %2533, %2535
  br i1 %2536, label %2537, label %2554

2537:
  %2538 = add nuw nsw i64 %2548, 2
  %2539 = getelementptr inbounds nuw i8, ptr @dst, i64 %2538
  %2540 = load i8, ptr %2539, align 1, !tbaa !10
  %2541 = getelementptr inbounds nuw i8, ptr @ref, i64 %2538
  %2542 = load i8, ptr %2541, align 1, !tbaa !10
  %2543 = icmp eq i8 %2540, %2542
  br i1 %2543, label %2544, label %2554

2544:
  %2545 = add nuw nsw i64 %2548, 3
  %2546 = icmp eq i64 %2545, 1200
  br i1 %2546, label %2555, label %2547, !llvm.loop !144

2547:
  %2548 = phi i64 [ 0, %2529 ], [ %2545, %2544 ]
  %2549 = getelementptr inbounds nuw i8, ptr @dst, i64 %2548
  %2550 = load i8, ptr %2549, align 1, !tbaa !10
  %2551 = getelementptr inbounds nuw i8, ptr @ref, i64 %2548
  %2552 = load i8, ptr %2551, align 1, !tbaa !10
  %2553 = icmp eq i8 %2550, %2552
  br i1 %2553, label %2530, label %2554

2554:
  tail call void @report(i64 noundef 16)
  store i1 true, ptr @bad, align 4
  br label %2556

2555:
  br label %2556

2556:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2557

2557:
  %2558 = phi i64 [ 0, %2556 ], [ %2578, %2557 ]
  %2559 = trunc i64 %2558 to i8
  %2560 = mul i8 %2559, 7
  %2561 = or disjoint i8 %2560, 3
  %2562 = getelementptr inbounds nuw i8, ptr @src, i64 %2558
  store i8 %2561, ptr %2562, align 4, !tbaa !10
  %2563 = or disjoint i64 %2558, 1
  %2564 = trunc i64 %2563 to i8
  %2565 = mul i8 %2564, 7
  %2566 = add i8 %2565, 3
  %2567 = getelementptr inbounds nuw i8, ptr @src, i64 %2563
  store i8 %2566, ptr %2567, align 1, !tbaa !10
  %2568 = or disjoint i64 %2558, 2
  %2569 = trunc i64 %2568 to i8
  %2570 = mul i8 %2569, 7
  %2571 = add i8 %2570, 3
  %2572 = getelementptr inbounds nuw i8, ptr @src, i64 %2568
  store i8 %2571, ptr %2572, align 2, !tbaa !10
  %2573 = or disjoint i64 %2558, 3
  %2574 = trunc i64 %2573 to i8
  %2575 = mul i8 %2574, 7
  %2576 = add i8 %2575, 3
  %2577 = getelementptr inbounds nuw i8, ptr @src, i64 %2573
  store i8 %2576, ptr %2577, align 1, !tbaa !10
  %2578 = add nuw nsw i64 %2558, 4
  %2579 = icmp eq i64 %2578, 1200
  br i1 %2579, label %2580, label %2557, !llvm.loop !11

2580:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c716()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 16)
  br label %2598

2581:
  %2582 = add nuw nsw i64 %2599, 1
  %2583 = getelementptr inbounds nuw i8, ptr @dst, i64 %2582
  %2584 = load i8, ptr %2583, align 1, !tbaa !10
  %2585 = getelementptr inbounds nuw i8, ptr @ref, i64 %2582
  %2586 = load i8, ptr %2585, align 1, !tbaa !10
  %2587 = icmp eq i8 %2584, %2586
  br i1 %2587, label %2588, label %2605

2588:
  %2589 = add nuw nsw i64 %2599, 2
  %2590 = getelementptr inbounds nuw i8, ptr @dst, i64 %2589
  %2591 = load i8, ptr %2590, align 1, !tbaa !10
  %2592 = getelementptr inbounds nuw i8, ptr @ref, i64 %2589
  %2593 = load i8, ptr %2592, align 1, !tbaa !10
  %2594 = icmp eq i8 %2591, %2593
  br i1 %2594, label %2595, label %2605

2595:
  %2596 = add nuw nsw i64 %2599, 3
  %2597 = icmp eq i64 %2596, 1200
  br i1 %2597, label %2606, label %2598, !llvm.loop !144

2598:
  %2599 = phi i64 [ 0, %2580 ], [ %2596, %2595 ]
  %2600 = getelementptr inbounds nuw i8, ptr @dst, i64 %2599
  %2601 = load i8, ptr %2600, align 1, !tbaa !10
  %2602 = getelementptr inbounds nuw i8, ptr @ref, i64 %2599
  %2603 = load i8, ptr %2602, align 1, !tbaa !10
  %2604 = icmp eq i8 %2601, %2603
  br i1 %2604, label %2581, label %2605

2605:
  tail call void @report(i64 noundef 16)
  store i1 true, ptr @bad, align 4
  br label %2607

2606:
  br label %2607

2607:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2608

2608:
  %2609 = phi i64 [ 0, %2607 ], [ %2629, %2608 ]
  %2610 = trunc i64 %2609 to i8
  %2611 = mul i8 %2610, 7
  %2612 = or disjoint i8 %2611, 3
  %2613 = getelementptr inbounds nuw i8, ptr @src, i64 %2609
  store i8 %2612, ptr %2613, align 4, !tbaa !10
  %2614 = or disjoint i64 %2609, 1
  %2615 = trunc i64 %2614 to i8
  %2616 = mul i8 %2615, 7
  %2617 = add i8 %2616, 3
  %2618 = getelementptr inbounds nuw i8, ptr @src, i64 %2614
  store i8 %2617, ptr %2618, align 1, !tbaa !10
  %2619 = or disjoint i64 %2609, 2
  %2620 = trunc i64 %2619 to i8
  %2621 = mul i8 %2620, 7
  %2622 = add i8 %2621, 3
  %2623 = getelementptr inbounds nuw i8, ptr @src, i64 %2619
  store i8 %2622, ptr %2623, align 2, !tbaa !10
  %2624 = or disjoint i64 %2609, 3
  %2625 = trunc i64 %2624 to i8
  %2626 = mul i8 %2625, 7
  %2627 = add i8 %2626, 3
  %2628 = getelementptr inbounds nuw i8, ptr @src, i64 %2624
  store i8 %2627, ptr %2628, align 1, !tbaa !10
  %2629 = add nuw nsw i64 %2609, 4
  %2630 = icmp eq i64 %2629, 1200
  br i1 %2630, label %2631, label %2608, !llvm.loop !11

2631:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr16()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 16)
  br label %2649

2632:
  %2633 = add nuw nsw i64 %2650, 1
  %2634 = getelementptr inbounds nuw i8, ptr @dst, i64 %2633
  %2635 = load i8, ptr %2634, align 1, !tbaa !10
  %2636 = getelementptr inbounds nuw i8, ptr @ref, i64 %2633
  %2637 = load i8, ptr %2636, align 1, !tbaa !10
  %2638 = icmp eq i8 %2635, %2637
  br i1 %2638, label %2639, label %2656

2639:
  %2640 = add nuw nsw i64 %2650, 2
  %2641 = getelementptr inbounds nuw i8, ptr @dst, i64 %2640
  %2642 = load i8, ptr %2641, align 1, !tbaa !10
  %2643 = getelementptr inbounds nuw i8, ptr @ref, i64 %2640
  %2644 = load i8, ptr %2643, align 1, !tbaa !10
  %2645 = icmp eq i8 %2642, %2644
  br i1 %2645, label %2646, label %2656

2646:
  %2647 = add nuw nsw i64 %2650, 3
  %2648 = icmp eq i64 %2647, 1200
  br i1 %2648, label %2657, label %2649, !llvm.loop !144

2649:
  %2650 = phi i64 [ 0, %2631 ], [ %2647, %2646 ]
  %2651 = getelementptr inbounds nuw i8, ptr @dst, i64 %2650
  %2652 = load i8, ptr %2651, align 1, !tbaa !10
  %2653 = getelementptr inbounds nuw i8, ptr @ref, i64 %2650
  %2654 = load i8, ptr %2653, align 1, !tbaa !10
  %2655 = icmp eq i8 %2652, %2654
  br i1 %2655, label %2632, label %2656

2656:
  tail call void @report(i64 noundef 16)
  store i1 true, ptr @bad, align 4
  br label %2658

2657:
  br label %2658

2658:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2659

2659:
  %2660 = phi i64 [ 0, %2658 ], [ %2680, %2659 ]
  %2661 = trunc i64 %2660 to i8
  %2662 = mul i8 %2661, 7
  %2663 = or disjoint i8 %2662, 3
  %2664 = getelementptr inbounds nuw i8, ptr @src, i64 %2660
  store i8 %2663, ptr %2664, align 4, !tbaa !10
  %2665 = or disjoint i64 %2660, 1
  %2666 = trunc i64 %2665 to i8
  %2667 = mul i8 %2666, 7
  %2668 = add i8 %2667, 3
  %2669 = getelementptr inbounds nuw i8, ptr @src, i64 %2665
  store i8 %2668, ptr %2669, align 1, !tbaa !10
  %2670 = or disjoint i64 %2660, 2
  %2671 = trunc i64 %2670 to i8
  %2672 = mul i8 %2671, 7
  %2673 = add i8 %2672, 3
  %2674 = getelementptr inbounds nuw i8, ptr @src, i64 %2670
  store i8 %2673, ptr %2674, align 2, !tbaa !10
  %2675 = or disjoint i64 %2660, 3
  %2676 = trunc i64 %2675 to i8
  %2677 = mul i8 %2676, 7
  %2678 = add i8 %2677, 3
  %2679 = getelementptr inbounds nuw i8, ptr @src, i64 %2675
  store i8 %2678, ptr %2679, align 1, !tbaa !10
  %2680 = add nuw nsw i64 %2660, 4
  %2681 = icmp eq i64 %2680, 1200
  br i1 %2681, label %2682, label %2659, !llvm.loop !11

2682:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv16(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 16)
  br label %2700

2683:
  %2684 = add nuw nsw i64 %2701, 1
  %2685 = getelementptr inbounds nuw i8, ptr @dst, i64 %2684
  %2686 = load i8, ptr %2685, align 1, !tbaa !10
  %2687 = getelementptr inbounds nuw i8, ptr @ref, i64 %2684
  %2688 = load i8, ptr %2687, align 1, !tbaa !10
  %2689 = icmp eq i8 %2686, %2688
  br i1 %2689, label %2690, label %2707

2690:
  %2691 = add nuw nsw i64 %2701, 2
  %2692 = getelementptr inbounds nuw i8, ptr @dst, i64 %2691
  %2693 = load i8, ptr %2692, align 1, !tbaa !10
  %2694 = getelementptr inbounds nuw i8, ptr @ref, i64 %2691
  %2695 = load i8, ptr %2694, align 1, !tbaa !10
  %2696 = icmp eq i8 %2693, %2695
  br i1 %2696, label %2697, label %2707

2697:
  %2698 = add nuw nsw i64 %2701, 3
  %2699 = icmp eq i64 %2698, 1200
  br i1 %2699, label %2708, label %2700, !llvm.loop !144

2700:
  %2701 = phi i64 [ 0, %2682 ], [ %2698, %2697 ]
  %2702 = getelementptr inbounds nuw i8, ptr @dst, i64 %2701
  %2703 = load i8, ptr %2702, align 1, !tbaa !10
  %2704 = getelementptr inbounds nuw i8, ptr @ref, i64 %2701
  %2705 = load i8, ptr %2704, align 1, !tbaa !10
  %2706 = icmp eq i8 %2703, %2705
  br i1 %2706, label %2683, label %2707

2707:
  tail call void @report(i64 noundef 16)
  store i1 true, ptr @bad, align 4
  br label %2709

2708:
  br label %2709

2709:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2710

2710:
  %2711 = phi i64 [ 0, %2709 ], [ %2731, %2710 ]
  %2712 = trunc i64 %2711 to i8
  %2713 = mul i8 %2712, 7
  %2714 = or disjoint i8 %2713, 3
  %2715 = getelementptr inbounds nuw i8, ptr @src, i64 %2711
  store i8 %2714, ptr %2715, align 4, !tbaa !10
  %2716 = or disjoint i64 %2711, 1
  %2717 = trunc i64 %2716 to i8
  %2718 = mul i8 %2717, 7
  %2719 = add i8 %2718, 3
  %2720 = getelementptr inbounds nuw i8, ptr @src, i64 %2716
  store i8 %2719, ptr %2720, align 1, !tbaa !10
  %2721 = or disjoint i64 %2711, 2
  %2722 = trunc i64 %2721 to i8
  %2723 = mul i8 %2722, 7
  %2724 = add i8 %2723, 3
  %2725 = getelementptr inbounds nuw i8, ptr @src, i64 %2721
  store i8 %2724, ptr %2725, align 2, !tbaa !10
  %2726 = or disjoint i64 %2711, 3
  %2727 = trunc i64 %2726 to i8
  %2728 = mul i8 %2727, 7
  %2729 = add i8 %2728, 3
  %2730 = getelementptr inbounds nuw i8, ptr @src, i64 %2726
  store i8 %2729, ptr %2730, align 1, !tbaa !10
  %2731 = add nuw nsw i64 %2711, 4
  %2732 = icmp eq i64 %2731, 1200
  br i1 %2732, label %2733, label %2710, !llvm.loop !11

2733:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv16(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 16)
  br label %2751

2734:
  %2735 = add nuw nsw i64 %2752, 1
  %2736 = getelementptr inbounds nuw i8, ptr @dst, i64 %2735
  %2737 = load i8, ptr %2736, align 1, !tbaa !10
  %2738 = getelementptr inbounds nuw i8, ptr @ref, i64 %2735
  %2739 = load i8, ptr %2738, align 1, !tbaa !10
  %2740 = icmp eq i8 %2737, %2739
  br i1 %2740, label %2741, label %2758

2741:
  %2742 = add nuw nsw i64 %2752, 2
  %2743 = getelementptr inbounds nuw i8, ptr @dst, i64 %2742
  %2744 = load i8, ptr %2743, align 1, !tbaa !10
  %2745 = getelementptr inbounds nuw i8, ptr @ref, i64 %2742
  %2746 = load i8, ptr %2745, align 1, !tbaa !10
  %2747 = icmp eq i8 %2744, %2746
  br i1 %2747, label %2748, label %2758

2748:
  %2749 = add nuw nsw i64 %2752, 3
  %2750 = icmp eq i64 %2749, 1200
  br i1 %2750, label %2759, label %2751, !llvm.loop !144

2751:
  %2752 = phi i64 [ 0, %2733 ], [ %2749, %2748 ]
  %2753 = getelementptr inbounds nuw i8, ptr @dst, i64 %2752
  %2754 = load i8, ptr %2753, align 1, !tbaa !10
  %2755 = getelementptr inbounds nuw i8, ptr @ref, i64 %2752
  %2756 = load i8, ptr %2755, align 1, !tbaa !10
  %2757 = icmp eq i8 %2754, %2756
  br i1 %2757, label %2734, label %2758

2758:
  tail call void @report(i64 noundef 16)
  store i1 true, ptr @bad, align 4
  br label %2760

2759:
  br label %2760

2760:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2761

2761:
  %2762 = phi i64 [ 0, %2760 ], [ %2782, %2761 ]
  %2763 = trunc i64 %2762 to i8
  %2764 = mul i8 %2763, 7
  %2765 = or disjoint i8 %2764, 3
  %2766 = getelementptr inbounds nuw i8, ptr @src, i64 %2762
  store i8 %2765, ptr %2766, align 4, !tbaa !10
  %2767 = or disjoint i64 %2762, 1
  %2768 = trunc i64 %2767 to i8
  %2769 = mul i8 %2768, 7
  %2770 = add i8 %2769, 3
  %2771 = getelementptr inbounds nuw i8, ptr @src, i64 %2767
  store i8 %2770, ptr %2771, align 1, !tbaa !10
  %2772 = or disjoint i64 %2762, 2
  %2773 = trunc i64 %2772 to i8
  %2774 = mul i8 %2773, 7
  %2775 = add i8 %2774, 3
  %2776 = getelementptr inbounds nuw i8, ptr @src, i64 %2772
  store i8 %2775, ptr %2776, align 2, !tbaa !10
  %2777 = or disjoint i64 %2762, 3
  %2778 = trunc i64 %2777 to i8
  %2779 = mul i8 %2778, 7
  %2780 = add i8 %2779, 3
  %2781 = getelementptr inbounds nuw i8, ptr @src, i64 %2777
  store i8 %2780, ptr %2781, align 1, !tbaa !10
  %2782 = add nuw nsw i64 %2762, 4
  %2783 = icmp eq i64 %2782, 1200
  br i1 %2783, label %2784, label %2761, !llvm.loop !11

2784:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp17()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 17)
  br label %2802

2785:
  %2786 = add nuw nsw i64 %2803, 1
  %2787 = getelementptr inbounds nuw i8, ptr @dst, i64 %2786
  %2788 = load i8, ptr %2787, align 1, !tbaa !10
  %2789 = getelementptr inbounds nuw i8, ptr @ref, i64 %2786
  %2790 = load i8, ptr %2789, align 1, !tbaa !10
  %2791 = icmp eq i8 %2788, %2790
  br i1 %2791, label %2792, label %2809

2792:
  %2793 = add nuw nsw i64 %2803, 2
  %2794 = getelementptr inbounds nuw i8, ptr @dst, i64 %2793
  %2795 = load i8, ptr %2794, align 1, !tbaa !10
  %2796 = getelementptr inbounds nuw i8, ptr @ref, i64 %2793
  %2797 = load i8, ptr %2796, align 1, !tbaa !10
  %2798 = icmp eq i8 %2795, %2797
  br i1 %2798, label %2799, label %2809

2799:
  %2800 = add nuw nsw i64 %2803, 3
  %2801 = icmp eq i64 %2800, 1200
  br i1 %2801, label %2810, label %2802, !llvm.loop !144

2802:
  %2803 = phi i64 [ 0, %2784 ], [ %2800, %2799 ]
  %2804 = getelementptr inbounds nuw i8, ptr @dst, i64 %2803
  %2805 = load i8, ptr %2804, align 1, !tbaa !10
  %2806 = getelementptr inbounds nuw i8, ptr @ref, i64 %2803
  %2807 = load i8, ptr %2806, align 1, !tbaa !10
  %2808 = icmp eq i8 %2805, %2807
  br i1 %2808, label %2785, label %2809

2809:
  tail call void @report(i64 noundef 17)
  store i1 true, ptr @bad, align 4
  br label %2811

2810:
  br label %2811

2811:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2812

2812:
  %2813 = phi i64 [ 0, %2811 ], [ %2833, %2812 ]
  %2814 = trunc i64 %2813 to i8
  %2815 = mul i8 %2814, 7
  %2816 = or disjoint i8 %2815, 3
  %2817 = getelementptr inbounds nuw i8, ptr @src, i64 %2813
  store i8 %2816, ptr %2817, align 4, !tbaa !10
  %2818 = or disjoint i64 %2813, 1
  %2819 = trunc i64 %2818 to i8
  %2820 = mul i8 %2819, 7
  %2821 = add i8 %2820, 3
  %2822 = getelementptr inbounds nuw i8, ptr @src, i64 %2818
  store i8 %2821, ptr %2822, align 1, !tbaa !10
  %2823 = or disjoint i64 %2813, 2
  %2824 = trunc i64 %2823 to i8
  %2825 = mul i8 %2824, 7
  %2826 = add i8 %2825, 3
  %2827 = getelementptr inbounds nuw i8, ptr @src, i64 %2823
  store i8 %2826, ptr %2827, align 2, !tbaa !10
  %2828 = or disjoint i64 %2813, 3
  %2829 = trunc i64 %2828 to i8
  %2830 = mul i8 %2829, 7
  %2831 = add i8 %2830, 3
  %2832 = getelementptr inbounds nuw i8, ptr @src, i64 %2828
  store i8 %2831, ptr %2832, align 1, !tbaa !10
  %2833 = add nuw nsw i64 %2813, 4
  %2834 = icmp eq i64 %2833, 1200
  br i1 %2834, label %2835, label %2812, !llvm.loop !11

2835:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr17()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 17)
  br label %2853

2836:
  %2837 = add nuw nsw i64 %2854, 1
  %2838 = getelementptr inbounds nuw i8, ptr @dst, i64 %2837
  %2839 = load i8, ptr %2838, align 1, !tbaa !10
  %2840 = getelementptr inbounds nuw i8, ptr @ref, i64 %2837
  %2841 = load i8, ptr %2840, align 1, !tbaa !10
  %2842 = icmp eq i8 %2839, %2841
  br i1 %2842, label %2843, label %2860

2843:
  %2844 = add nuw nsw i64 %2854, 2
  %2845 = getelementptr inbounds nuw i8, ptr @dst, i64 %2844
  %2846 = load i8, ptr %2845, align 1, !tbaa !10
  %2847 = getelementptr inbounds nuw i8, ptr @ref, i64 %2844
  %2848 = load i8, ptr %2847, align 1, !tbaa !10
  %2849 = icmp eq i8 %2846, %2848
  br i1 %2849, label %2850, label %2860

2850:
  %2851 = add nuw nsw i64 %2854, 3
  %2852 = icmp eq i64 %2851, 1200
  br i1 %2852, label %2861, label %2853, !llvm.loop !144

2853:
  %2854 = phi i64 [ 0, %2835 ], [ %2851, %2850 ]
  %2855 = getelementptr inbounds nuw i8, ptr @dst, i64 %2854
  %2856 = load i8, ptr %2855, align 1, !tbaa !10
  %2857 = getelementptr inbounds nuw i8, ptr @ref, i64 %2854
  %2858 = load i8, ptr %2857, align 1, !tbaa !10
  %2859 = icmp eq i8 %2856, %2858
  br i1 %2859, label %2836, label %2860

2860:
  tail call void @report(i64 noundef 17)
  store i1 true, ptr @bad, align 4
  br label %2862

2861:
  br label %2862

2862:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2863

2863:
  %2864 = phi i64 [ 0, %2862 ], [ %2884, %2863 ]
  %2865 = trunc i64 %2864 to i8
  %2866 = mul i8 %2865, 7
  %2867 = or disjoint i8 %2866, 3
  %2868 = getelementptr inbounds nuw i8, ptr @src, i64 %2864
  store i8 %2867, ptr %2868, align 4, !tbaa !10
  %2869 = or disjoint i64 %2864, 1
  %2870 = trunc i64 %2869 to i8
  %2871 = mul i8 %2870, 7
  %2872 = add i8 %2871, 3
  %2873 = getelementptr inbounds nuw i8, ptr @src, i64 %2869
  store i8 %2872, ptr %2873, align 1, !tbaa !10
  %2874 = or disjoint i64 %2864, 2
  %2875 = trunc i64 %2874 to i8
  %2876 = mul i8 %2875, 7
  %2877 = add i8 %2876, 3
  %2878 = getelementptr inbounds nuw i8, ptr @src, i64 %2874
  store i8 %2877, ptr %2878, align 2, !tbaa !10
  %2879 = or disjoint i64 %2864, 3
  %2880 = trunc i64 %2879 to i8
  %2881 = mul i8 %2880, 7
  %2882 = add i8 %2881, 3
  %2883 = getelementptr inbounds nuw i8, ptr @src, i64 %2879
  store i8 %2882, ptr %2883, align 1, !tbaa !10
  %2884 = add nuw nsw i64 %2864, 4
  %2885 = icmp eq i64 %2884, 1200
  br i1 %2885, label %2886, label %2863, !llvm.loop !11

2886:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c717()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 17)
  br label %2904

2887:
  %2888 = add nuw nsw i64 %2905, 1
  %2889 = getelementptr inbounds nuw i8, ptr @dst, i64 %2888
  %2890 = load i8, ptr %2889, align 1, !tbaa !10
  %2891 = getelementptr inbounds nuw i8, ptr @ref, i64 %2888
  %2892 = load i8, ptr %2891, align 1, !tbaa !10
  %2893 = icmp eq i8 %2890, %2892
  br i1 %2893, label %2894, label %2911

2894:
  %2895 = add nuw nsw i64 %2905, 2
  %2896 = getelementptr inbounds nuw i8, ptr @dst, i64 %2895
  %2897 = load i8, ptr %2896, align 1, !tbaa !10
  %2898 = getelementptr inbounds nuw i8, ptr @ref, i64 %2895
  %2899 = load i8, ptr %2898, align 1, !tbaa !10
  %2900 = icmp eq i8 %2897, %2899
  br i1 %2900, label %2901, label %2911

2901:
  %2902 = add nuw nsw i64 %2905, 3
  %2903 = icmp eq i64 %2902, 1200
  br i1 %2903, label %2912, label %2904, !llvm.loop !144

2904:
  %2905 = phi i64 [ 0, %2886 ], [ %2902, %2901 ]
  %2906 = getelementptr inbounds nuw i8, ptr @dst, i64 %2905
  %2907 = load i8, ptr %2906, align 1, !tbaa !10
  %2908 = getelementptr inbounds nuw i8, ptr @ref, i64 %2905
  %2909 = load i8, ptr %2908, align 1, !tbaa !10
  %2910 = icmp eq i8 %2907, %2909
  br i1 %2910, label %2887, label %2911

2911:
  tail call void @report(i64 noundef 17)
  store i1 true, ptr @bad, align 4
  br label %2913

2912:
  br label %2913

2913:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2914

2914:
  %2915 = phi i64 [ 0, %2913 ], [ %2935, %2914 ]
  %2916 = trunc i64 %2915 to i8
  %2917 = mul i8 %2916, 7
  %2918 = or disjoint i8 %2917, 3
  %2919 = getelementptr inbounds nuw i8, ptr @src, i64 %2915
  store i8 %2918, ptr %2919, align 4, !tbaa !10
  %2920 = or disjoint i64 %2915, 1
  %2921 = trunc i64 %2920 to i8
  %2922 = mul i8 %2921, 7
  %2923 = add i8 %2922, 3
  %2924 = getelementptr inbounds nuw i8, ptr @src, i64 %2920
  store i8 %2923, ptr %2924, align 1, !tbaa !10
  %2925 = or disjoint i64 %2915, 2
  %2926 = trunc i64 %2925 to i8
  %2927 = mul i8 %2926, 7
  %2928 = add i8 %2927, 3
  %2929 = getelementptr inbounds nuw i8, ptr @src, i64 %2925
  store i8 %2928, ptr %2929, align 2, !tbaa !10
  %2930 = or disjoint i64 %2915, 3
  %2931 = trunc i64 %2930 to i8
  %2932 = mul i8 %2931, 7
  %2933 = add i8 %2932, 3
  %2934 = getelementptr inbounds nuw i8, ptr @src, i64 %2930
  store i8 %2933, ptr %2934, align 1, !tbaa !10
  %2935 = add nuw nsw i64 %2915, 4
  %2936 = icmp eq i64 %2935, 1200
  br i1 %2936, label %2937, label %2914, !llvm.loop !11

2937:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr17()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 17)
  br label %2955

2938:
  %2939 = add nuw nsw i64 %2956, 1
  %2940 = getelementptr inbounds nuw i8, ptr @dst, i64 %2939
  %2941 = load i8, ptr %2940, align 1, !tbaa !10
  %2942 = getelementptr inbounds nuw i8, ptr @ref, i64 %2939
  %2943 = load i8, ptr %2942, align 1, !tbaa !10
  %2944 = icmp eq i8 %2941, %2943
  br i1 %2944, label %2945, label %2962

2945:
  %2946 = add nuw nsw i64 %2956, 2
  %2947 = getelementptr inbounds nuw i8, ptr @dst, i64 %2946
  %2948 = load i8, ptr %2947, align 1, !tbaa !10
  %2949 = getelementptr inbounds nuw i8, ptr @ref, i64 %2946
  %2950 = load i8, ptr %2949, align 1, !tbaa !10
  %2951 = icmp eq i8 %2948, %2950
  br i1 %2951, label %2952, label %2962

2952:
  %2953 = add nuw nsw i64 %2956, 3
  %2954 = icmp eq i64 %2953, 1200
  br i1 %2954, label %2963, label %2955, !llvm.loop !144

2955:
  %2956 = phi i64 [ 0, %2937 ], [ %2953, %2952 ]
  %2957 = getelementptr inbounds nuw i8, ptr @dst, i64 %2956
  %2958 = load i8, ptr %2957, align 1, !tbaa !10
  %2959 = getelementptr inbounds nuw i8, ptr @ref, i64 %2956
  %2960 = load i8, ptr %2959, align 1, !tbaa !10
  %2961 = icmp eq i8 %2958, %2960
  br i1 %2961, label %2938, label %2962

2962:
  tail call void @report(i64 noundef 17)
  store i1 true, ptr @bad, align 4
  br label %2964

2963:
  br label %2964

2964:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %2965

2965:
  %2966 = phi i64 [ 0, %2964 ], [ %2986, %2965 ]
  %2967 = trunc i64 %2966 to i8
  %2968 = mul i8 %2967, 7
  %2969 = or disjoint i8 %2968, 3
  %2970 = getelementptr inbounds nuw i8, ptr @src, i64 %2966
  store i8 %2969, ptr %2970, align 4, !tbaa !10
  %2971 = or disjoint i64 %2966, 1
  %2972 = trunc i64 %2971 to i8
  %2973 = mul i8 %2972, 7
  %2974 = add i8 %2973, 3
  %2975 = getelementptr inbounds nuw i8, ptr @src, i64 %2971
  store i8 %2974, ptr %2975, align 1, !tbaa !10
  %2976 = or disjoint i64 %2966, 2
  %2977 = trunc i64 %2976 to i8
  %2978 = mul i8 %2977, 7
  %2979 = add i8 %2978, 3
  %2980 = getelementptr inbounds nuw i8, ptr @src, i64 %2976
  store i8 %2979, ptr %2980, align 2, !tbaa !10
  %2981 = or disjoint i64 %2966, 3
  %2982 = trunc i64 %2981 to i8
  %2983 = mul i8 %2982, 7
  %2984 = add i8 %2983, 3
  %2985 = getelementptr inbounds nuw i8, ptr @src, i64 %2981
  store i8 %2984, ptr %2985, align 1, !tbaa !10
  %2986 = add nuw nsw i64 %2966, 4
  %2987 = icmp eq i64 %2986, 1200
  br i1 %2987, label %2988, label %2965, !llvm.loop !11

2988:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv17(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 17)
  br label %3006

2989:
  %2990 = add nuw nsw i64 %3007, 1
  %2991 = getelementptr inbounds nuw i8, ptr @dst, i64 %2990
  %2992 = load i8, ptr %2991, align 1, !tbaa !10
  %2993 = getelementptr inbounds nuw i8, ptr @ref, i64 %2990
  %2994 = load i8, ptr %2993, align 1, !tbaa !10
  %2995 = icmp eq i8 %2992, %2994
  br i1 %2995, label %2996, label %3013

2996:
  %2997 = add nuw nsw i64 %3007, 2
  %2998 = getelementptr inbounds nuw i8, ptr @dst, i64 %2997
  %2999 = load i8, ptr %2998, align 1, !tbaa !10
  %3000 = getelementptr inbounds nuw i8, ptr @ref, i64 %2997
  %3001 = load i8, ptr %3000, align 1, !tbaa !10
  %3002 = icmp eq i8 %2999, %3001
  br i1 %3002, label %3003, label %3013

3003:
  %3004 = add nuw nsw i64 %3007, 3
  %3005 = icmp eq i64 %3004, 1200
  br i1 %3005, label %3014, label %3006, !llvm.loop !144

3006:
  %3007 = phi i64 [ 0, %2988 ], [ %3004, %3003 ]
  %3008 = getelementptr inbounds nuw i8, ptr @dst, i64 %3007
  %3009 = load i8, ptr %3008, align 1, !tbaa !10
  %3010 = getelementptr inbounds nuw i8, ptr @ref, i64 %3007
  %3011 = load i8, ptr %3010, align 1, !tbaa !10
  %3012 = icmp eq i8 %3009, %3011
  br i1 %3012, label %2989, label %3013

3013:
  tail call void @report(i64 noundef 17)
  store i1 true, ptr @bad, align 4
  br label %3015

3014:
  br label %3015

3015:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3016

3016:
  %3017 = phi i64 [ 0, %3015 ], [ %3037, %3016 ]
  %3018 = trunc i64 %3017 to i8
  %3019 = mul i8 %3018, 7
  %3020 = or disjoint i8 %3019, 3
  %3021 = getelementptr inbounds nuw i8, ptr @src, i64 %3017
  store i8 %3020, ptr %3021, align 4, !tbaa !10
  %3022 = or disjoint i64 %3017, 1
  %3023 = trunc i64 %3022 to i8
  %3024 = mul i8 %3023, 7
  %3025 = add i8 %3024, 3
  %3026 = getelementptr inbounds nuw i8, ptr @src, i64 %3022
  store i8 %3025, ptr %3026, align 1, !tbaa !10
  %3027 = or disjoint i64 %3017, 2
  %3028 = trunc i64 %3027 to i8
  %3029 = mul i8 %3028, 7
  %3030 = add i8 %3029, 3
  %3031 = getelementptr inbounds nuw i8, ptr @src, i64 %3027
  store i8 %3030, ptr %3031, align 2, !tbaa !10
  %3032 = or disjoint i64 %3017, 3
  %3033 = trunc i64 %3032 to i8
  %3034 = mul i8 %3033, 7
  %3035 = add i8 %3034, 3
  %3036 = getelementptr inbounds nuw i8, ptr @src, i64 %3032
  store i8 %3035, ptr %3036, align 1, !tbaa !10
  %3037 = add nuw nsw i64 %3017, 4
  %3038 = icmp eq i64 %3037, 1200
  br i1 %3038, label %3039, label %3016, !llvm.loop !11

3039:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv17(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 17)
  br label %3057

3040:
  %3041 = add nuw nsw i64 %3058, 1
  %3042 = getelementptr inbounds nuw i8, ptr @dst, i64 %3041
  %3043 = load i8, ptr %3042, align 1, !tbaa !10
  %3044 = getelementptr inbounds nuw i8, ptr @ref, i64 %3041
  %3045 = load i8, ptr %3044, align 1, !tbaa !10
  %3046 = icmp eq i8 %3043, %3045
  br i1 %3046, label %3047, label %3064

3047:
  %3048 = add nuw nsw i64 %3058, 2
  %3049 = getelementptr inbounds nuw i8, ptr @dst, i64 %3048
  %3050 = load i8, ptr %3049, align 1, !tbaa !10
  %3051 = getelementptr inbounds nuw i8, ptr @ref, i64 %3048
  %3052 = load i8, ptr %3051, align 1, !tbaa !10
  %3053 = icmp eq i8 %3050, %3052
  br i1 %3053, label %3054, label %3064

3054:
  %3055 = add nuw nsw i64 %3058, 3
  %3056 = icmp eq i64 %3055, 1200
  br i1 %3056, label %3065, label %3057, !llvm.loop !144

3057:
  %3058 = phi i64 [ 0, %3039 ], [ %3055, %3054 ]
  %3059 = getelementptr inbounds nuw i8, ptr @dst, i64 %3058
  %3060 = load i8, ptr %3059, align 1, !tbaa !10
  %3061 = getelementptr inbounds nuw i8, ptr @ref, i64 %3058
  %3062 = load i8, ptr %3061, align 1, !tbaa !10
  %3063 = icmp eq i8 %3060, %3062
  br i1 %3063, label %3040, label %3064

3064:
  tail call void @report(i64 noundef 17)
  store i1 true, ptr @bad, align 4
  br label %3066

3065:
  br label %3066

3066:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3067

3067:
  %3068 = phi i64 [ 0, %3066 ], [ %3088, %3067 ]
  %3069 = trunc i64 %3068 to i8
  %3070 = mul i8 %3069, 7
  %3071 = or disjoint i8 %3070, 3
  %3072 = getelementptr inbounds nuw i8, ptr @src, i64 %3068
  store i8 %3071, ptr %3072, align 4, !tbaa !10
  %3073 = or disjoint i64 %3068, 1
  %3074 = trunc i64 %3073 to i8
  %3075 = mul i8 %3074, 7
  %3076 = add i8 %3075, 3
  %3077 = getelementptr inbounds nuw i8, ptr @src, i64 %3073
  store i8 %3076, ptr %3077, align 1, !tbaa !10
  %3078 = or disjoint i64 %3068, 2
  %3079 = trunc i64 %3078 to i8
  %3080 = mul i8 %3079, 7
  %3081 = add i8 %3080, 3
  %3082 = getelementptr inbounds nuw i8, ptr @src, i64 %3078
  store i8 %3081, ptr %3082, align 2, !tbaa !10
  %3083 = or disjoint i64 %3068, 3
  %3084 = trunc i64 %3083 to i8
  %3085 = mul i8 %3084, 7
  %3086 = add i8 %3085, 3
  %3087 = getelementptr inbounds nuw i8, ptr @src, i64 %3083
  store i8 %3086, ptr %3087, align 1, !tbaa !10
  %3088 = add nuw nsw i64 %3068, 4
  %3089 = icmp eq i64 %3088, 1200
  br i1 %3089, label %3090, label %3067, !llvm.loop !11

3090:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp24()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 24)
  br label %3108

3091:
  %3092 = add nuw nsw i64 %3109, 1
  %3093 = getelementptr inbounds nuw i8, ptr @dst, i64 %3092
  %3094 = load i8, ptr %3093, align 1, !tbaa !10
  %3095 = getelementptr inbounds nuw i8, ptr @ref, i64 %3092
  %3096 = load i8, ptr %3095, align 1, !tbaa !10
  %3097 = icmp eq i8 %3094, %3096
  br i1 %3097, label %3098, label %3115

3098:
  %3099 = add nuw nsw i64 %3109, 2
  %3100 = getelementptr inbounds nuw i8, ptr @dst, i64 %3099
  %3101 = load i8, ptr %3100, align 1, !tbaa !10
  %3102 = getelementptr inbounds nuw i8, ptr @ref, i64 %3099
  %3103 = load i8, ptr %3102, align 1, !tbaa !10
  %3104 = icmp eq i8 %3101, %3103
  br i1 %3104, label %3105, label %3115

3105:
  %3106 = add nuw nsw i64 %3109, 3
  %3107 = icmp eq i64 %3106, 1200
  br i1 %3107, label %3116, label %3108, !llvm.loop !144

3108:
  %3109 = phi i64 [ 0, %3090 ], [ %3106, %3105 ]
  %3110 = getelementptr inbounds nuw i8, ptr @dst, i64 %3109
  %3111 = load i8, ptr %3110, align 1, !tbaa !10
  %3112 = getelementptr inbounds nuw i8, ptr @ref, i64 %3109
  %3113 = load i8, ptr %3112, align 1, !tbaa !10
  %3114 = icmp eq i8 %3111, %3113
  br i1 %3114, label %3091, label %3115

3115:
  tail call void @report(i64 noundef 24)
  store i1 true, ptr @bad, align 4
  br label %3117

3116:
  br label %3117

3117:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3118

3118:
  %3119 = phi i64 [ 0, %3117 ], [ %3139, %3118 ]
  %3120 = trunc i64 %3119 to i8
  %3121 = mul i8 %3120, 7
  %3122 = or disjoint i8 %3121, 3
  %3123 = getelementptr inbounds nuw i8, ptr @src, i64 %3119
  store i8 %3122, ptr %3123, align 4, !tbaa !10
  %3124 = or disjoint i64 %3119, 1
  %3125 = trunc i64 %3124 to i8
  %3126 = mul i8 %3125, 7
  %3127 = add i8 %3126, 3
  %3128 = getelementptr inbounds nuw i8, ptr @src, i64 %3124
  store i8 %3127, ptr %3128, align 1, !tbaa !10
  %3129 = or disjoint i64 %3119, 2
  %3130 = trunc i64 %3129 to i8
  %3131 = mul i8 %3130, 7
  %3132 = add i8 %3131, 3
  %3133 = getelementptr inbounds nuw i8, ptr @src, i64 %3129
  store i8 %3132, ptr %3133, align 2, !tbaa !10
  %3134 = or disjoint i64 %3119, 3
  %3135 = trunc i64 %3134 to i8
  %3136 = mul i8 %3135, 7
  %3137 = add i8 %3136, 3
  %3138 = getelementptr inbounds nuw i8, ptr @src, i64 %3134
  store i8 %3137, ptr %3138, align 1, !tbaa !10
  %3139 = add nuw nsw i64 %3119, 4
  %3140 = icmp eq i64 %3139, 1200
  br i1 %3140, label %3141, label %3118, !llvm.loop !11

3141:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr24()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 24)
  br label %3159

3142:
  %3143 = add nuw nsw i64 %3160, 1
  %3144 = getelementptr inbounds nuw i8, ptr @dst, i64 %3143
  %3145 = load i8, ptr %3144, align 1, !tbaa !10
  %3146 = getelementptr inbounds nuw i8, ptr @ref, i64 %3143
  %3147 = load i8, ptr %3146, align 1, !tbaa !10
  %3148 = icmp eq i8 %3145, %3147
  br i1 %3148, label %3149, label %3166

3149:
  %3150 = add nuw nsw i64 %3160, 2
  %3151 = getelementptr inbounds nuw i8, ptr @dst, i64 %3150
  %3152 = load i8, ptr %3151, align 1, !tbaa !10
  %3153 = getelementptr inbounds nuw i8, ptr @ref, i64 %3150
  %3154 = load i8, ptr %3153, align 1, !tbaa !10
  %3155 = icmp eq i8 %3152, %3154
  br i1 %3155, label %3156, label %3166

3156:
  %3157 = add nuw nsw i64 %3160, 3
  %3158 = icmp eq i64 %3157, 1200
  br i1 %3158, label %3167, label %3159, !llvm.loop !144

3159:
  %3160 = phi i64 [ 0, %3141 ], [ %3157, %3156 ]
  %3161 = getelementptr inbounds nuw i8, ptr @dst, i64 %3160
  %3162 = load i8, ptr %3161, align 1, !tbaa !10
  %3163 = getelementptr inbounds nuw i8, ptr @ref, i64 %3160
  %3164 = load i8, ptr %3163, align 1, !tbaa !10
  %3165 = icmp eq i8 %3162, %3164
  br i1 %3165, label %3142, label %3166

3166:
  tail call void @report(i64 noundef 24)
  store i1 true, ptr @bad, align 4
  br label %3168

3167:
  br label %3168

3168:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3169

3169:
  %3170 = phi i64 [ 0, %3168 ], [ %3190, %3169 ]
  %3171 = trunc i64 %3170 to i8
  %3172 = mul i8 %3171, 7
  %3173 = or disjoint i8 %3172, 3
  %3174 = getelementptr inbounds nuw i8, ptr @src, i64 %3170
  store i8 %3173, ptr %3174, align 4, !tbaa !10
  %3175 = or disjoint i64 %3170, 1
  %3176 = trunc i64 %3175 to i8
  %3177 = mul i8 %3176, 7
  %3178 = add i8 %3177, 3
  %3179 = getelementptr inbounds nuw i8, ptr @src, i64 %3175
  store i8 %3178, ptr %3179, align 1, !tbaa !10
  %3180 = or disjoint i64 %3170, 2
  %3181 = trunc i64 %3180 to i8
  %3182 = mul i8 %3181, 7
  %3183 = add i8 %3182, 3
  %3184 = getelementptr inbounds nuw i8, ptr @src, i64 %3180
  store i8 %3183, ptr %3184, align 2, !tbaa !10
  %3185 = or disjoint i64 %3170, 3
  %3186 = trunc i64 %3185 to i8
  %3187 = mul i8 %3186, 7
  %3188 = add i8 %3187, 3
  %3189 = getelementptr inbounds nuw i8, ptr @src, i64 %3185
  store i8 %3188, ptr %3189, align 1, !tbaa !10
  %3190 = add nuw nsw i64 %3170, 4
  %3191 = icmp eq i64 %3190, 1200
  br i1 %3191, label %3192, label %3169, !llvm.loop !11

3192:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c724()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 24)
  br label %3210

3193:
  %3194 = add nuw nsw i64 %3211, 1
  %3195 = getelementptr inbounds nuw i8, ptr @dst, i64 %3194
  %3196 = load i8, ptr %3195, align 1, !tbaa !10
  %3197 = getelementptr inbounds nuw i8, ptr @ref, i64 %3194
  %3198 = load i8, ptr %3197, align 1, !tbaa !10
  %3199 = icmp eq i8 %3196, %3198
  br i1 %3199, label %3200, label %3217

3200:
  %3201 = add nuw nsw i64 %3211, 2
  %3202 = getelementptr inbounds nuw i8, ptr @dst, i64 %3201
  %3203 = load i8, ptr %3202, align 1, !tbaa !10
  %3204 = getelementptr inbounds nuw i8, ptr @ref, i64 %3201
  %3205 = load i8, ptr %3204, align 1, !tbaa !10
  %3206 = icmp eq i8 %3203, %3205
  br i1 %3206, label %3207, label %3217

3207:
  %3208 = add nuw nsw i64 %3211, 3
  %3209 = icmp eq i64 %3208, 1200
  br i1 %3209, label %3218, label %3210, !llvm.loop !144

3210:
  %3211 = phi i64 [ 0, %3192 ], [ %3208, %3207 ]
  %3212 = getelementptr inbounds nuw i8, ptr @dst, i64 %3211
  %3213 = load i8, ptr %3212, align 1, !tbaa !10
  %3214 = getelementptr inbounds nuw i8, ptr @ref, i64 %3211
  %3215 = load i8, ptr %3214, align 1, !tbaa !10
  %3216 = icmp eq i8 %3213, %3215
  br i1 %3216, label %3193, label %3217

3217:
  tail call void @report(i64 noundef 24)
  store i1 true, ptr @bad, align 4
  br label %3219

3218:
  br label %3219

3219:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3220

3220:
  %3221 = phi i64 [ 0, %3219 ], [ %3241, %3220 ]
  %3222 = trunc i64 %3221 to i8
  %3223 = mul i8 %3222, 7
  %3224 = or disjoint i8 %3223, 3
  %3225 = getelementptr inbounds nuw i8, ptr @src, i64 %3221
  store i8 %3224, ptr %3225, align 4, !tbaa !10
  %3226 = or disjoint i64 %3221, 1
  %3227 = trunc i64 %3226 to i8
  %3228 = mul i8 %3227, 7
  %3229 = add i8 %3228, 3
  %3230 = getelementptr inbounds nuw i8, ptr @src, i64 %3226
  store i8 %3229, ptr %3230, align 1, !tbaa !10
  %3231 = or disjoint i64 %3221, 2
  %3232 = trunc i64 %3231 to i8
  %3233 = mul i8 %3232, 7
  %3234 = add i8 %3233, 3
  %3235 = getelementptr inbounds nuw i8, ptr @src, i64 %3231
  store i8 %3234, ptr %3235, align 2, !tbaa !10
  %3236 = or disjoint i64 %3221, 3
  %3237 = trunc i64 %3236 to i8
  %3238 = mul i8 %3237, 7
  %3239 = add i8 %3238, 3
  %3240 = getelementptr inbounds nuw i8, ptr @src, i64 %3236
  store i8 %3239, ptr %3240, align 1, !tbaa !10
  %3241 = add nuw nsw i64 %3221, 4
  %3242 = icmp eq i64 %3241, 1200
  br i1 %3242, label %3243, label %3220, !llvm.loop !11

3243:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr24()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 24)
  br label %3261

3244:
  %3245 = add nuw nsw i64 %3262, 1
  %3246 = getelementptr inbounds nuw i8, ptr @dst, i64 %3245
  %3247 = load i8, ptr %3246, align 1, !tbaa !10
  %3248 = getelementptr inbounds nuw i8, ptr @ref, i64 %3245
  %3249 = load i8, ptr %3248, align 1, !tbaa !10
  %3250 = icmp eq i8 %3247, %3249
  br i1 %3250, label %3251, label %3268

3251:
  %3252 = add nuw nsw i64 %3262, 2
  %3253 = getelementptr inbounds nuw i8, ptr @dst, i64 %3252
  %3254 = load i8, ptr %3253, align 1, !tbaa !10
  %3255 = getelementptr inbounds nuw i8, ptr @ref, i64 %3252
  %3256 = load i8, ptr %3255, align 1, !tbaa !10
  %3257 = icmp eq i8 %3254, %3256
  br i1 %3257, label %3258, label %3268

3258:
  %3259 = add nuw nsw i64 %3262, 3
  %3260 = icmp eq i64 %3259, 1200
  br i1 %3260, label %3269, label %3261, !llvm.loop !144

3261:
  %3262 = phi i64 [ 0, %3243 ], [ %3259, %3258 ]
  %3263 = getelementptr inbounds nuw i8, ptr @dst, i64 %3262
  %3264 = load i8, ptr %3263, align 1, !tbaa !10
  %3265 = getelementptr inbounds nuw i8, ptr @ref, i64 %3262
  %3266 = load i8, ptr %3265, align 1, !tbaa !10
  %3267 = icmp eq i8 %3264, %3266
  br i1 %3267, label %3244, label %3268

3268:
  tail call void @report(i64 noundef 24)
  store i1 true, ptr @bad, align 4
  br label %3270

3269:
  br label %3270

3270:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3271

3271:
  %3272 = phi i64 [ 0, %3270 ], [ %3292, %3271 ]
  %3273 = trunc i64 %3272 to i8
  %3274 = mul i8 %3273, 7
  %3275 = or disjoint i8 %3274, 3
  %3276 = getelementptr inbounds nuw i8, ptr @src, i64 %3272
  store i8 %3275, ptr %3276, align 4, !tbaa !10
  %3277 = or disjoint i64 %3272, 1
  %3278 = trunc i64 %3277 to i8
  %3279 = mul i8 %3278, 7
  %3280 = add i8 %3279, 3
  %3281 = getelementptr inbounds nuw i8, ptr @src, i64 %3277
  store i8 %3280, ptr %3281, align 1, !tbaa !10
  %3282 = or disjoint i64 %3272, 2
  %3283 = trunc i64 %3282 to i8
  %3284 = mul i8 %3283, 7
  %3285 = add i8 %3284, 3
  %3286 = getelementptr inbounds nuw i8, ptr @src, i64 %3282
  store i8 %3285, ptr %3286, align 2, !tbaa !10
  %3287 = or disjoint i64 %3272, 3
  %3288 = trunc i64 %3287 to i8
  %3289 = mul i8 %3288, 7
  %3290 = add i8 %3289, 3
  %3291 = getelementptr inbounds nuw i8, ptr @src, i64 %3287
  store i8 %3290, ptr %3291, align 1, !tbaa !10
  %3292 = add nuw nsw i64 %3272, 4
  %3293 = icmp eq i64 %3292, 1200
  br i1 %3293, label %3294, label %3271, !llvm.loop !11

3294:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv24(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 24)
  br label %3312

3295:
  %3296 = add nuw nsw i64 %3313, 1
  %3297 = getelementptr inbounds nuw i8, ptr @dst, i64 %3296
  %3298 = load i8, ptr %3297, align 1, !tbaa !10
  %3299 = getelementptr inbounds nuw i8, ptr @ref, i64 %3296
  %3300 = load i8, ptr %3299, align 1, !tbaa !10
  %3301 = icmp eq i8 %3298, %3300
  br i1 %3301, label %3302, label %3319

3302:
  %3303 = add nuw nsw i64 %3313, 2
  %3304 = getelementptr inbounds nuw i8, ptr @dst, i64 %3303
  %3305 = load i8, ptr %3304, align 1, !tbaa !10
  %3306 = getelementptr inbounds nuw i8, ptr @ref, i64 %3303
  %3307 = load i8, ptr %3306, align 1, !tbaa !10
  %3308 = icmp eq i8 %3305, %3307
  br i1 %3308, label %3309, label %3319

3309:
  %3310 = add nuw nsw i64 %3313, 3
  %3311 = icmp eq i64 %3310, 1200
  br i1 %3311, label %3320, label %3312, !llvm.loop !144

3312:
  %3313 = phi i64 [ 0, %3294 ], [ %3310, %3309 ]
  %3314 = getelementptr inbounds nuw i8, ptr @dst, i64 %3313
  %3315 = load i8, ptr %3314, align 1, !tbaa !10
  %3316 = getelementptr inbounds nuw i8, ptr @ref, i64 %3313
  %3317 = load i8, ptr %3316, align 1, !tbaa !10
  %3318 = icmp eq i8 %3315, %3317
  br i1 %3318, label %3295, label %3319

3319:
  tail call void @report(i64 noundef 24)
  store i1 true, ptr @bad, align 4
  br label %3321

3320:
  br label %3321

3321:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3322

3322:
  %3323 = phi i64 [ 0, %3321 ], [ %3343, %3322 ]
  %3324 = trunc i64 %3323 to i8
  %3325 = mul i8 %3324, 7
  %3326 = or disjoint i8 %3325, 3
  %3327 = getelementptr inbounds nuw i8, ptr @src, i64 %3323
  store i8 %3326, ptr %3327, align 4, !tbaa !10
  %3328 = or disjoint i64 %3323, 1
  %3329 = trunc i64 %3328 to i8
  %3330 = mul i8 %3329, 7
  %3331 = add i8 %3330, 3
  %3332 = getelementptr inbounds nuw i8, ptr @src, i64 %3328
  store i8 %3331, ptr %3332, align 1, !tbaa !10
  %3333 = or disjoint i64 %3323, 2
  %3334 = trunc i64 %3333 to i8
  %3335 = mul i8 %3334, 7
  %3336 = add i8 %3335, 3
  %3337 = getelementptr inbounds nuw i8, ptr @src, i64 %3333
  store i8 %3336, ptr %3337, align 2, !tbaa !10
  %3338 = or disjoint i64 %3323, 3
  %3339 = trunc i64 %3338 to i8
  %3340 = mul i8 %3339, 7
  %3341 = add i8 %3340, 3
  %3342 = getelementptr inbounds nuw i8, ptr @src, i64 %3338
  store i8 %3341, ptr %3342, align 1, !tbaa !10
  %3343 = add nuw nsw i64 %3323, 4
  %3344 = icmp eq i64 %3343, 1200
  br i1 %3344, label %3345, label %3322, !llvm.loop !11

3345:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv24(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 24)
  br label %3363

3346:
  %3347 = add nuw nsw i64 %3364, 1
  %3348 = getelementptr inbounds nuw i8, ptr @dst, i64 %3347
  %3349 = load i8, ptr %3348, align 1, !tbaa !10
  %3350 = getelementptr inbounds nuw i8, ptr @ref, i64 %3347
  %3351 = load i8, ptr %3350, align 1, !tbaa !10
  %3352 = icmp eq i8 %3349, %3351
  br i1 %3352, label %3353, label %3370

3353:
  %3354 = add nuw nsw i64 %3364, 2
  %3355 = getelementptr inbounds nuw i8, ptr @dst, i64 %3354
  %3356 = load i8, ptr %3355, align 1, !tbaa !10
  %3357 = getelementptr inbounds nuw i8, ptr @ref, i64 %3354
  %3358 = load i8, ptr %3357, align 1, !tbaa !10
  %3359 = icmp eq i8 %3356, %3358
  br i1 %3359, label %3360, label %3370

3360:
  %3361 = add nuw nsw i64 %3364, 3
  %3362 = icmp eq i64 %3361, 1200
  br i1 %3362, label %3371, label %3363, !llvm.loop !144

3363:
  %3364 = phi i64 [ 0, %3345 ], [ %3361, %3360 ]
  %3365 = getelementptr inbounds nuw i8, ptr @dst, i64 %3364
  %3366 = load i8, ptr %3365, align 1, !tbaa !10
  %3367 = getelementptr inbounds nuw i8, ptr @ref, i64 %3364
  %3368 = load i8, ptr %3367, align 1, !tbaa !10
  %3369 = icmp eq i8 %3366, %3368
  br i1 %3369, label %3346, label %3370

3370:
  tail call void @report(i64 noundef 24)
  store i1 true, ptr @bad, align 4
  br label %3372

3371:
  br label %3372

3372:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3373

3373:
  %3374 = phi i64 [ 0, %3372 ], [ %3394, %3373 ]
  %3375 = trunc i64 %3374 to i8
  %3376 = mul i8 %3375, 7
  %3377 = or disjoint i8 %3376, 3
  %3378 = getelementptr inbounds nuw i8, ptr @src, i64 %3374
  store i8 %3377, ptr %3378, align 4, !tbaa !10
  %3379 = or disjoint i64 %3374, 1
  %3380 = trunc i64 %3379 to i8
  %3381 = mul i8 %3380, 7
  %3382 = add i8 %3381, 3
  %3383 = getelementptr inbounds nuw i8, ptr @src, i64 %3379
  store i8 %3382, ptr %3383, align 1, !tbaa !10
  %3384 = or disjoint i64 %3374, 2
  %3385 = trunc i64 %3384 to i8
  %3386 = mul i8 %3385, 7
  %3387 = add i8 %3386, 3
  %3388 = getelementptr inbounds nuw i8, ptr @src, i64 %3384
  store i8 %3387, ptr %3388, align 2, !tbaa !10
  %3389 = or disjoint i64 %3374, 3
  %3390 = trunc i64 %3389 to i8
  %3391 = mul i8 %3390, 7
  %3392 = add i8 %3391, 3
  %3393 = getelementptr inbounds nuw i8, ptr @src, i64 %3389
  store i8 %3392, ptr %3393, align 1, !tbaa !10
  %3394 = add nuw nsw i64 %3374, 4
  %3395 = icmp eq i64 %3394, 1200
  br i1 %3395, label %3396, label %3373, !llvm.loop !11

3396:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp31()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 31)
  br label %3414

3397:
  %3398 = add nuw nsw i64 %3415, 1
  %3399 = getelementptr inbounds nuw i8, ptr @dst, i64 %3398
  %3400 = load i8, ptr %3399, align 1, !tbaa !10
  %3401 = getelementptr inbounds nuw i8, ptr @ref, i64 %3398
  %3402 = load i8, ptr %3401, align 1, !tbaa !10
  %3403 = icmp eq i8 %3400, %3402
  br i1 %3403, label %3404, label %3421

3404:
  %3405 = add nuw nsw i64 %3415, 2
  %3406 = getelementptr inbounds nuw i8, ptr @dst, i64 %3405
  %3407 = load i8, ptr %3406, align 1, !tbaa !10
  %3408 = getelementptr inbounds nuw i8, ptr @ref, i64 %3405
  %3409 = load i8, ptr %3408, align 1, !tbaa !10
  %3410 = icmp eq i8 %3407, %3409
  br i1 %3410, label %3411, label %3421

3411:
  %3412 = add nuw nsw i64 %3415, 3
  %3413 = icmp eq i64 %3412, 1200
  br i1 %3413, label %3422, label %3414, !llvm.loop !144

3414:
  %3415 = phi i64 [ 0, %3396 ], [ %3412, %3411 ]
  %3416 = getelementptr inbounds nuw i8, ptr @dst, i64 %3415
  %3417 = load i8, ptr %3416, align 1, !tbaa !10
  %3418 = getelementptr inbounds nuw i8, ptr @ref, i64 %3415
  %3419 = load i8, ptr %3418, align 1, !tbaa !10
  %3420 = icmp eq i8 %3417, %3419
  br i1 %3420, label %3397, label %3421

3421:
  tail call void @report(i64 noundef 31)
  store i1 true, ptr @bad, align 4
  br label %3423

3422:
  br label %3423

3423:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3424

3424:
  %3425 = phi i64 [ 0, %3423 ], [ %3445, %3424 ]
  %3426 = trunc i64 %3425 to i8
  %3427 = mul i8 %3426, 7
  %3428 = or disjoint i8 %3427, 3
  %3429 = getelementptr inbounds nuw i8, ptr @src, i64 %3425
  store i8 %3428, ptr %3429, align 4, !tbaa !10
  %3430 = or disjoint i64 %3425, 1
  %3431 = trunc i64 %3430 to i8
  %3432 = mul i8 %3431, 7
  %3433 = add i8 %3432, 3
  %3434 = getelementptr inbounds nuw i8, ptr @src, i64 %3430
  store i8 %3433, ptr %3434, align 1, !tbaa !10
  %3435 = or disjoint i64 %3425, 2
  %3436 = trunc i64 %3435 to i8
  %3437 = mul i8 %3436, 7
  %3438 = add i8 %3437, 3
  %3439 = getelementptr inbounds nuw i8, ptr @src, i64 %3435
  store i8 %3438, ptr %3439, align 2, !tbaa !10
  %3440 = or disjoint i64 %3425, 3
  %3441 = trunc i64 %3440 to i8
  %3442 = mul i8 %3441, 7
  %3443 = add i8 %3442, 3
  %3444 = getelementptr inbounds nuw i8, ptr @src, i64 %3440
  store i8 %3443, ptr %3444, align 1, !tbaa !10
  %3445 = add nuw nsw i64 %3425, 4
  %3446 = icmp eq i64 %3445, 1200
  br i1 %3446, label %3447, label %3424, !llvm.loop !11

3447:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr31()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 31)
  br label %3465

3448:
  %3449 = add nuw nsw i64 %3466, 1
  %3450 = getelementptr inbounds nuw i8, ptr @dst, i64 %3449
  %3451 = load i8, ptr %3450, align 1, !tbaa !10
  %3452 = getelementptr inbounds nuw i8, ptr @ref, i64 %3449
  %3453 = load i8, ptr %3452, align 1, !tbaa !10
  %3454 = icmp eq i8 %3451, %3453
  br i1 %3454, label %3455, label %3472

3455:
  %3456 = add nuw nsw i64 %3466, 2
  %3457 = getelementptr inbounds nuw i8, ptr @dst, i64 %3456
  %3458 = load i8, ptr %3457, align 1, !tbaa !10
  %3459 = getelementptr inbounds nuw i8, ptr @ref, i64 %3456
  %3460 = load i8, ptr %3459, align 1, !tbaa !10
  %3461 = icmp eq i8 %3458, %3460
  br i1 %3461, label %3462, label %3472

3462:
  %3463 = add nuw nsw i64 %3466, 3
  %3464 = icmp eq i64 %3463, 1200
  br i1 %3464, label %3473, label %3465, !llvm.loop !144

3465:
  %3466 = phi i64 [ 0, %3447 ], [ %3463, %3462 ]
  %3467 = getelementptr inbounds nuw i8, ptr @dst, i64 %3466
  %3468 = load i8, ptr %3467, align 1, !tbaa !10
  %3469 = getelementptr inbounds nuw i8, ptr @ref, i64 %3466
  %3470 = load i8, ptr %3469, align 1, !tbaa !10
  %3471 = icmp eq i8 %3468, %3470
  br i1 %3471, label %3448, label %3472

3472:
  tail call void @report(i64 noundef 31)
  store i1 true, ptr @bad, align 4
  br label %3474

3473:
  br label %3474

3474:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3475

3475:
  %3476 = phi i64 [ 0, %3474 ], [ %3496, %3475 ]
  %3477 = trunc i64 %3476 to i8
  %3478 = mul i8 %3477, 7
  %3479 = or disjoint i8 %3478, 3
  %3480 = getelementptr inbounds nuw i8, ptr @src, i64 %3476
  store i8 %3479, ptr %3480, align 4, !tbaa !10
  %3481 = or disjoint i64 %3476, 1
  %3482 = trunc i64 %3481 to i8
  %3483 = mul i8 %3482, 7
  %3484 = add i8 %3483, 3
  %3485 = getelementptr inbounds nuw i8, ptr @src, i64 %3481
  store i8 %3484, ptr %3485, align 1, !tbaa !10
  %3486 = or disjoint i64 %3476, 2
  %3487 = trunc i64 %3486 to i8
  %3488 = mul i8 %3487, 7
  %3489 = add i8 %3488, 3
  %3490 = getelementptr inbounds nuw i8, ptr @src, i64 %3486
  store i8 %3489, ptr %3490, align 2, !tbaa !10
  %3491 = or disjoint i64 %3476, 3
  %3492 = trunc i64 %3491 to i8
  %3493 = mul i8 %3492, 7
  %3494 = add i8 %3493, 3
  %3495 = getelementptr inbounds nuw i8, ptr @src, i64 %3491
  store i8 %3494, ptr %3495, align 1, !tbaa !10
  %3496 = add nuw nsw i64 %3476, 4
  %3497 = icmp eq i64 %3496, 1200
  br i1 %3497, label %3498, label %3475, !llvm.loop !11

3498:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c731()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 31)
  br label %3516

3499:
  %3500 = add nuw nsw i64 %3517, 1
  %3501 = getelementptr inbounds nuw i8, ptr @dst, i64 %3500
  %3502 = load i8, ptr %3501, align 1, !tbaa !10
  %3503 = getelementptr inbounds nuw i8, ptr @ref, i64 %3500
  %3504 = load i8, ptr %3503, align 1, !tbaa !10
  %3505 = icmp eq i8 %3502, %3504
  br i1 %3505, label %3506, label %3523

3506:
  %3507 = add nuw nsw i64 %3517, 2
  %3508 = getelementptr inbounds nuw i8, ptr @dst, i64 %3507
  %3509 = load i8, ptr %3508, align 1, !tbaa !10
  %3510 = getelementptr inbounds nuw i8, ptr @ref, i64 %3507
  %3511 = load i8, ptr %3510, align 1, !tbaa !10
  %3512 = icmp eq i8 %3509, %3511
  br i1 %3512, label %3513, label %3523

3513:
  %3514 = add nuw nsw i64 %3517, 3
  %3515 = icmp eq i64 %3514, 1200
  br i1 %3515, label %3524, label %3516, !llvm.loop !144

3516:
  %3517 = phi i64 [ 0, %3498 ], [ %3514, %3513 ]
  %3518 = getelementptr inbounds nuw i8, ptr @dst, i64 %3517
  %3519 = load i8, ptr %3518, align 1, !tbaa !10
  %3520 = getelementptr inbounds nuw i8, ptr @ref, i64 %3517
  %3521 = load i8, ptr %3520, align 1, !tbaa !10
  %3522 = icmp eq i8 %3519, %3521
  br i1 %3522, label %3499, label %3523

3523:
  tail call void @report(i64 noundef 31)
  store i1 true, ptr @bad, align 4
  br label %3525

3524:
  br label %3525

3525:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3526

3526:
  %3527 = phi i64 [ 0, %3525 ], [ %3547, %3526 ]
  %3528 = trunc i64 %3527 to i8
  %3529 = mul i8 %3528, 7
  %3530 = or disjoint i8 %3529, 3
  %3531 = getelementptr inbounds nuw i8, ptr @src, i64 %3527
  store i8 %3530, ptr %3531, align 4, !tbaa !10
  %3532 = or disjoint i64 %3527, 1
  %3533 = trunc i64 %3532 to i8
  %3534 = mul i8 %3533, 7
  %3535 = add i8 %3534, 3
  %3536 = getelementptr inbounds nuw i8, ptr @src, i64 %3532
  store i8 %3535, ptr %3536, align 1, !tbaa !10
  %3537 = or disjoint i64 %3527, 2
  %3538 = trunc i64 %3537 to i8
  %3539 = mul i8 %3538, 7
  %3540 = add i8 %3539, 3
  %3541 = getelementptr inbounds nuw i8, ptr @src, i64 %3537
  store i8 %3540, ptr %3541, align 2, !tbaa !10
  %3542 = or disjoint i64 %3527, 3
  %3543 = trunc i64 %3542 to i8
  %3544 = mul i8 %3543, 7
  %3545 = add i8 %3544, 3
  %3546 = getelementptr inbounds nuw i8, ptr @src, i64 %3542
  store i8 %3545, ptr %3546, align 1, !tbaa !10
  %3547 = add nuw nsw i64 %3527, 4
  %3548 = icmp eq i64 %3547, 1200
  br i1 %3548, label %3549, label %3526, !llvm.loop !11

3549:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr31()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 31)
  br label %3567

3550:
  %3551 = add nuw nsw i64 %3568, 1
  %3552 = getelementptr inbounds nuw i8, ptr @dst, i64 %3551
  %3553 = load i8, ptr %3552, align 1, !tbaa !10
  %3554 = getelementptr inbounds nuw i8, ptr @ref, i64 %3551
  %3555 = load i8, ptr %3554, align 1, !tbaa !10
  %3556 = icmp eq i8 %3553, %3555
  br i1 %3556, label %3557, label %3574

3557:
  %3558 = add nuw nsw i64 %3568, 2
  %3559 = getelementptr inbounds nuw i8, ptr @dst, i64 %3558
  %3560 = load i8, ptr %3559, align 1, !tbaa !10
  %3561 = getelementptr inbounds nuw i8, ptr @ref, i64 %3558
  %3562 = load i8, ptr %3561, align 1, !tbaa !10
  %3563 = icmp eq i8 %3560, %3562
  br i1 %3563, label %3564, label %3574

3564:
  %3565 = add nuw nsw i64 %3568, 3
  %3566 = icmp eq i64 %3565, 1200
  br i1 %3566, label %3575, label %3567, !llvm.loop !144

3567:
  %3568 = phi i64 [ 0, %3549 ], [ %3565, %3564 ]
  %3569 = getelementptr inbounds nuw i8, ptr @dst, i64 %3568
  %3570 = load i8, ptr %3569, align 1, !tbaa !10
  %3571 = getelementptr inbounds nuw i8, ptr @ref, i64 %3568
  %3572 = load i8, ptr %3571, align 1, !tbaa !10
  %3573 = icmp eq i8 %3570, %3572
  br i1 %3573, label %3550, label %3574

3574:
  tail call void @report(i64 noundef 31)
  store i1 true, ptr @bad, align 4
  br label %3576

3575:
  br label %3576

3576:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3577

3577:
  %3578 = phi i64 [ 0, %3576 ], [ %3598, %3577 ]
  %3579 = trunc i64 %3578 to i8
  %3580 = mul i8 %3579, 7
  %3581 = or disjoint i8 %3580, 3
  %3582 = getelementptr inbounds nuw i8, ptr @src, i64 %3578
  store i8 %3581, ptr %3582, align 4, !tbaa !10
  %3583 = or disjoint i64 %3578, 1
  %3584 = trunc i64 %3583 to i8
  %3585 = mul i8 %3584, 7
  %3586 = add i8 %3585, 3
  %3587 = getelementptr inbounds nuw i8, ptr @src, i64 %3583
  store i8 %3586, ptr %3587, align 1, !tbaa !10
  %3588 = or disjoint i64 %3578, 2
  %3589 = trunc i64 %3588 to i8
  %3590 = mul i8 %3589, 7
  %3591 = add i8 %3590, 3
  %3592 = getelementptr inbounds nuw i8, ptr @src, i64 %3588
  store i8 %3591, ptr %3592, align 2, !tbaa !10
  %3593 = or disjoint i64 %3578, 3
  %3594 = trunc i64 %3593 to i8
  %3595 = mul i8 %3594, 7
  %3596 = add i8 %3595, 3
  %3597 = getelementptr inbounds nuw i8, ptr @src, i64 %3593
  store i8 %3596, ptr %3597, align 1, !tbaa !10
  %3598 = add nuw nsw i64 %3578, 4
  %3599 = icmp eq i64 %3598, 1200
  br i1 %3599, label %3600, label %3577, !llvm.loop !11

3600:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv31(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 31)
  br label %3618

3601:
  %3602 = add nuw nsw i64 %3619, 1
  %3603 = getelementptr inbounds nuw i8, ptr @dst, i64 %3602
  %3604 = load i8, ptr %3603, align 1, !tbaa !10
  %3605 = getelementptr inbounds nuw i8, ptr @ref, i64 %3602
  %3606 = load i8, ptr %3605, align 1, !tbaa !10
  %3607 = icmp eq i8 %3604, %3606
  br i1 %3607, label %3608, label %3625

3608:
  %3609 = add nuw nsw i64 %3619, 2
  %3610 = getelementptr inbounds nuw i8, ptr @dst, i64 %3609
  %3611 = load i8, ptr %3610, align 1, !tbaa !10
  %3612 = getelementptr inbounds nuw i8, ptr @ref, i64 %3609
  %3613 = load i8, ptr %3612, align 1, !tbaa !10
  %3614 = icmp eq i8 %3611, %3613
  br i1 %3614, label %3615, label %3625

3615:
  %3616 = add nuw nsw i64 %3619, 3
  %3617 = icmp eq i64 %3616, 1200
  br i1 %3617, label %3626, label %3618, !llvm.loop !144

3618:
  %3619 = phi i64 [ 0, %3600 ], [ %3616, %3615 ]
  %3620 = getelementptr inbounds nuw i8, ptr @dst, i64 %3619
  %3621 = load i8, ptr %3620, align 1, !tbaa !10
  %3622 = getelementptr inbounds nuw i8, ptr @ref, i64 %3619
  %3623 = load i8, ptr %3622, align 1, !tbaa !10
  %3624 = icmp eq i8 %3621, %3623
  br i1 %3624, label %3601, label %3625

3625:
  tail call void @report(i64 noundef 31)
  store i1 true, ptr @bad, align 4
  br label %3627

3626:
  br label %3627

3627:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3628

3628:
  %3629 = phi i64 [ 0, %3627 ], [ %3649, %3628 ]
  %3630 = trunc i64 %3629 to i8
  %3631 = mul i8 %3630, 7
  %3632 = or disjoint i8 %3631, 3
  %3633 = getelementptr inbounds nuw i8, ptr @src, i64 %3629
  store i8 %3632, ptr %3633, align 4, !tbaa !10
  %3634 = or disjoint i64 %3629, 1
  %3635 = trunc i64 %3634 to i8
  %3636 = mul i8 %3635, 7
  %3637 = add i8 %3636, 3
  %3638 = getelementptr inbounds nuw i8, ptr @src, i64 %3634
  store i8 %3637, ptr %3638, align 1, !tbaa !10
  %3639 = or disjoint i64 %3629, 2
  %3640 = trunc i64 %3639 to i8
  %3641 = mul i8 %3640, 7
  %3642 = add i8 %3641, 3
  %3643 = getelementptr inbounds nuw i8, ptr @src, i64 %3639
  store i8 %3642, ptr %3643, align 2, !tbaa !10
  %3644 = or disjoint i64 %3629, 3
  %3645 = trunc i64 %3644 to i8
  %3646 = mul i8 %3645, 7
  %3647 = add i8 %3646, 3
  %3648 = getelementptr inbounds nuw i8, ptr @src, i64 %3644
  store i8 %3647, ptr %3648, align 1, !tbaa !10
  %3649 = add nuw nsw i64 %3629, 4
  %3650 = icmp eq i64 %3649, 1200
  br i1 %3650, label %3651, label %3628, !llvm.loop !11

3651:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv31(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 31)
  br label %3669

3652:
  %3653 = add nuw nsw i64 %3670, 1
  %3654 = getelementptr inbounds nuw i8, ptr @dst, i64 %3653
  %3655 = load i8, ptr %3654, align 1, !tbaa !10
  %3656 = getelementptr inbounds nuw i8, ptr @ref, i64 %3653
  %3657 = load i8, ptr %3656, align 1, !tbaa !10
  %3658 = icmp eq i8 %3655, %3657
  br i1 %3658, label %3659, label %3676

3659:
  %3660 = add nuw nsw i64 %3670, 2
  %3661 = getelementptr inbounds nuw i8, ptr @dst, i64 %3660
  %3662 = load i8, ptr %3661, align 1, !tbaa !10
  %3663 = getelementptr inbounds nuw i8, ptr @ref, i64 %3660
  %3664 = load i8, ptr %3663, align 1, !tbaa !10
  %3665 = icmp eq i8 %3662, %3664
  br i1 %3665, label %3666, label %3676

3666:
  %3667 = add nuw nsw i64 %3670, 3
  %3668 = icmp eq i64 %3667, 1200
  br i1 %3668, label %3677, label %3669, !llvm.loop !144

3669:
  %3670 = phi i64 [ 0, %3651 ], [ %3667, %3666 ]
  %3671 = getelementptr inbounds nuw i8, ptr @dst, i64 %3670
  %3672 = load i8, ptr %3671, align 1, !tbaa !10
  %3673 = getelementptr inbounds nuw i8, ptr @ref, i64 %3670
  %3674 = load i8, ptr %3673, align 1, !tbaa !10
  %3675 = icmp eq i8 %3672, %3674
  br i1 %3675, label %3652, label %3676

3676:
  tail call void @report(i64 noundef 31)
  store i1 true, ptr @bad, align 4
  br label %3678

3677:
  br label %3678

3678:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3679

3679:
  %3680 = phi i64 [ 0, %3678 ], [ %3700, %3679 ]
  %3681 = trunc i64 %3680 to i8
  %3682 = mul i8 %3681, 7
  %3683 = or disjoint i8 %3682, 3
  %3684 = getelementptr inbounds nuw i8, ptr @src, i64 %3680
  store i8 %3683, ptr %3684, align 4, !tbaa !10
  %3685 = or disjoint i64 %3680, 1
  %3686 = trunc i64 %3685 to i8
  %3687 = mul i8 %3686, 7
  %3688 = add i8 %3687, 3
  %3689 = getelementptr inbounds nuw i8, ptr @src, i64 %3685
  store i8 %3688, ptr %3689, align 1, !tbaa !10
  %3690 = or disjoint i64 %3680, 2
  %3691 = trunc i64 %3690 to i8
  %3692 = mul i8 %3691, 7
  %3693 = add i8 %3692, 3
  %3694 = getelementptr inbounds nuw i8, ptr @src, i64 %3690
  store i8 %3693, ptr %3694, align 2, !tbaa !10
  %3695 = or disjoint i64 %3680, 3
  %3696 = trunc i64 %3695 to i8
  %3697 = mul i8 %3696, 7
  %3698 = add i8 %3697, 3
  %3699 = getelementptr inbounds nuw i8, ptr @src, i64 %3695
  store i8 %3698, ptr %3699, align 1, !tbaa !10
  %3700 = add nuw nsw i64 %3680, 4
  %3701 = icmp eq i64 %3700, 1200
  br i1 %3701, label %3702, label %3679, !llvm.loop !11

3702:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp32()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 32)
  br label %3720

3703:
  %3704 = add nuw nsw i64 %3721, 1
  %3705 = getelementptr inbounds nuw i8, ptr @dst, i64 %3704
  %3706 = load i8, ptr %3705, align 1, !tbaa !10
  %3707 = getelementptr inbounds nuw i8, ptr @ref, i64 %3704
  %3708 = load i8, ptr %3707, align 1, !tbaa !10
  %3709 = icmp eq i8 %3706, %3708
  br i1 %3709, label %3710, label %3727

3710:
  %3711 = add nuw nsw i64 %3721, 2
  %3712 = getelementptr inbounds nuw i8, ptr @dst, i64 %3711
  %3713 = load i8, ptr %3712, align 1, !tbaa !10
  %3714 = getelementptr inbounds nuw i8, ptr @ref, i64 %3711
  %3715 = load i8, ptr %3714, align 1, !tbaa !10
  %3716 = icmp eq i8 %3713, %3715
  br i1 %3716, label %3717, label %3727

3717:
  %3718 = add nuw nsw i64 %3721, 3
  %3719 = icmp eq i64 %3718, 1200
  br i1 %3719, label %3728, label %3720, !llvm.loop !144

3720:
  %3721 = phi i64 [ 0, %3702 ], [ %3718, %3717 ]
  %3722 = getelementptr inbounds nuw i8, ptr @dst, i64 %3721
  %3723 = load i8, ptr %3722, align 1, !tbaa !10
  %3724 = getelementptr inbounds nuw i8, ptr @ref, i64 %3721
  %3725 = load i8, ptr %3724, align 1, !tbaa !10
  %3726 = icmp eq i8 %3723, %3725
  br i1 %3726, label %3703, label %3727

3727:
  tail call void @report(i64 noundef 32)
  store i1 true, ptr @bad, align 4
  br label %3729

3728:
  br label %3729

3729:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3730

3730:
  %3731 = phi i64 [ 0, %3729 ], [ %3751, %3730 ]
  %3732 = trunc i64 %3731 to i8
  %3733 = mul i8 %3732, 7
  %3734 = or disjoint i8 %3733, 3
  %3735 = getelementptr inbounds nuw i8, ptr @src, i64 %3731
  store i8 %3734, ptr %3735, align 4, !tbaa !10
  %3736 = or disjoint i64 %3731, 1
  %3737 = trunc i64 %3736 to i8
  %3738 = mul i8 %3737, 7
  %3739 = add i8 %3738, 3
  %3740 = getelementptr inbounds nuw i8, ptr @src, i64 %3736
  store i8 %3739, ptr %3740, align 1, !tbaa !10
  %3741 = or disjoint i64 %3731, 2
  %3742 = trunc i64 %3741 to i8
  %3743 = mul i8 %3742, 7
  %3744 = add i8 %3743, 3
  %3745 = getelementptr inbounds nuw i8, ptr @src, i64 %3741
  store i8 %3744, ptr %3745, align 2, !tbaa !10
  %3746 = or disjoint i64 %3731, 3
  %3747 = trunc i64 %3746 to i8
  %3748 = mul i8 %3747, 7
  %3749 = add i8 %3748, 3
  %3750 = getelementptr inbounds nuw i8, ptr @src, i64 %3746
  store i8 %3749, ptr %3750, align 1, !tbaa !10
  %3751 = add nuw nsw i64 %3731, 4
  %3752 = icmp eq i64 %3751, 1200
  br i1 %3752, label %3753, label %3730, !llvm.loop !11

3753:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr32()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 32)
  br label %3771

3754:
  %3755 = add nuw nsw i64 %3772, 1
  %3756 = getelementptr inbounds nuw i8, ptr @dst, i64 %3755
  %3757 = load i8, ptr %3756, align 1, !tbaa !10
  %3758 = getelementptr inbounds nuw i8, ptr @ref, i64 %3755
  %3759 = load i8, ptr %3758, align 1, !tbaa !10
  %3760 = icmp eq i8 %3757, %3759
  br i1 %3760, label %3761, label %3778

3761:
  %3762 = add nuw nsw i64 %3772, 2
  %3763 = getelementptr inbounds nuw i8, ptr @dst, i64 %3762
  %3764 = load i8, ptr %3763, align 1, !tbaa !10
  %3765 = getelementptr inbounds nuw i8, ptr @ref, i64 %3762
  %3766 = load i8, ptr %3765, align 1, !tbaa !10
  %3767 = icmp eq i8 %3764, %3766
  br i1 %3767, label %3768, label %3778

3768:
  %3769 = add nuw nsw i64 %3772, 3
  %3770 = icmp eq i64 %3769, 1200
  br i1 %3770, label %3779, label %3771, !llvm.loop !144

3771:
  %3772 = phi i64 [ 0, %3753 ], [ %3769, %3768 ]
  %3773 = getelementptr inbounds nuw i8, ptr @dst, i64 %3772
  %3774 = load i8, ptr %3773, align 1, !tbaa !10
  %3775 = getelementptr inbounds nuw i8, ptr @ref, i64 %3772
  %3776 = load i8, ptr %3775, align 1, !tbaa !10
  %3777 = icmp eq i8 %3774, %3776
  br i1 %3777, label %3754, label %3778

3778:
  tail call void @report(i64 noundef 32)
  store i1 true, ptr @bad, align 4
  br label %3780

3779:
  br label %3780

3780:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3781

3781:
  %3782 = phi i64 [ 0, %3780 ], [ %3802, %3781 ]
  %3783 = trunc i64 %3782 to i8
  %3784 = mul i8 %3783, 7
  %3785 = or disjoint i8 %3784, 3
  %3786 = getelementptr inbounds nuw i8, ptr @src, i64 %3782
  store i8 %3785, ptr %3786, align 4, !tbaa !10
  %3787 = or disjoint i64 %3782, 1
  %3788 = trunc i64 %3787 to i8
  %3789 = mul i8 %3788, 7
  %3790 = add i8 %3789, 3
  %3791 = getelementptr inbounds nuw i8, ptr @src, i64 %3787
  store i8 %3790, ptr %3791, align 1, !tbaa !10
  %3792 = or disjoint i64 %3782, 2
  %3793 = trunc i64 %3792 to i8
  %3794 = mul i8 %3793, 7
  %3795 = add i8 %3794, 3
  %3796 = getelementptr inbounds nuw i8, ptr @src, i64 %3792
  store i8 %3795, ptr %3796, align 2, !tbaa !10
  %3797 = or disjoint i64 %3782, 3
  %3798 = trunc i64 %3797 to i8
  %3799 = mul i8 %3798, 7
  %3800 = add i8 %3799, 3
  %3801 = getelementptr inbounds nuw i8, ptr @src, i64 %3797
  store i8 %3800, ptr %3801, align 1, !tbaa !10
  %3802 = add nuw nsw i64 %3782, 4
  %3803 = icmp eq i64 %3802, 1200
  br i1 %3803, label %3804, label %3781, !llvm.loop !11

3804:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c732()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 32)
  br label %3822

3805:
  %3806 = add nuw nsw i64 %3823, 1
  %3807 = getelementptr inbounds nuw i8, ptr @dst, i64 %3806
  %3808 = load i8, ptr %3807, align 1, !tbaa !10
  %3809 = getelementptr inbounds nuw i8, ptr @ref, i64 %3806
  %3810 = load i8, ptr %3809, align 1, !tbaa !10
  %3811 = icmp eq i8 %3808, %3810
  br i1 %3811, label %3812, label %3829

3812:
  %3813 = add nuw nsw i64 %3823, 2
  %3814 = getelementptr inbounds nuw i8, ptr @dst, i64 %3813
  %3815 = load i8, ptr %3814, align 1, !tbaa !10
  %3816 = getelementptr inbounds nuw i8, ptr @ref, i64 %3813
  %3817 = load i8, ptr %3816, align 1, !tbaa !10
  %3818 = icmp eq i8 %3815, %3817
  br i1 %3818, label %3819, label %3829

3819:
  %3820 = add nuw nsw i64 %3823, 3
  %3821 = icmp eq i64 %3820, 1200
  br i1 %3821, label %3830, label %3822, !llvm.loop !144

3822:
  %3823 = phi i64 [ 0, %3804 ], [ %3820, %3819 ]
  %3824 = getelementptr inbounds nuw i8, ptr @dst, i64 %3823
  %3825 = load i8, ptr %3824, align 1, !tbaa !10
  %3826 = getelementptr inbounds nuw i8, ptr @ref, i64 %3823
  %3827 = load i8, ptr %3826, align 1, !tbaa !10
  %3828 = icmp eq i8 %3825, %3827
  br i1 %3828, label %3805, label %3829

3829:
  tail call void @report(i64 noundef 32)
  store i1 true, ptr @bad, align 4
  br label %3831

3830:
  br label %3831

3831:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3832

3832:
  %3833 = phi i64 [ 0, %3831 ], [ %3853, %3832 ]
  %3834 = trunc i64 %3833 to i8
  %3835 = mul i8 %3834, 7
  %3836 = or disjoint i8 %3835, 3
  %3837 = getelementptr inbounds nuw i8, ptr @src, i64 %3833
  store i8 %3836, ptr %3837, align 4, !tbaa !10
  %3838 = or disjoint i64 %3833, 1
  %3839 = trunc i64 %3838 to i8
  %3840 = mul i8 %3839, 7
  %3841 = add i8 %3840, 3
  %3842 = getelementptr inbounds nuw i8, ptr @src, i64 %3838
  store i8 %3841, ptr %3842, align 1, !tbaa !10
  %3843 = or disjoint i64 %3833, 2
  %3844 = trunc i64 %3843 to i8
  %3845 = mul i8 %3844, 7
  %3846 = add i8 %3845, 3
  %3847 = getelementptr inbounds nuw i8, ptr @src, i64 %3843
  store i8 %3846, ptr %3847, align 2, !tbaa !10
  %3848 = or disjoint i64 %3833, 3
  %3849 = trunc i64 %3848 to i8
  %3850 = mul i8 %3849, 7
  %3851 = add i8 %3850, 3
  %3852 = getelementptr inbounds nuw i8, ptr @src, i64 %3848
  store i8 %3851, ptr %3852, align 1, !tbaa !10
  %3853 = add nuw nsw i64 %3833, 4
  %3854 = icmp eq i64 %3853, 1200
  br i1 %3854, label %3855, label %3832, !llvm.loop !11

3855:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr32()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 32)
  br label %3873

3856:
  %3857 = add nuw nsw i64 %3874, 1
  %3858 = getelementptr inbounds nuw i8, ptr @dst, i64 %3857
  %3859 = load i8, ptr %3858, align 1, !tbaa !10
  %3860 = getelementptr inbounds nuw i8, ptr @ref, i64 %3857
  %3861 = load i8, ptr %3860, align 1, !tbaa !10
  %3862 = icmp eq i8 %3859, %3861
  br i1 %3862, label %3863, label %3880

3863:
  %3864 = add nuw nsw i64 %3874, 2
  %3865 = getelementptr inbounds nuw i8, ptr @dst, i64 %3864
  %3866 = load i8, ptr %3865, align 1, !tbaa !10
  %3867 = getelementptr inbounds nuw i8, ptr @ref, i64 %3864
  %3868 = load i8, ptr %3867, align 1, !tbaa !10
  %3869 = icmp eq i8 %3866, %3868
  br i1 %3869, label %3870, label %3880

3870:
  %3871 = add nuw nsw i64 %3874, 3
  %3872 = icmp eq i64 %3871, 1200
  br i1 %3872, label %3881, label %3873, !llvm.loop !144

3873:
  %3874 = phi i64 [ 0, %3855 ], [ %3871, %3870 ]
  %3875 = getelementptr inbounds nuw i8, ptr @dst, i64 %3874
  %3876 = load i8, ptr %3875, align 1, !tbaa !10
  %3877 = getelementptr inbounds nuw i8, ptr @ref, i64 %3874
  %3878 = load i8, ptr %3877, align 1, !tbaa !10
  %3879 = icmp eq i8 %3876, %3878
  br i1 %3879, label %3856, label %3880

3880:
  tail call void @report(i64 noundef 32)
  store i1 true, ptr @bad, align 4
  br label %3882

3881:
  br label %3882

3882:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3883

3883:
  %3884 = phi i64 [ 0, %3882 ], [ %3904, %3883 ]
  %3885 = trunc i64 %3884 to i8
  %3886 = mul i8 %3885, 7
  %3887 = or disjoint i8 %3886, 3
  %3888 = getelementptr inbounds nuw i8, ptr @src, i64 %3884
  store i8 %3887, ptr %3888, align 4, !tbaa !10
  %3889 = or disjoint i64 %3884, 1
  %3890 = trunc i64 %3889 to i8
  %3891 = mul i8 %3890, 7
  %3892 = add i8 %3891, 3
  %3893 = getelementptr inbounds nuw i8, ptr @src, i64 %3889
  store i8 %3892, ptr %3893, align 1, !tbaa !10
  %3894 = or disjoint i64 %3884, 2
  %3895 = trunc i64 %3894 to i8
  %3896 = mul i8 %3895, 7
  %3897 = add i8 %3896, 3
  %3898 = getelementptr inbounds nuw i8, ptr @src, i64 %3894
  store i8 %3897, ptr %3898, align 2, !tbaa !10
  %3899 = or disjoint i64 %3884, 3
  %3900 = trunc i64 %3899 to i8
  %3901 = mul i8 %3900, 7
  %3902 = add i8 %3901, 3
  %3903 = getelementptr inbounds nuw i8, ptr @src, i64 %3899
  store i8 %3902, ptr %3903, align 1, !tbaa !10
  %3904 = add nuw nsw i64 %3884, 4
  %3905 = icmp eq i64 %3904, 1200
  br i1 %3905, label %3906, label %3883, !llvm.loop !11

3906:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv32(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 32)
  br label %3924

3907:
  %3908 = add nuw nsw i64 %3925, 1
  %3909 = getelementptr inbounds nuw i8, ptr @dst, i64 %3908
  %3910 = load i8, ptr %3909, align 1, !tbaa !10
  %3911 = getelementptr inbounds nuw i8, ptr @ref, i64 %3908
  %3912 = load i8, ptr %3911, align 1, !tbaa !10
  %3913 = icmp eq i8 %3910, %3912
  br i1 %3913, label %3914, label %3931

3914:
  %3915 = add nuw nsw i64 %3925, 2
  %3916 = getelementptr inbounds nuw i8, ptr @dst, i64 %3915
  %3917 = load i8, ptr %3916, align 1, !tbaa !10
  %3918 = getelementptr inbounds nuw i8, ptr @ref, i64 %3915
  %3919 = load i8, ptr %3918, align 1, !tbaa !10
  %3920 = icmp eq i8 %3917, %3919
  br i1 %3920, label %3921, label %3931

3921:
  %3922 = add nuw nsw i64 %3925, 3
  %3923 = icmp eq i64 %3922, 1200
  br i1 %3923, label %3932, label %3924, !llvm.loop !144

3924:
  %3925 = phi i64 [ 0, %3906 ], [ %3922, %3921 ]
  %3926 = getelementptr inbounds nuw i8, ptr @dst, i64 %3925
  %3927 = load i8, ptr %3926, align 1, !tbaa !10
  %3928 = getelementptr inbounds nuw i8, ptr @ref, i64 %3925
  %3929 = load i8, ptr %3928, align 1, !tbaa !10
  %3930 = icmp eq i8 %3927, %3929
  br i1 %3930, label %3907, label %3931

3931:
  tail call void @report(i64 noundef 32)
  store i1 true, ptr @bad, align 4
  br label %3933

3932:
  br label %3933

3933:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3934

3934:
  %3935 = phi i64 [ 0, %3933 ], [ %3955, %3934 ]
  %3936 = trunc i64 %3935 to i8
  %3937 = mul i8 %3936, 7
  %3938 = or disjoint i8 %3937, 3
  %3939 = getelementptr inbounds nuw i8, ptr @src, i64 %3935
  store i8 %3938, ptr %3939, align 4, !tbaa !10
  %3940 = or disjoint i64 %3935, 1
  %3941 = trunc i64 %3940 to i8
  %3942 = mul i8 %3941, 7
  %3943 = add i8 %3942, 3
  %3944 = getelementptr inbounds nuw i8, ptr @src, i64 %3940
  store i8 %3943, ptr %3944, align 1, !tbaa !10
  %3945 = or disjoint i64 %3935, 2
  %3946 = trunc i64 %3945 to i8
  %3947 = mul i8 %3946, 7
  %3948 = add i8 %3947, 3
  %3949 = getelementptr inbounds nuw i8, ptr @src, i64 %3945
  store i8 %3948, ptr %3949, align 2, !tbaa !10
  %3950 = or disjoint i64 %3935, 3
  %3951 = trunc i64 %3950 to i8
  %3952 = mul i8 %3951, 7
  %3953 = add i8 %3952, 3
  %3954 = getelementptr inbounds nuw i8, ptr @src, i64 %3950
  store i8 %3953, ptr %3954, align 1, !tbaa !10
  %3955 = add nuw nsw i64 %3935, 4
  %3956 = icmp eq i64 %3955, 1200
  br i1 %3956, label %3957, label %3934, !llvm.loop !11

3957:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv32(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 32)
  br label %3975

3958:
  %3959 = add nuw nsw i64 %3976, 1
  %3960 = getelementptr inbounds nuw i8, ptr @dst, i64 %3959
  %3961 = load i8, ptr %3960, align 1, !tbaa !10
  %3962 = getelementptr inbounds nuw i8, ptr @ref, i64 %3959
  %3963 = load i8, ptr %3962, align 1, !tbaa !10
  %3964 = icmp eq i8 %3961, %3963
  br i1 %3964, label %3965, label %3982

3965:
  %3966 = add nuw nsw i64 %3976, 2
  %3967 = getelementptr inbounds nuw i8, ptr @dst, i64 %3966
  %3968 = load i8, ptr %3967, align 1, !tbaa !10
  %3969 = getelementptr inbounds nuw i8, ptr @ref, i64 %3966
  %3970 = load i8, ptr %3969, align 1, !tbaa !10
  %3971 = icmp eq i8 %3968, %3970
  br i1 %3971, label %3972, label %3982

3972:
  %3973 = add nuw nsw i64 %3976, 3
  %3974 = icmp eq i64 %3973, 1200
  br i1 %3974, label %3983, label %3975, !llvm.loop !144

3975:
  %3976 = phi i64 [ 0, %3957 ], [ %3973, %3972 ]
  %3977 = getelementptr inbounds nuw i8, ptr @dst, i64 %3976
  %3978 = load i8, ptr %3977, align 1, !tbaa !10
  %3979 = getelementptr inbounds nuw i8, ptr @ref, i64 %3976
  %3980 = load i8, ptr %3979, align 1, !tbaa !10
  %3981 = icmp eq i8 %3978, %3980
  br i1 %3981, label %3958, label %3982

3982:
  tail call void @report(i64 noundef 32)
  store i1 true, ptr @bad, align 4
  br label %3984

3983:
  br label %3984

3984:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %3985

3985:
  %3986 = phi i64 [ 0, %3984 ], [ %4006, %3985 ]
  %3987 = trunc i64 %3986 to i8
  %3988 = mul i8 %3987, 7
  %3989 = or disjoint i8 %3988, 3
  %3990 = getelementptr inbounds nuw i8, ptr @src, i64 %3986
  store i8 %3989, ptr %3990, align 4, !tbaa !10
  %3991 = or disjoint i64 %3986, 1
  %3992 = trunc i64 %3991 to i8
  %3993 = mul i8 %3992, 7
  %3994 = add i8 %3993, 3
  %3995 = getelementptr inbounds nuw i8, ptr @src, i64 %3991
  store i8 %3994, ptr %3995, align 1, !tbaa !10
  %3996 = or disjoint i64 %3986, 2
  %3997 = trunc i64 %3996 to i8
  %3998 = mul i8 %3997, 7
  %3999 = add i8 %3998, 3
  %4000 = getelementptr inbounds nuw i8, ptr @src, i64 %3996
  store i8 %3999, ptr %4000, align 2, !tbaa !10
  %4001 = or disjoint i64 %3986, 3
  %4002 = trunc i64 %4001 to i8
  %4003 = mul i8 %4002, 7
  %4004 = add i8 %4003, 3
  %4005 = getelementptr inbounds nuw i8, ptr @src, i64 %4001
  store i8 %4004, ptr %4005, align 1, !tbaa !10
  %4006 = add nuw nsw i64 %3986, 4
  %4007 = icmp eq i64 %4006, 1200
  br i1 %4007, label %4008, label %3985, !llvm.loop !11

4008:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp33()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 33)
  br label %4026

4009:
  %4010 = add nuw nsw i64 %4027, 1
  %4011 = getelementptr inbounds nuw i8, ptr @dst, i64 %4010
  %4012 = load i8, ptr %4011, align 1, !tbaa !10
  %4013 = getelementptr inbounds nuw i8, ptr @ref, i64 %4010
  %4014 = load i8, ptr %4013, align 1, !tbaa !10
  %4015 = icmp eq i8 %4012, %4014
  br i1 %4015, label %4016, label %4033

4016:
  %4017 = add nuw nsw i64 %4027, 2
  %4018 = getelementptr inbounds nuw i8, ptr @dst, i64 %4017
  %4019 = load i8, ptr %4018, align 1, !tbaa !10
  %4020 = getelementptr inbounds nuw i8, ptr @ref, i64 %4017
  %4021 = load i8, ptr %4020, align 1, !tbaa !10
  %4022 = icmp eq i8 %4019, %4021
  br i1 %4022, label %4023, label %4033

4023:
  %4024 = add nuw nsw i64 %4027, 3
  %4025 = icmp eq i64 %4024, 1200
  br i1 %4025, label %4034, label %4026, !llvm.loop !144

4026:
  %4027 = phi i64 [ 0, %4008 ], [ %4024, %4023 ]
  %4028 = getelementptr inbounds nuw i8, ptr @dst, i64 %4027
  %4029 = load i8, ptr %4028, align 1, !tbaa !10
  %4030 = getelementptr inbounds nuw i8, ptr @ref, i64 %4027
  %4031 = load i8, ptr %4030, align 1, !tbaa !10
  %4032 = icmp eq i8 %4029, %4031
  br i1 %4032, label %4009, label %4033

4033:
  tail call void @report(i64 noundef 33)
  store i1 true, ptr @bad, align 4
  br label %4035

4034:
  br label %4035

4035:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4036

4036:
  %4037 = phi i64 [ 0, %4035 ], [ %4057, %4036 ]
  %4038 = trunc i64 %4037 to i8
  %4039 = mul i8 %4038, 7
  %4040 = or disjoint i8 %4039, 3
  %4041 = getelementptr inbounds nuw i8, ptr @src, i64 %4037
  store i8 %4040, ptr %4041, align 4, !tbaa !10
  %4042 = or disjoint i64 %4037, 1
  %4043 = trunc i64 %4042 to i8
  %4044 = mul i8 %4043, 7
  %4045 = add i8 %4044, 3
  %4046 = getelementptr inbounds nuw i8, ptr @src, i64 %4042
  store i8 %4045, ptr %4046, align 1, !tbaa !10
  %4047 = or disjoint i64 %4037, 2
  %4048 = trunc i64 %4047 to i8
  %4049 = mul i8 %4048, 7
  %4050 = add i8 %4049, 3
  %4051 = getelementptr inbounds nuw i8, ptr @src, i64 %4047
  store i8 %4050, ptr %4051, align 2, !tbaa !10
  %4052 = or disjoint i64 %4037, 3
  %4053 = trunc i64 %4052 to i8
  %4054 = mul i8 %4053, 7
  %4055 = add i8 %4054, 3
  %4056 = getelementptr inbounds nuw i8, ptr @src, i64 %4052
  store i8 %4055, ptr %4056, align 1, !tbaa !10
  %4057 = add nuw nsw i64 %4037, 4
  %4058 = icmp eq i64 %4057, 1200
  br i1 %4058, label %4059, label %4036, !llvm.loop !11

4059:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr33()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 33)
  br label %4077

4060:
  %4061 = add nuw nsw i64 %4078, 1
  %4062 = getelementptr inbounds nuw i8, ptr @dst, i64 %4061
  %4063 = load i8, ptr %4062, align 1, !tbaa !10
  %4064 = getelementptr inbounds nuw i8, ptr @ref, i64 %4061
  %4065 = load i8, ptr %4064, align 1, !tbaa !10
  %4066 = icmp eq i8 %4063, %4065
  br i1 %4066, label %4067, label %4084

4067:
  %4068 = add nuw nsw i64 %4078, 2
  %4069 = getelementptr inbounds nuw i8, ptr @dst, i64 %4068
  %4070 = load i8, ptr %4069, align 1, !tbaa !10
  %4071 = getelementptr inbounds nuw i8, ptr @ref, i64 %4068
  %4072 = load i8, ptr %4071, align 1, !tbaa !10
  %4073 = icmp eq i8 %4070, %4072
  br i1 %4073, label %4074, label %4084

4074:
  %4075 = add nuw nsw i64 %4078, 3
  %4076 = icmp eq i64 %4075, 1200
  br i1 %4076, label %4085, label %4077, !llvm.loop !144

4077:
  %4078 = phi i64 [ 0, %4059 ], [ %4075, %4074 ]
  %4079 = getelementptr inbounds nuw i8, ptr @dst, i64 %4078
  %4080 = load i8, ptr %4079, align 1, !tbaa !10
  %4081 = getelementptr inbounds nuw i8, ptr @ref, i64 %4078
  %4082 = load i8, ptr %4081, align 1, !tbaa !10
  %4083 = icmp eq i8 %4080, %4082
  br i1 %4083, label %4060, label %4084

4084:
  tail call void @report(i64 noundef 33)
  store i1 true, ptr @bad, align 4
  br label %4086

4085:
  br label %4086

4086:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4087

4087:
  %4088 = phi i64 [ 0, %4086 ], [ %4108, %4087 ]
  %4089 = trunc i64 %4088 to i8
  %4090 = mul i8 %4089, 7
  %4091 = or disjoint i8 %4090, 3
  %4092 = getelementptr inbounds nuw i8, ptr @src, i64 %4088
  store i8 %4091, ptr %4092, align 4, !tbaa !10
  %4093 = or disjoint i64 %4088, 1
  %4094 = trunc i64 %4093 to i8
  %4095 = mul i8 %4094, 7
  %4096 = add i8 %4095, 3
  %4097 = getelementptr inbounds nuw i8, ptr @src, i64 %4093
  store i8 %4096, ptr %4097, align 1, !tbaa !10
  %4098 = or disjoint i64 %4088, 2
  %4099 = trunc i64 %4098 to i8
  %4100 = mul i8 %4099, 7
  %4101 = add i8 %4100, 3
  %4102 = getelementptr inbounds nuw i8, ptr @src, i64 %4098
  store i8 %4101, ptr %4102, align 2, !tbaa !10
  %4103 = or disjoint i64 %4088, 3
  %4104 = trunc i64 %4103 to i8
  %4105 = mul i8 %4104, 7
  %4106 = add i8 %4105, 3
  %4107 = getelementptr inbounds nuw i8, ptr @src, i64 %4103
  store i8 %4106, ptr %4107, align 1, !tbaa !10
  %4108 = add nuw nsw i64 %4088, 4
  %4109 = icmp eq i64 %4108, 1200
  br i1 %4109, label %4110, label %4087, !llvm.loop !11

4110:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c733()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 33)
  br label %4128

4111:
  %4112 = add nuw nsw i64 %4129, 1
  %4113 = getelementptr inbounds nuw i8, ptr @dst, i64 %4112
  %4114 = load i8, ptr %4113, align 1, !tbaa !10
  %4115 = getelementptr inbounds nuw i8, ptr @ref, i64 %4112
  %4116 = load i8, ptr %4115, align 1, !tbaa !10
  %4117 = icmp eq i8 %4114, %4116
  br i1 %4117, label %4118, label %4135

4118:
  %4119 = add nuw nsw i64 %4129, 2
  %4120 = getelementptr inbounds nuw i8, ptr @dst, i64 %4119
  %4121 = load i8, ptr %4120, align 1, !tbaa !10
  %4122 = getelementptr inbounds nuw i8, ptr @ref, i64 %4119
  %4123 = load i8, ptr %4122, align 1, !tbaa !10
  %4124 = icmp eq i8 %4121, %4123
  br i1 %4124, label %4125, label %4135

4125:
  %4126 = add nuw nsw i64 %4129, 3
  %4127 = icmp eq i64 %4126, 1200
  br i1 %4127, label %4136, label %4128, !llvm.loop !144

4128:
  %4129 = phi i64 [ 0, %4110 ], [ %4126, %4125 ]
  %4130 = getelementptr inbounds nuw i8, ptr @dst, i64 %4129
  %4131 = load i8, ptr %4130, align 1, !tbaa !10
  %4132 = getelementptr inbounds nuw i8, ptr @ref, i64 %4129
  %4133 = load i8, ptr %4132, align 1, !tbaa !10
  %4134 = icmp eq i8 %4131, %4133
  br i1 %4134, label %4111, label %4135

4135:
  tail call void @report(i64 noundef 33)
  store i1 true, ptr @bad, align 4
  br label %4137

4136:
  br label %4137

4137:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4138

4138:
  %4139 = phi i64 [ 0, %4137 ], [ %4159, %4138 ]
  %4140 = trunc i64 %4139 to i8
  %4141 = mul i8 %4140, 7
  %4142 = or disjoint i8 %4141, 3
  %4143 = getelementptr inbounds nuw i8, ptr @src, i64 %4139
  store i8 %4142, ptr %4143, align 4, !tbaa !10
  %4144 = or disjoint i64 %4139, 1
  %4145 = trunc i64 %4144 to i8
  %4146 = mul i8 %4145, 7
  %4147 = add i8 %4146, 3
  %4148 = getelementptr inbounds nuw i8, ptr @src, i64 %4144
  store i8 %4147, ptr %4148, align 1, !tbaa !10
  %4149 = or disjoint i64 %4139, 2
  %4150 = trunc i64 %4149 to i8
  %4151 = mul i8 %4150, 7
  %4152 = add i8 %4151, 3
  %4153 = getelementptr inbounds nuw i8, ptr @src, i64 %4149
  store i8 %4152, ptr %4153, align 2, !tbaa !10
  %4154 = or disjoint i64 %4139, 3
  %4155 = trunc i64 %4154 to i8
  %4156 = mul i8 %4155, 7
  %4157 = add i8 %4156, 3
  %4158 = getelementptr inbounds nuw i8, ptr @src, i64 %4154
  store i8 %4157, ptr %4158, align 1, !tbaa !10
  %4159 = add nuw nsw i64 %4139, 4
  %4160 = icmp eq i64 %4159, 1200
  br i1 %4160, label %4161, label %4138, !llvm.loop !11

4161:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr33()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 33)
  br label %4179

4162:
  %4163 = add nuw nsw i64 %4180, 1
  %4164 = getelementptr inbounds nuw i8, ptr @dst, i64 %4163
  %4165 = load i8, ptr %4164, align 1, !tbaa !10
  %4166 = getelementptr inbounds nuw i8, ptr @ref, i64 %4163
  %4167 = load i8, ptr %4166, align 1, !tbaa !10
  %4168 = icmp eq i8 %4165, %4167
  br i1 %4168, label %4169, label %4186

4169:
  %4170 = add nuw nsw i64 %4180, 2
  %4171 = getelementptr inbounds nuw i8, ptr @dst, i64 %4170
  %4172 = load i8, ptr %4171, align 1, !tbaa !10
  %4173 = getelementptr inbounds nuw i8, ptr @ref, i64 %4170
  %4174 = load i8, ptr %4173, align 1, !tbaa !10
  %4175 = icmp eq i8 %4172, %4174
  br i1 %4175, label %4176, label %4186

4176:
  %4177 = add nuw nsw i64 %4180, 3
  %4178 = icmp eq i64 %4177, 1200
  br i1 %4178, label %4187, label %4179, !llvm.loop !144

4179:
  %4180 = phi i64 [ 0, %4161 ], [ %4177, %4176 ]
  %4181 = getelementptr inbounds nuw i8, ptr @dst, i64 %4180
  %4182 = load i8, ptr %4181, align 1, !tbaa !10
  %4183 = getelementptr inbounds nuw i8, ptr @ref, i64 %4180
  %4184 = load i8, ptr %4183, align 1, !tbaa !10
  %4185 = icmp eq i8 %4182, %4184
  br i1 %4185, label %4162, label %4186

4186:
  tail call void @report(i64 noundef 33)
  store i1 true, ptr @bad, align 4
  br label %4188

4187:
  br label %4188

4188:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4189

4189:
  %4190 = phi i64 [ 0, %4188 ], [ %4210, %4189 ]
  %4191 = trunc i64 %4190 to i8
  %4192 = mul i8 %4191, 7
  %4193 = or disjoint i8 %4192, 3
  %4194 = getelementptr inbounds nuw i8, ptr @src, i64 %4190
  store i8 %4193, ptr %4194, align 4, !tbaa !10
  %4195 = or disjoint i64 %4190, 1
  %4196 = trunc i64 %4195 to i8
  %4197 = mul i8 %4196, 7
  %4198 = add i8 %4197, 3
  %4199 = getelementptr inbounds nuw i8, ptr @src, i64 %4195
  store i8 %4198, ptr %4199, align 1, !tbaa !10
  %4200 = or disjoint i64 %4190, 2
  %4201 = trunc i64 %4200 to i8
  %4202 = mul i8 %4201, 7
  %4203 = add i8 %4202, 3
  %4204 = getelementptr inbounds nuw i8, ptr @src, i64 %4200
  store i8 %4203, ptr %4204, align 2, !tbaa !10
  %4205 = or disjoint i64 %4190, 3
  %4206 = trunc i64 %4205 to i8
  %4207 = mul i8 %4206, 7
  %4208 = add i8 %4207, 3
  %4209 = getelementptr inbounds nuw i8, ptr @src, i64 %4205
  store i8 %4208, ptr %4209, align 1, !tbaa !10
  %4210 = add nuw nsw i64 %4190, 4
  %4211 = icmp eq i64 %4210, 1200
  br i1 %4211, label %4212, label %4189, !llvm.loop !11

4212:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv33(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 33)
  br label %4230

4213:
  %4214 = add nuw nsw i64 %4231, 1
  %4215 = getelementptr inbounds nuw i8, ptr @dst, i64 %4214
  %4216 = load i8, ptr %4215, align 1, !tbaa !10
  %4217 = getelementptr inbounds nuw i8, ptr @ref, i64 %4214
  %4218 = load i8, ptr %4217, align 1, !tbaa !10
  %4219 = icmp eq i8 %4216, %4218
  br i1 %4219, label %4220, label %4237

4220:
  %4221 = add nuw nsw i64 %4231, 2
  %4222 = getelementptr inbounds nuw i8, ptr @dst, i64 %4221
  %4223 = load i8, ptr %4222, align 1, !tbaa !10
  %4224 = getelementptr inbounds nuw i8, ptr @ref, i64 %4221
  %4225 = load i8, ptr %4224, align 1, !tbaa !10
  %4226 = icmp eq i8 %4223, %4225
  br i1 %4226, label %4227, label %4237

4227:
  %4228 = add nuw nsw i64 %4231, 3
  %4229 = icmp eq i64 %4228, 1200
  br i1 %4229, label %4238, label %4230, !llvm.loop !144

4230:
  %4231 = phi i64 [ 0, %4212 ], [ %4228, %4227 ]
  %4232 = getelementptr inbounds nuw i8, ptr @dst, i64 %4231
  %4233 = load i8, ptr %4232, align 1, !tbaa !10
  %4234 = getelementptr inbounds nuw i8, ptr @ref, i64 %4231
  %4235 = load i8, ptr %4234, align 1, !tbaa !10
  %4236 = icmp eq i8 %4233, %4235
  br i1 %4236, label %4213, label %4237

4237:
  tail call void @report(i64 noundef 33)
  store i1 true, ptr @bad, align 4
  br label %4239

4238:
  br label %4239

4239:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4240

4240:
  %4241 = phi i64 [ 0, %4239 ], [ %4261, %4240 ]
  %4242 = trunc i64 %4241 to i8
  %4243 = mul i8 %4242, 7
  %4244 = or disjoint i8 %4243, 3
  %4245 = getelementptr inbounds nuw i8, ptr @src, i64 %4241
  store i8 %4244, ptr %4245, align 4, !tbaa !10
  %4246 = or disjoint i64 %4241, 1
  %4247 = trunc i64 %4246 to i8
  %4248 = mul i8 %4247, 7
  %4249 = add i8 %4248, 3
  %4250 = getelementptr inbounds nuw i8, ptr @src, i64 %4246
  store i8 %4249, ptr %4250, align 1, !tbaa !10
  %4251 = or disjoint i64 %4241, 2
  %4252 = trunc i64 %4251 to i8
  %4253 = mul i8 %4252, 7
  %4254 = add i8 %4253, 3
  %4255 = getelementptr inbounds nuw i8, ptr @src, i64 %4251
  store i8 %4254, ptr %4255, align 2, !tbaa !10
  %4256 = or disjoint i64 %4241, 3
  %4257 = trunc i64 %4256 to i8
  %4258 = mul i8 %4257, 7
  %4259 = add i8 %4258, 3
  %4260 = getelementptr inbounds nuw i8, ptr @src, i64 %4256
  store i8 %4259, ptr %4260, align 1, !tbaa !10
  %4261 = add nuw nsw i64 %4241, 4
  %4262 = icmp eq i64 %4261, 1200
  br i1 %4262, label %4263, label %4240, !llvm.loop !11

4263:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv33(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 33)
  br label %4281

4264:
  %4265 = add nuw nsw i64 %4282, 1
  %4266 = getelementptr inbounds nuw i8, ptr @dst, i64 %4265
  %4267 = load i8, ptr %4266, align 1, !tbaa !10
  %4268 = getelementptr inbounds nuw i8, ptr @ref, i64 %4265
  %4269 = load i8, ptr %4268, align 1, !tbaa !10
  %4270 = icmp eq i8 %4267, %4269
  br i1 %4270, label %4271, label %4288

4271:
  %4272 = add nuw nsw i64 %4282, 2
  %4273 = getelementptr inbounds nuw i8, ptr @dst, i64 %4272
  %4274 = load i8, ptr %4273, align 1, !tbaa !10
  %4275 = getelementptr inbounds nuw i8, ptr @ref, i64 %4272
  %4276 = load i8, ptr %4275, align 1, !tbaa !10
  %4277 = icmp eq i8 %4274, %4276
  br i1 %4277, label %4278, label %4288

4278:
  %4279 = add nuw nsw i64 %4282, 3
  %4280 = icmp eq i64 %4279, 1200
  br i1 %4280, label %4289, label %4281, !llvm.loop !144

4281:
  %4282 = phi i64 [ 0, %4263 ], [ %4279, %4278 ]
  %4283 = getelementptr inbounds nuw i8, ptr @dst, i64 %4282
  %4284 = load i8, ptr %4283, align 1, !tbaa !10
  %4285 = getelementptr inbounds nuw i8, ptr @ref, i64 %4282
  %4286 = load i8, ptr %4285, align 1, !tbaa !10
  %4287 = icmp eq i8 %4284, %4286
  br i1 %4287, label %4264, label %4288

4288:
  tail call void @report(i64 noundef 33)
  store i1 true, ptr @bad, align 4
  br label %4290

4289:
  br label %4290

4290:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4291

4291:
  %4292 = phi i64 [ 0, %4290 ], [ %4312, %4291 ]
  %4293 = trunc i64 %4292 to i8
  %4294 = mul i8 %4293, 7
  %4295 = or disjoint i8 %4294, 3
  %4296 = getelementptr inbounds nuw i8, ptr @src, i64 %4292
  store i8 %4295, ptr %4296, align 4, !tbaa !10
  %4297 = or disjoint i64 %4292, 1
  %4298 = trunc i64 %4297 to i8
  %4299 = mul i8 %4298, 7
  %4300 = add i8 %4299, 3
  %4301 = getelementptr inbounds nuw i8, ptr @src, i64 %4297
  store i8 %4300, ptr %4301, align 1, !tbaa !10
  %4302 = or disjoint i64 %4292, 2
  %4303 = trunc i64 %4302 to i8
  %4304 = mul i8 %4303, 7
  %4305 = add i8 %4304, 3
  %4306 = getelementptr inbounds nuw i8, ptr @src, i64 %4302
  store i8 %4305, ptr %4306, align 2, !tbaa !10
  %4307 = or disjoint i64 %4292, 3
  %4308 = trunc i64 %4307 to i8
  %4309 = mul i8 %4308, 7
  %4310 = add i8 %4309, 3
  %4311 = getelementptr inbounds nuw i8, ptr @src, i64 %4307
  store i8 %4310, ptr %4311, align 1, !tbaa !10
  %4312 = add nuw nsw i64 %4292, 4
  %4313 = icmp eq i64 %4312, 1200
  br i1 %4313, label %4314, label %4291, !llvm.loop !11

4314:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp40()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 40)
  br label %4332

4315:
  %4316 = add nuw nsw i64 %4333, 1
  %4317 = getelementptr inbounds nuw i8, ptr @dst, i64 %4316
  %4318 = load i8, ptr %4317, align 1, !tbaa !10
  %4319 = getelementptr inbounds nuw i8, ptr @ref, i64 %4316
  %4320 = load i8, ptr %4319, align 1, !tbaa !10
  %4321 = icmp eq i8 %4318, %4320
  br i1 %4321, label %4322, label %4339

4322:
  %4323 = add nuw nsw i64 %4333, 2
  %4324 = getelementptr inbounds nuw i8, ptr @dst, i64 %4323
  %4325 = load i8, ptr %4324, align 1, !tbaa !10
  %4326 = getelementptr inbounds nuw i8, ptr @ref, i64 %4323
  %4327 = load i8, ptr %4326, align 1, !tbaa !10
  %4328 = icmp eq i8 %4325, %4327
  br i1 %4328, label %4329, label %4339

4329:
  %4330 = add nuw nsw i64 %4333, 3
  %4331 = icmp eq i64 %4330, 1200
  br i1 %4331, label %4340, label %4332, !llvm.loop !144

4332:
  %4333 = phi i64 [ 0, %4314 ], [ %4330, %4329 ]
  %4334 = getelementptr inbounds nuw i8, ptr @dst, i64 %4333
  %4335 = load i8, ptr %4334, align 1, !tbaa !10
  %4336 = getelementptr inbounds nuw i8, ptr @ref, i64 %4333
  %4337 = load i8, ptr %4336, align 1, !tbaa !10
  %4338 = icmp eq i8 %4335, %4337
  br i1 %4338, label %4315, label %4339

4339:
  tail call void @report(i64 noundef 40)
  store i1 true, ptr @bad, align 4
  br label %4341

4340:
  br label %4341

4341:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4342

4342:
  %4343 = phi i64 [ 0, %4341 ], [ %4363, %4342 ]
  %4344 = trunc i64 %4343 to i8
  %4345 = mul i8 %4344, 7
  %4346 = or disjoint i8 %4345, 3
  %4347 = getelementptr inbounds nuw i8, ptr @src, i64 %4343
  store i8 %4346, ptr %4347, align 4, !tbaa !10
  %4348 = or disjoint i64 %4343, 1
  %4349 = trunc i64 %4348 to i8
  %4350 = mul i8 %4349, 7
  %4351 = add i8 %4350, 3
  %4352 = getelementptr inbounds nuw i8, ptr @src, i64 %4348
  store i8 %4351, ptr %4352, align 1, !tbaa !10
  %4353 = or disjoint i64 %4343, 2
  %4354 = trunc i64 %4353 to i8
  %4355 = mul i8 %4354, 7
  %4356 = add i8 %4355, 3
  %4357 = getelementptr inbounds nuw i8, ptr @src, i64 %4353
  store i8 %4356, ptr %4357, align 2, !tbaa !10
  %4358 = or disjoint i64 %4343, 3
  %4359 = trunc i64 %4358 to i8
  %4360 = mul i8 %4359, 7
  %4361 = add i8 %4360, 3
  %4362 = getelementptr inbounds nuw i8, ptr @src, i64 %4358
  store i8 %4361, ptr %4362, align 1, !tbaa !10
  %4363 = add nuw nsw i64 %4343, 4
  %4364 = icmp eq i64 %4363, 1200
  br i1 %4364, label %4365, label %4342, !llvm.loop !11

4365:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr40()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 40)
  br label %4383

4366:
  %4367 = add nuw nsw i64 %4384, 1
  %4368 = getelementptr inbounds nuw i8, ptr @dst, i64 %4367
  %4369 = load i8, ptr %4368, align 1, !tbaa !10
  %4370 = getelementptr inbounds nuw i8, ptr @ref, i64 %4367
  %4371 = load i8, ptr %4370, align 1, !tbaa !10
  %4372 = icmp eq i8 %4369, %4371
  br i1 %4372, label %4373, label %4390

4373:
  %4374 = add nuw nsw i64 %4384, 2
  %4375 = getelementptr inbounds nuw i8, ptr @dst, i64 %4374
  %4376 = load i8, ptr %4375, align 1, !tbaa !10
  %4377 = getelementptr inbounds nuw i8, ptr @ref, i64 %4374
  %4378 = load i8, ptr %4377, align 1, !tbaa !10
  %4379 = icmp eq i8 %4376, %4378
  br i1 %4379, label %4380, label %4390

4380:
  %4381 = add nuw nsw i64 %4384, 3
  %4382 = icmp eq i64 %4381, 1200
  br i1 %4382, label %4391, label %4383, !llvm.loop !144

4383:
  %4384 = phi i64 [ 0, %4365 ], [ %4381, %4380 ]
  %4385 = getelementptr inbounds nuw i8, ptr @dst, i64 %4384
  %4386 = load i8, ptr %4385, align 1, !tbaa !10
  %4387 = getelementptr inbounds nuw i8, ptr @ref, i64 %4384
  %4388 = load i8, ptr %4387, align 1, !tbaa !10
  %4389 = icmp eq i8 %4386, %4388
  br i1 %4389, label %4366, label %4390

4390:
  tail call void @report(i64 noundef 40)
  store i1 true, ptr @bad, align 4
  br label %4392

4391:
  br label %4392

4392:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4393

4393:
  %4394 = phi i64 [ 0, %4392 ], [ %4414, %4393 ]
  %4395 = trunc i64 %4394 to i8
  %4396 = mul i8 %4395, 7
  %4397 = or disjoint i8 %4396, 3
  %4398 = getelementptr inbounds nuw i8, ptr @src, i64 %4394
  store i8 %4397, ptr %4398, align 4, !tbaa !10
  %4399 = or disjoint i64 %4394, 1
  %4400 = trunc i64 %4399 to i8
  %4401 = mul i8 %4400, 7
  %4402 = add i8 %4401, 3
  %4403 = getelementptr inbounds nuw i8, ptr @src, i64 %4399
  store i8 %4402, ptr %4403, align 1, !tbaa !10
  %4404 = or disjoint i64 %4394, 2
  %4405 = trunc i64 %4404 to i8
  %4406 = mul i8 %4405, 7
  %4407 = add i8 %4406, 3
  %4408 = getelementptr inbounds nuw i8, ptr @src, i64 %4404
  store i8 %4407, ptr %4408, align 2, !tbaa !10
  %4409 = or disjoint i64 %4394, 3
  %4410 = trunc i64 %4409 to i8
  %4411 = mul i8 %4410, 7
  %4412 = add i8 %4411, 3
  %4413 = getelementptr inbounds nuw i8, ptr @src, i64 %4409
  store i8 %4412, ptr %4413, align 1, !tbaa !10
  %4414 = add nuw nsw i64 %4394, 4
  %4415 = icmp eq i64 %4414, 1200
  br i1 %4415, label %4416, label %4393, !llvm.loop !11

4416:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c740()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 40)
  br label %4434

4417:
  %4418 = add nuw nsw i64 %4435, 1
  %4419 = getelementptr inbounds nuw i8, ptr @dst, i64 %4418
  %4420 = load i8, ptr %4419, align 1, !tbaa !10
  %4421 = getelementptr inbounds nuw i8, ptr @ref, i64 %4418
  %4422 = load i8, ptr %4421, align 1, !tbaa !10
  %4423 = icmp eq i8 %4420, %4422
  br i1 %4423, label %4424, label %4441

4424:
  %4425 = add nuw nsw i64 %4435, 2
  %4426 = getelementptr inbounds nuw i8, ptr @dst, i64 %4425
  %4427 = load i8, ptr %4426, align 1, !tbaa !10
  %4428 = getelementptr inbounds nuw i8, ptr @ref, i64 %4425
  %4429 = load i8, ptr %4428, align 1, !tbaa !10
  %4430 = icmp eq i8 %4427, %4429
  br i1 %4430, label %4431, label %4441

4431:
  %4432 = add nuw nsw i64 %4435, 3
  %4433 = icmp eq i64 %4432, 1200
  br i1 %4433, label %4442, label %4434, !llvm.loop !144

4434:
  %4435 = phi i64 [ 0, %4416 ], [ %4432, %4431 ]
  %4436 = getelementptr inbounds nuw i8, ptr @dst, i64 %4435
  %4437 = load i8, ptr %4436, align 1, !tbaa !10
  %4438 = getelementptr inbounds nuw i8, ptr @ref, i64 %4435
  %4439 = load i8, ptr %4438, align 1, !tbaa !10
  %4440 = icmp eq i8 %4437, %4439
  br i1 %4440, label %4417, label %4441

4441:
  tail call void @report(i64 noundef 40)
  store i1 true, ptr @bad, align 4
  br label %4443

4442:
  br label %4443

4443:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4444

4444:
  %4445 = phi i64 [ 0, %4443 ], [ %4465, %4444 ]
  %4446 = trunc i64 %4445 to i8
  %4447 = mul i8 %4446, 7
  %4448 = or disjoint i8 %4447, 3
  %4449 = getelementptr inbounds nuw i8, ptr @src, i64 %4445
  store i8 %4448, ptr %4449, align 4, !tbaa !10
  %4450 = or disjoint i64 %4445, 1
  %4451 = trunc i64 %4450 to i8
  %4452 = mul i8 %4451, 7
  %4453 = add i8 %4452, 3
  %4454 = getelementptr inbounds nuw i8, ptr @src, i64 %4450
  store i8 %4453, ptr %4454, align 1, !tbaa !10
  %4455 = or disjoint i64 %4445, 2
  %4456 = trunc i64 %4455 to i8
  %4457 = mul i8 %4456, 7
  %4458 = add i8 %4457, 3
  %4459 = getelementptr inbounds nuw i8, ptr @src, i64 %4455
  store i8 %4458, ptr %4459, align 2, !tbaa !10
  %4460 = or disjoint i64 %4445, 3
  %4461 = trunc i64 %4460 to i8
  %4462 = mul i8 %4461, 7
  %4463 = add i8 %4462, 3
  %4464 = getelementptr inbounds nuw i8, ptr @src, i64 %4460
  store i8 %4463, ptr %4464, align 1, !tbaa !10
  %4465 = add nuw nsw i64 %4445, 4
  %4466 = icmp eq i64 %4465, 1200
  br i1 %4466, label %4467, label %4444, !llvm.loop !11

4467:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr40()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 40)
  br label %4485

4468:
  %4469 = add nuw nsw i64 %4486, 1
  %4470 = getelementptr inbounds nuw i8, ptr @dst, i64 %4469
  %4471 = load i8, ptr %4470, align 1, !tbaa !10
  %4472 = getelementptr inbounds nuw i8, ptr @ref, i64 %4469
  %4473 = load i8, ptr %4472, align 1, !tbaa !10
  %4474 = icmp eq i8 %4471, %4473
  br i1 %4474, label %4475, label %4492

4475:
  %4476 = add nuw nsw i64 %4486, 2
  %4477 = getelementptr inbounds nuw i8, ptr @dst, i64 %4476
  %4478 = load i8, ptr %4477, align 1, !tbaa !10
  %4479 = getelementptr inbounds nuw i8, ptr @ref, i64 %4476
  %4480 = load i8, ptr %4479, align 1, !tbaa !10
  %4481 = icmp eq i8 %4478, %4480
  br i1 %4481, label %4482, label %4492

4482:
  %4483 = add nuw nsw i64 %4486, 3
  %4484 = icmp eq i64 %4483, 1200
  br i1 %4484, label %4493, label %4485, !llvm.loop !144

4485:
  %4486 = phi i64 [ 0, %4467 ], [ %4483, %4482 ]
  %4487 = getelementptr inbounds nuw i8, ptr @dst, i64 %4486
  %4488 = load i8, ptr %4487, align 1, !tbaa !10
  %4489 = getelementptr inbounds nuw i8, ptr @ref, i64 %4486
  %4490 = load i8, ptr %4489, align 1, !tbaa !10
  %4491 = icmp eq i8 %4488, %4490
  br i1 %4491, label %4468, label %4492

4492:
  tail call void @report(i64 noundef 40)
  store i1 true, ptr @bad, align 4
  br label %4494

4493:
  br label %4494

4494:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4495

4495:
  %4496 = phi i64 [ 0, %4494 ], [ %4516, %4495 ]
  %4497 = trunc i64 %4496 to i8
  %4498 = mul i8 %4497, 7
  %4499 = or disjoint i8 %4498, 3
  %4500 = getelementptr inbounds nuw i8, ptr @src, i64 %4496
  store i8 %4499, ptr %4500, align 4, !tbaa !10
  %4501 = or disjoint i64 %4496, 1
  %4502 = trunc i64 %4501 to i8
  %4503 = mul i8 %4502, 7
  %4504 = add i8 %4503, 3
  %4505 = getelementptr inbounds nuw i8, ptr @src, i64 %4501
  store i8 %4504, ptr %4505, align 1, !tbaa !10
  %4506 = or disjoint i64 %4496, 2
  %4507 = trunc i64 %4506 to i8
  %4508 = mul i8 %4507, 7
  %4509 = add i8 %4508, 3
  %4510 = getelementptr inbounds nuw i8, ptr @src, i64 %4506
  store i8 %4509, ptr %4510, align 2, !tbaa !10
  %4511 = or disjoint i64 %4496, 3
  %4512 = trunc i64 %4511 to i8
  %4513 = mul i8 %4512, 7
  %4514 = add i8 %4513, 3
  %4515 = getelementptr inbounds nuw i8, ptr @src, i64 %4511
  store i8 %4514, ptr %4515, align 1, !tbaa !10
  %4516 = add nuw nsw i64 %4496, 4
  %4517 = icmp eq i64 %4516, 1200
  br i1 %4517, label %4518, label %4495, !llvm.loop !11

4518:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv40(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 40)
  br label %4536

4519:
  %4520 = add nuw nsw i64 %4537, 1
  %4521 = getelementptr inbounds nuw i8, ptr @dst, i64 %4520
  %4522 = load i8, ptr %4521, align 1, !tbaa !10
  %4523 = getelementptr inbounds nuw i8, ptr @ref, i64 %4520
  %4524 = load i8, ptr %4523, align 1, !tbaa !10
  %4525 = icmp eq i8 %4522, %4524
  br i1 %4525, label %4526, label %4543

4526:
  %4527 = add nuw nsw i64 %4537, 2
  %4528 = getelementptr inbounds nuw i8, ptr @dst, i64 %4527
  %4529 = load i8, ptr %4528, align 1, !tbaa !10
  %4530 = getelementptr inbounds nuw i8, ptr @ref, i64 %4527
  %4531 = load i8, ptr %4530, align 1, !tbaa !10
  %4532 = icmp eq i8 %4529, %4531
  br i1 %4532, label %4533, label %4543

4533:
  %4534 = add nuw nsw i64 %4537, 3
  %4535 = icmp eq i64 %4534, 1200
  br i1 %4535, label %4544, label %4536, !llvm.loop !144

4536:
  %4537 = phi i64 [ 0, %4518 ], [ %4534, %4533 ]
  %4538 = getelementptr inbounds nuw i8, ptr @dst, i64 %4537
  %4539 = load i8, ptr %4538, align 1, !tbaa !10
  %4540 = getelementptr inbounds nuw i8, ptr @ref, i64 %4537
  %4541 = load i8, ptr %4540, align 1, !tbaa !10
  %4542 = icmp eq i8 %4539, %4541
  br i1 %4542, label %4519, label %4543

4543:
  tail call void @report(i64 noundef 40)
  store i1 true, ptr @bad, align 4
  br label %4545

4544:
  br label %4545

4545:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4546

4546:
  %4547 = phi i64 [ 0, %4545 ], [ %4567, %4546 ]
  %4548 = trunc i64 %4547 to i8
  %4549 = mul i8 %4548, 7
  %4550 = or disjoint i8 %4549, 3
  %4551 = getelementptr inbounds nuw i8, ptr @src, i64 %4547
  store i8 %4550, ptr %4551, align 4, !tbaa !10
  %4552 = or disjoint i64 %4547, 1
  %4553 = trunc i64 %4552 to i8
  %4554 = mul i8 %4553, 7
  %4555 = add i8 %4554, 3
  %4556 = getelementptr inbounds nuw i8, ptr @src, i64 %4552
  store i8 %4555, ptr %4556, align 1, !tbaa !10
  %4557 = or disjoint i64 %4547, 2
  %4558 = trunc i64 %4557 to i8
  %4559 = mul i8 %4558, 7
  %4560 = add i8 %4559, 3
  %4561 = getelementptr inbounds nuw i8, ptr @src, i64 %4557
  store i8 %4560, ptr %4561, align 2, !tbaa !10
  %4562 = or disjoint i64 %4547, 3
  %4563 = trunc i64 %4562 to i8
  %4564 = mul i8 %4563, 7
  %4565 = add i8 %4564, 3
  %4566 = getelementptr inbounds nuw i8, ptr @src, i64 %4562
  store i8 %4565, ptr %4566, align 1, !tbaa !10
  %4567 = add nuw nsw i64 %4547, 4
  %4568 = icmp eq i64 %4567, 1200
  br i1 %4568, label %4569, label %4546, !llvm.loop !11

4569:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv40(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 40)
  br label %4587

4570:
  %4571 = add nuw nsw i64 %4588, 1
  %4572 = getelementptr inbounds nuw i8, ptr @dst, i64 %4571
  %4573 = load i8, ptr %4572, align 1, !tbaa !10
  %4574 = getelementptr inbounds nuw i8, ptr @ref, i64 %4571
  %4575 = load i8, ptr %4574, align 1, !tbaa !10
  %4576 = icmp eq i8 %4573, %4575
  br i1 %4576, label %4577, label %4594

4577:
  %4578 = add nuw nsw i64 %4588, 2
  %4579 = getelementptr inbounds nuw i8, ptr @dst, i64 %4578
  %4580 = load i8, ptr %4579, align 1, !tbaa !10
  %4581 = getelementptr inbounds nuw i8, ptr @ref, i64 %4578
  %4582 = load i8, ptr %4581, align 1, !tbaa !10
  %4583 = icmp eq i8 %4580, %4582
  br i1 %4583, label %4584, label %4594

4584:
  %4585 = add nuw nsw i64 %4588, 3
  %4586 = icmp eq i64 %4585, 1200
  br i1 %4586, label %4595, label %4587, !llvm.loop !144

4587:
  %4588 = phi i64 [ 0, %4569 ], [ %4585, %4584 ]
  %4589 = getelementptr inbounds nuw i8, ptr @dst, i64 %4588
  %4590 = load i8, ptr %4589, align 1, !tbaa !10
  %4591 = getelementptr inbounds nuw i8, ptr @ref, i64 %4588
  %4592 = load i8, ptr %4591, align 1, !tbaa !10
  %4593 = icmp eq i8 %4590, %4592
  br i1 %4593, label %4570, label %4594

4594:
  tail call void @report(i64 noundef 40)
  store i1 true, ptr @bad, align 4
  br label %4596

4595:
  br label %4596

4596:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4597

4597:
  %4598 = phi i64 [ 0, %4596 ], [ %4618, %4597 ]
  %4599 = trunc i64 %4598 to i8
  %4600 = mul i8 %4599, 7
  %4601 = or disjoint i8 %4600, 3
  %4602 = getelementptr inbounds nuw i8, ptr @src, i64 %4598
  store i8 %4601, ptr %4602, align 4, !tbaa !10
  %4603 = or disjoint i64 %4598, 1
  %4604 = trunc i64 %4603 to i8
  %4605 = mul i8 %4604, 7
  %4606 = add i8 %4605, 3
  %4607 = getelementptr inbounds nuw i8, ptr @src, i64 %4603
  store i8 %4606, ptr %4607, align 1, !tbaa !10
  %4608 = or disjoint i64 %4598, 2
  %4609 = trunc i64 %4608 to i8
  %4610 = mul i8 %4609, 7
  %4611 = add i8 %4610, 3
  %4612 = getelementptr inbounds nuw i8, ptr @src, i64 %4608
  store i8 %4611, ptr %4612, align 2, !tbaa !10
  %4613 = or disjoint i64 %4598, 3
  %4614 = trunc i64 %4613 to i8
  %4615 = mul i8 %4614, 7
  %4616 = add i8 %4615, 3
  %4617 = getelementptr inbounds nuw i8, ptr @src, i64 %4613
  store i8 %4616, ptr %4617, align 1, !tbaa !10
  %4618 = add nuw nsw i64 %4598, 4
  %4619 = icmp eq i64 %4618, 1200
  br i1 %4619, label %4620, label %4597, !llvm.loop !11

4620:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp63()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 63)
  br label %4638

4621:
  %4622 = add nuw nsw i64 %4639, 1
  %4623 = getelementptr inbounds nuw i8, ptr @dst, i64 %4622
  %4624 = load i8, ptr %4623, align 1, !tbaa !10
  %4625 = getelementptr inbounds nuw i8, ptr @ref, i64 %4622
  %4626 = load i8, ptr %4625, align 1, !tbaa !10
  %4627 = icmp eq i8 %4624, %4626
  br i1 %4627, label %4628, label %4645

4628:
  %4629 = add nuw nsw i64 %4639, 2
  %4630 = getelementptr inbounds nuw i8, ptr @dst, i64 %4629
  %4631 = load i8, ptr %4630, align 1, !tbaa !10
  %4632 = getelementptr inbounds nuw i8, ptr @ref, i64 %4629
  %4633 = load i8, ptr %4632, align 1, !tbaa !10
  %4634 = icmp eq i8 %4631, %4633
  br i1 %4634, label %4635, label %4645

4635:
  %4636 = add nuw nsw i64 %4639, 3
  %4637 = icmp eq i64 %4636, 1200
  br i1 %4637, label %4646, label %4638, !llvm.loop !144

4638:
  %4639 = phi i64 [ 0, %4620 ], [ %4636, %4635 ]
  %4640 = getelementptr inbounds nuw i8, ptr @dst, i64 %4639
  %4641 = load i8, ptr %4640, align 1, !tbaa !10
  %4642 = getelementptr inbounds nuw i8, ptr @ref, i64 %4639
  %4643 = load i8, ptr %4642, align 1, !tbaa !10
  %4644 = icmp eq i8 %4641, %4643
  br i1 %4644, label %4621, label %4645

4645:
  tail call void @report(i64 noundef 63)
  store i1 true, ptr @bad, align 4
  br label %4647

4646:
  br label %4647

4647:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4648

4648:
  %4649 = phi i64 [ 0, %4647 ], [ %4669, %4648 ]
  %4650 = trunc i64 %4649 to i8
  %4651 = mul i8 %4650, 7
  %4652 = or disjoint i8 %4651, 3
  %4653 = getelementptr inbounds nuw i8, ptr @src, i64 %4649
  store i8 %4652, ptr %4653, align 4, !tbaa !10
  %4654 = or disjoint i64 %4649, 1
  %4655 = trunc i64 %4654 to i8
  %4656 = mul i8 %4655, 7
  %4657 = add i8 %4656, 3
  %4658 = getelementptr inbounds nuw i8, ptr @src, i64 %4654
  store i8 %4657, ptr %4658, align 1, !tbaa !10
  %4659 = or disjoint i64 %4649, 2
  %4660 = trunc i64 %4659 to i8
  %4661 = mul i8 %4660, 7
  %4662 = add i8 %4661, 3
  %4663 = getelementptr inbounds nuw i8, ptr @src, i64 %4659
  store i8 %4662, ptr %4663, align 2, !tbaa !10
  %4664 = or disjoint i64 %4649, 3
  %4665 = trunc i64 %4664 to i8
  %4666 = mul i8 %4665, 7
  %4667 = add i8 %4666, 3
  %4668 = getelementptr inbounds nuw i8, ptr @src, i64 %4664
  store i8 %4667, ptr %4668, align 1, !tbaa !10
  %4669 = add nuw nsw i64 %4649, 4
  %4670 = icmp eq i64 %4669, 1200
  br i1 %4670, label %4671, label %4648, !llvm.loop !11

4671:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr63()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 63)
  br label %4689

4672:
  %4673 = add nuw nsw i64 %4690, 1
  %4674 = getelementptr inbounds nuw i8, ptr @dst, i64 %4673
  %4675 = load i8, ptr %4674, align 1, !tbaa !10
  %4676 = getelementptr inbounds nuw i8, ptr @ref, i64 %4673
  %4677 = load i8, ptr %4676, align 1, !tbaa !10
  %4678 = icmp eq i8 %4675, %4677
  br i1 %4678, label %4679, label %4696

4679:
  %4680 = add nuw nsw i64 %4690, 2
  %4681 = getelementptr inbounds nuw i8, ptr @dst, i64 %4680
  %4682 = load i8, ptr %4681, align 1, !tbaa !10
  %4683 = getelementptr inbounds nuw i8, ptr @ref, i64 %4680
  %4684 = load i8, ptr %4683, align 1, !tbaa !10
  %4685 = icmp eq i8 %4682, %4684
  br i1 %4685, label %4686, label %4696

4686:
  %4687 = add nuw nsw i64 %4690, 3
  %4688 = icmp eq i64 %4687, 1200
  br i1 %4688, label %4697, label %4689, !llvm.loop !144

4689:
  %4690 = phi i64 [ 0, %4671 ], [ %4687, %4686 ]
  %4691 = getelementptr inbounds nuw i8, ptr @dst, i64 %4690
  %4692 = load i8, ptr %4691, align 1, !tbaa !10
  %4693 = getelementptr inbounds nuw i8, ptr @ref, i64 %4690
  %4694 = load i8, ptr %4693, align 1, !tbaa !10
  %4695 = icmp eq i8 %4692, %4694
  br i1 %4695, label %4672, label %4696

4696:
  tail call void @report(i64 noundef 63)
  store i1 true, ptr @bad, align 4
  br label %4698

4697:
  br label %4698

4698:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4699

4699:
  %4700 = phi i64 [ 0, %4698 ], [ %4720, %4699 ]
  %4701 = trunc i64 %4700 to i8
  %4702 = mul i8 %4701, 7
  %4703 = or disjoint i8 %4702, 3
  %4704 = getelementptr inbounds nuw i8, ptr @src, i64 %4700
  store i8 %4703, ptr %4704, align 4, !tbaa !10
  %4705 = or disjoint i64 %4700, 1
  %4706 = trunc i64 %4705 to i8
  %4707 = mul i8 %4706, 7
  %4708 = add i8 %4707, 3
  %4709 = getelementptr inbounds nuw i8, ptr @src, i64 %4705
  store i8 %4708, ptr %4709, align 1, !tbaa !10
  %4710 = or disjoint i64 %4700, 2
  %4711 = trunc i64 %4710 to i8
  %4712 = mul i8 %4711, 7
  %4713 = add i8 %4712, 3
  %4714 = getelementptr inbounds nuw i8, ptr @src, i64 %4710
  store i8 %4713, ptr %4714, align 2, !tbaa !10
  %4715 = or disjoint i64 %4700, 3
  %4716 = trunc i64 %4715 to i8
  %4717 = mul i8 %4716, 7
  %4718 = add i8 %4717, 3
  %4719 = getelementptr inbounds nuw i8, ptr @src, i64 %4715
  store i8 %4718, ptr %4719, align 1, !tbaa !10
  %4720 = add nuw nsw i64 %4700, 4
  %4721 = icmp eq i64 %4720, 1200
  br i1 %4721, label %4722, label %4699, !llvm.loop !11

4722:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c763()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 63)
  br label %4740

4723:
  %4724 = add nuw nsw i64 %4741, 1
  %4725 = getelementptr inbounds nuw i8, ptr @dst, i64 %4724
  %4726 = load i8, ptr %4725, align 1, !tbaa !10
  %4727 = getelementptr inbounds nuw i8, ptr @ref, i64 %4724
  %4728 = load i8, ptr %4727, align 1, !tbaa !10
  %4729 = icmp eq i8 %4726, %4728
  br i1 %4729, label %4730, label %4747

4730:
  %4731 = add nuw nsw i64 %4741, 2
  %4732 = getelementptr inbounds nuw i8, ptr @dst, i64 %4731
  %4733 = load i8, ptr %4732, align 1, !tbaa !10
  %4734 = getelementptr inbounds nuw i8, ptr @ref, i64 %4731
  %4735 = load i8, ptr %4734, align 1, !tbaa !10
  %4736 = icmp eq i8 %4733, %4735
  br i1 %4736, label %4737, label %4747

4737:
  %4738 = add nuw nsw i64 %4741, 3
  %4739 = icmp eq i64 %4738, 1200
  br i1 %4739, label %4748, label %4740, !llvm.loop !144

4740:
  %4741 = phi i64 [ 0, %4722 ], [ %4738, %4737 ]
  %4742 = getelementptr inbounds nuw i8, ptr @dst, i64 %4741
  %4743 = load i8, ptr %4742, align 1, !tbaa !10
  %4744 = getelementptr inbounds nuw i8, ptr @ref, i64 %4741
  %4745 = load i8, ptr %4744, align 1, !tbaa !10
  %4746 = icmp eq i8 %4743, %4745
  br i1 %4746, label %4723, label %4747

4747:
  tail call void @report(i64 noundef 63)
  store i1 true, ptr @bad, align 4
  br label %4749

4748:
  br label %4749

4749:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4750

4750:
  %4751 = phi i64 [ 0, %4749 ], [ %4771, %4750 ]
  %4752 = trunc i64 %4751 to i8
  %4753 = mul i8 %4752, 7
  %4754 = or disjoint i8 %4753, 3
  %4755 = getelementptr inbounds nuw i8, ptr @src, i64 %4751
  store i8 %4754, ptr %4755, align 4, !tbaa !10
  %4756 = or disjoint i64 %4751, 1
  %4757 = trunc i64 %4756 to i8
  %4758 = mul i8 %4757, 7
  %4759 = add i8 %4758, 3
  %4760 = getelementptr inbounds nuw i8, ptr @src, i64 %4756
  store i8 %4759, ptr %4760, align 1, !tbaa !10
  %4761 = or disjoint i64 %4751, 2
  %4762 = trunc i64 %4761 to i8
  %4763 = mul i8 %4762, 7
  %4764 = add i8 %4763, 3
  %4765 = getelementptr inbounds nuw i8, ptr @src, i64 %4761
  store i8 %4764, ptr %4765, align 2, !tbaa !10
  %4766 = or disjoint i64 %4751, 3
  %4767 = trunc i64 %4766 to i8
  %4768 = mul i8 %4767, 7
  %4769 = add i8 %4768, 3
  %4770 = getelementptr inbounds nuw i8, ptr @src, i64 %4766
  store i8 %4769, ptr %4770, align 1, !tbaa !10
  %4771 = add nuw nsw i64 %4751, 4
  %4772 = icmp eq i64 %4771, 1200
  br i1 %4772, label %4773, label %4750, !llvm.loop !11

4773:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr63()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 63)
  br label %4791

4774:
  %4775 = add nuw nsw i64 %4792, 1
  %4776 = getelementptr inbounds nuw i8, ptr @dst, i64 %4775
  %4777 = load i8, ptr %4776, align 1, !tbaa !10
  %4778 = getelementptr inbounds nuw i8, ptr @ref, i64 %4775
  %4779 = load i8, ptr %4778, align 1, !tbaa !10
  %4780 = icmp eq i8 %4777, %4779
  br i1 %4780, label %4781, label %4798

4781:
  %4782 = add nuw nsw i64 %4792, 2
  %4783 = getelementptr inbounds nuw i8, ptr @dst, i64 %4782
  %4784 = load i8, ptr %4783, align 1, !tbaa !10
  %4785 = getelementptr inbounds nuw i8, ptr @ref, i64 %4782
  %4786 = load i8, ptr %4785, align 1, !tbaa !10
  %4787 = icmp eq i8 %4784, %4786
  br i1 %4787, label %4788, label %4798

4788:
  %4789 = add nuw nsw i64 %4792, 3
  %4790 = icmp eq i64 %4789, 1200
  br i1 %4790, label %4799, label %4791, !llvm.loop !144

4791:
  %4792 = phi i64 [ 0, %4773 ], [ %4789, %4788 ]
  %4793 = getelementptr inbounds nuw i8, ptr @dst, i64 %4792
  %4794 = load i8, ptr %4793, align 1, !tbaa !10
  %4795 = getelementptr inbounds nuw i8, ptr @ref, i64 %4792
  %4796 = load i8, ptr %4795, align 1, !tbaa !10
  %4797 = icmp eq i8 %4794, %4796
  br i1 %4797, label %4774, label %4798

4798:
  tail call void @report(i64 noundef 63)
  store i1 true, ptr @bad, align 4
  br label %4800

4799:
  br label %4800

4800:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4801

4801:
  %4802 = phi i64 [ 0, %4800 ], [ %4822, %4801 ]
  %4803 = trunc i64 %4802 to i8
  %4804 = mul i8 %4803, 7
  %4805 = or disjoint i8 %4804, 3
  %4806 = getelementptr inbounds nuw i8, ptr @src, i64 %4802
  store i8 %4805, ptr %4806, align 4, !tbaa !10
  %4807 = or disjoint i64 %4802, 1
  %4808 = trunc i64 %4807 to i8
  %4809 = mul i8 %4808, 7
  %4810 = add i8 %4809, 3
  %4811 = getelementptr inbounds nuw i8, ptr @src, i64 %4807
  store i8 %4810, ptr %4811, align 1, !tbaa !10
  %4812 = or disjoint i64 %4802, 2
  %4813 = trunc i64 %4812 to i8
  %4814 = mul i8 %4813, 7
  %4815 = add i8 %4814, 3
  %4816 = getelementptr inbounds nuw i8, ptr @src, i64 %4812
  store i8 %4815, ptr %4816, align 2, !tbaa !10
  %4817 = or disjoint i64 %4802, 3
  %4818 = trunc i64 %4817 to i8
  %4819 = mul i8 %4818, 7
  %4820 = add i8 %4819, 3
  %4821 = getelementptr inbounds nuw i8, ptr @src, i64 %4817
  store i8 %4820, ptr %4821, align 1, !tbaa !10
  %4822 = add nuw nsw i64 %4802, 4
  %4823 = icmp eq i64 %4822, 1200
  br i1 %4823, label %4824, label %4801, !llvm.loop !11

4824:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv63(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 63)
  br label %4842

4825:
  %4826 = add nuw nsw i64 %4843, 1
  %4827 = getelementptr inbounds nuw i8, ptr @dst, i64 %4826
  %4828 = load i8, ptr %4827, align 1, !tbaa !10
  %4829 = getelementptr inbounds nuw i8, ptr @ref, i64 %4826
  %4830 = load i8, ptr %4829, align 1, !tbaa !10
  %4831 = icmp eq i8 %4828, %4830
  br i1 %4831, label %4832, label %4849

4832:
  %4833 = add nuw nsw i64 %4843, 2
  %4834 = getelementptr inbounds nuw i8, ptr @dst, i64 %4833
  %4835 = load i8, ptr %4834, align 1, !tbaa !10
  %4836 = getelementptr inbounds nuw i8, ptr @ref, i64 %4833
  %4837 = load i8, ptr %4836, align 1, !tbaa !10
  %4838 = icmp eq i8 %4835, %4837
  br i1 %4838, label %4839, label %4849

4839:
  %4840 = add nuw nsw i64 %4843, 3
  %4841 = icmp eq i64 %4840, 1200
  br i1 %4841, label %4850, label %4842, !llvm.loop !144

4842:
  %4843 = phi i64 [ 0, %4824 ], [ %4840, %4839 ]
  %4844 = getelementptr inbounds nuw i8, ptr @dst, i64 %4843
  %4845 = load i8, ptr %4844, align 1, !tbaa !10
  %4846 = getelementptr inbounds nuw i8, ptr @ref, i64 %4843
  %4847 = load i8, ptr %4846, align 1, !tbaa !10
  %4848 = icmp eq i8 %4845, %4847
  br i1 %4848, label %4825, label %4849

4849:
  tail call void @report(i64 noundef 63)
  store i1 true, ptr @bad, align 4
  br label %4851

4850:
  br label %4851

4851:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4852

4852:
  %4853 = phi i64 [ 0, %4851 ], [ %4873, %4852 ]
  %4854 = trunc i64 %4853 to i8
  %4855 = mul i8 %4854, 7
  %4856 = or disjoint i8 %4855, 3
  %4857 = getelementptr inbounds nuw i8, ptr @src, i64 %4853
  store i8 %4856, ptr %4857, align 4, !tbaa !10
  %4858 = or disjoint i64 %4853, 1
  %4859 = trunc i64 %4858 to i8
  %4860 = mul i8 %4859, 7
  %4861 = add i8 %4860, 3
  %4862 = getelementptr inbounds nuw i8, ptr @src, i64 %4858
  store i8 %4861, ptr %4862, align 1, !tbaa !10
  %4863 = or disjoint i64 %4853, 2
  %4864 = trunc i64 %4863 to i8
  %4865 = mul i8 %4864, 7
  %4866 = add i8 %4865, 3
  %4867 = getelementptr inbounds nuw i8, ptr @src, i64 %4863
  store i8 %4866, ptr %4867, align 2, !tbaa !10
  %4868 = or disjoint i64 %4853, 3
  %4869 = trunc i64 %4868 to i8
  %4870 = mul i8 %4869, 7
  %4871 = add i8 %4870, 3
  %4872 = getelementptr inbounds nuw i8, ptr @src, i64 %4868
  store i8 %4871, ptr %4872, align 1, !tbaa !10
  %4873 = add nuw nsw i64 %4853, 4
  %4874 = icmp eq i64 %4873, 1200
  br i1 %4874, label %4875, label %4852, !llvm.loop !11

4875:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv63(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 63)
  br label %4893

4876:
  %4877 = add nuw nsw i64 %4894, 1
  %4878 = getelementptr inbounds nuw i8, ptr @dst, i64 %4877
  %4879 = load i8, ptr %4878, align 1, !tbaa !10
  %4880 = getelementptr inbounds nuw i8, ptr @ref, i64 %4877
  %4881 = load i8, ptr %4880, align 1, !tbaa !10
  %4882 = icmp eq i8 %4879, %4881
  br i1 %4882, label %4883, label %4900

4883:
  %4884 = add nuw nsw i64 %4894, 2
  %4885 = getelementptr inbounds nuw i8, ptr @dst, i64 %4884
  %4886 = load i8, ptr %4885, align 1, !tbaa !10
  %4887 = getelementptr inbounds nuw i8, ptr @ref, i64 %4884
  %4888 = load i8, ptr %4887, align 1, !tbaa !10
  %4889 = icmp eq i8 %4886, %4888
  br i1 %4889, label %4890, label %4900

4890:
  %4891 = add nuw nsw i64 %4894, 3
  %4892 = icmp eq i64 %4891, 1200
  br i1 %4892, label %4901, label %4893, !llvm.loop !144

4893:
  %4894 = phi i64 [ 0, %4875 ], [ %4891, %4890 ]
  %4895 = getelementptr inbounds nuw i8, ptr @dst, i64 %4894
  %4896 = load i8, ptr %4895, align 1, !tbaa !10
  %4897 = getelementptr inbounds nuw i8, ptr @ref, i64 %4894
  %4898 = load i8, ptr %4897, align 1, !tbaa !10
  %4899 = icmp eq i8 %4896, %4898
  br i1 %4899, label %4876, label %4900

4900:
  tail call void @report(i64 noundef 63)
  store i1 true, ptr @bad, align 4
  br label %4902

4901:
  br label %4902

4902:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4903

4903:
  %4904 = phi i64 [ 0, %4902 ], [ %4924, %4903 ]
  %4905 = trunc i64 %4904 to i8
  %4906 = mul i8 %4905, 7
  %4907 = or disjoint i8 %4906, 3
  %4908 = getelementptr inbounds nuw i8, ptr @src, i64 %4904
  store i8 %4907, ptr %4908, align 4, !tbaa !10
  %4909 = or disjoint i64 %4904, 1
  %4910 = trunc i64 %4909 to i8
  %4911 = mul i8 %4910, 7
  %4912 = add i8 %4911, 3
  %4913 = getelementptr inbounds nuw i8, ptr @src, i64 %4909
  store i8 %4912, ptr %4913, align 1, !tbaa !10
  %4914 = or disjoint i64 %4904, 2
  %4915 = trunc i64 %4914 to i8
  %4916 = mul i8 %4915, 7
  %4917 = add i8 %4916, 3
  %4918 = getelementptr inbounds nuw i8, ptr @src, i64 %4914
  store i8 %4917, ptr %4918, align 2, !tbaa !10
  %4919 = or disjoint i64 %4904, 3
  %4920 = trunc i64 %4919 to i8
  %4921 = mul i8 %4920, 7
  %4922 = add i8 %4921, 3
  %4923 = getelementptr inbounds nuw i8, ptr @src, i64 %4919
  store i8 %4922, ptr %4923, align 1, !tbaa !10
  %4924 = add nuw nsw i64 %4904, 4
  %4925 = icmp eq i64 %4924, 1200
  br i1 %4925, label %4926, label %4903, !llvm.loop !11

4926:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp64()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 64)
  br label %4944

4927:
  %4928 = add nuw nsw i64 %4945, 1
  %4929 = getelementptr inbounds nuw i8, ptr @dst, i64 %4928
  %4930 = load i8, ptr %4929, align 1, !tbaa !10
  %4931 = getelementptr inbounds nuw i8, ptr @ref, i64 %4928
  %4932 = load i8, ptr %4931, align 1, !tbaa !10
  %4933 = icmp eq i8 %4930, %4932
  br i1 %4933, label %4934, label %4951

4934:
  %4935 = add nuw nsw i64 %4945, 2
  %4936 = getelementptr inbounds nuw i8, ptr @dst, i64 %4935
  %4937 = load i8, ptr %4936, align 1, !tbaa !10
  %4938 = getelementptr inbounds nuw i8, ptr @ref, i64 %4935
  %4939 = load i8, ptr %4938, align 1, !tbaa !10
  %4940 = icmp eq i8 %4937, %4939
  br i1 %4940, label %4941, label %4951

4941:
  %4942 = add nuw nsw i64 %4945, 3
  %4943 = icmp eq i64 %4942, 1200
  br i1 %4943, label %4952, label %4944, !llvm.loop !144

4944:
  %4945 = phi i64 [ 0, %4926 ], [ %4942, %4941 ]
  %4946 = getelementptr inbounds nuw i8, ptr @dst, i64 %4945
  %4947 = load i8, ptr %4946, align 1, !tbaa !10
  %4948 = getelementptr inbounds nuw i8, ptr @ref, i64 %4945
  %4949 = load i8, ptr %4948, align 1, !tbaa !10
  %4950 = icmp eq i8 %4947, %4949
  br i1 %4950, label %4927, label %4951

4951:
  tail call void @report(i64 noundef 64)
  store i1 true, ptr @bad, align 4
  br label %4953

4952:
  br label %4953

4953:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %4954

4954:
  %4955 = phi i64 [ 0, %4953 ], [ %4975, %4954 ]
  %4956 = trunc i64 %4955 to i8
  %4957 = mul i8 %4956, 7
  %4958 = or disjoint i8 %4957, 3
  %4959 = getelementptr inbounds nuw i8, ptr @src, i64 %4955
  store i8 %4958, ptr %4959, align 4, !tbaa !10
  %4960 = or disjoint i64 %4955, 1
  %4961 = trunc i64 %4960 to i8
  %4962 = mul i8 %4961, 7
  %4963 = add i8 %4962, 3
  %4964 = getelementptr inbounds nuw i8, ptr @src, i64 %4960
  store i8 %4963, ptr %4964, align 1, !tbaa !10
  %4965 = or disjoint i64 %4955, 2
  %4966 = trunc i64 %4965 to i8
  %4967 = mul i8 %4966, 7
  %4968 = add i8 %4967, 3
  %4969 = getelementptr inbounds nuw i8, ptr @src, i64 %4965
  store i8 %4968, ptr %4969, align 2, !tbaa !10
  %4970 = or disjoint i64 %4955, 3
  %4971 = trunc i64 %4970 to i8
  %4972 = mul i8 %4971, 7
  %4973 = add i8 %4972, 3
  %4974 = getelementptr inbounds nuw i8, ptr @src, i64 %4970
  store i8 %4973, ptr %4974, align 1, !tbaa !10
  %4975 = add nuw nsw i64 %4955, 4
  %4976 = icmp eq i64 %4975, 1200
  br i1 %4976, label %4977, label %4954, !llvm.loop !11

4977:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr64()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 64)
  br label %4995

4978:
  %4979 = add nuw nsw i64 %4996, 1
  %4980 = getelementptr inbounds nuw i8, ptr @dst, i64 %4979
  %4981 = load i8, ptr %4980, align 1, !tbaa !10
  %4982 = getelementptr inbounds nuw i8, ptr @ref, i64 %4979
  %4983 = load i8, ptr %4982, align 1, !tbaa !10
  %4984 = icmp eq i8 %4981, %4983
  br i1 %4984, label %4985, label %5002

4985:
  %4986 = add nuw nsw i64 %4996, 2
  %4987 = getelementptr inbounds nuw i8, ptr @dst, i64 %4986
  %4988 = load i8, ptr %4987, align 1, !tbaa !10
  %4989 = getelementptr inbounds nuw i8, ptr @ref, i64 %4986
  %4990 = load i8, ptr %4989, align 1, !tbaa !10
  %4991 = icmp eq i8 %4988, %4990
  br i1 %4991, label %4992, label %5002

4992:
  %4993 = add nuw nsw i64 %4996, 3
  %4994 = icmp eq i64 %4993, 1200
  br i1 %4994, label %5003, label %4995, !llvm.loop !144

4995:
  %4996 = phi i64 [ 0, %4977 ], [ %4993, %4992 ]
  %4997 = getelementptr inbounds nuw i8, ptr @dst, i64 %4996
  %4998 = load i8, ptr %4997, align 1, !tbaa !10
  %4999 = getelementptr inbounds nuw i8, ptr @ref, i64 %4996
  %5000 = load i8, ptr %4999, align 1, !tbaa !10
  %5001 = icmp eq i8 %4998, %5000
  br i1 %5001, label %4978, label %5002

5002:
  tail call void @report(i64 noundef 64)
  store i1 true, ptr @bad, align 4
  br label %5004

5003:
  br label %5004

5004:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5005

5005:
  %5006 = phi i64 [ 0, %5004 ], [ %5026, %5005 ]
  %5007 = trunc i64 %5006 to i8
  %5008 = mul i8 %5007, 7
  %5009 = or disjoint i8 %5008, 3
  %5010 = getelementptr inbounds nuw i8, ptr @src, i64 %5006
  store i8 %5009, ptr %5010, align 4, !tbaa !10
  %5011 = or disjoint i64 %5006, 1
  %5012 = trunc i64 %5011 to i8
  %5013 = mul i8 %5012, 7
  %5014 = add i8 %5013, 3
  %5015 = getelementptr inbounds nuw i8, ptr @src, i64 %5011
  store i8 %5014, ptr %5015, align 1, !tbaa !10
  %5016 = or disjoint i64 %5006, 2
  %5017 = trunc i64 %5016 to i8
  %5018 = mul i8 %5017, 7
  %5019 = add i8 %5018, 3
  %5020 = getelementptr inbounds nuw i8, ptr @src, i64 %5016
  store i8 %5019, ptr %5020, align 2, !tbaa !10
  %5021 = or disjoint i64 %5006, 3
  %5022 = trunc i64 %5021 to i8
  %5023 = mul i8 %5022, 7
  %5024 = add i8 %5023, 3
  %5025 = getelementptr inbounds nuw i8, ptr @src, i64 %5021
  store i8 %5024, ptr %5025, align 1, !tbaa !10
  %5026 = add nuw nsw i64 %5006, 4
  %5027 = icmp eq i64 %5026, 1200
  br i1 %5027, label %5028, label %5005, !llvm.loop !11

5028:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c764()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 64)
  br label %5046

5029:
  %5030 = add nuw nsw i64 %5047, 1
  %5031 = getelementptr inbounds nuw i8, ptr @dst, i64 %5030
  %5032 = load i8, ptr %5031, align 1, !tbaa !10
  %5033 = getelementptr inbounds nuw i8, ptr @ref, i64 %5030
  %5034 = load i8, ptr %5033, align 1, !tbaa !10
  %5035 = icmp eq i8 %5032, %5034
  br i1 %5035, label %5036, label %5053

5036:
  %5037 = add nuw nsw i64 %5047, 2
  %5038 = getelementptr inbounds nuw i8, ptr @dst, i64 %5037
  %5039 = load i8, ptr %5038, align 1, !tbaa !10
  %5040 = getelementptr inbounds nuw i8, ptr @ref, i64 %5037
  %5041 = load i8, ptr %5040, align 1, !tbaa !10
  %5042 = icmp eq i8 %5039, %5041
  br i1 %5042, label %5043, label %5053

5043:
  %5044 = add nuw nsw i64 %5047, 3
  %5045 = icmp eq i64 %5044, 1200
  br i1 %5045, label %5054, label %5046, !llvm.loop !144

5046:
  %5047 = phi i64 [ 0, %5028 ], [ %5044, %5043 ]
  %5048 = getelementptr inbounds nuw i8, ptr @dst, i64 %5047
  %5049 = load i8, ptr %5048, align 1, !tbaa !10
  %5050 = getelementptr inbounds nuw i8, ptr @ref, i64 %5047
  %5051 = load i8, ptr %5050, align 1, !tbaa !10
  %5052 = icmp eq i8 %5049, %5051
  br i1 %5052, label %5029, label %5053

5053:
  tail call void @report(i64 noundef 64)
  store i1 true, ptr @bad, align 4
  br label %5055

5054:
  br label %5055

5055:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5056

5056:
  %5057 = phi i64 [ 0, %5055 ], [ %5077, %5056 ]
  %5058 = trunc i64 %5057 to i8
  %5059 = mul i8 %5058, 7
  %5060 = or disjoint i8 %5059, 3
  %5061 = getelementptr inbounds nuw i8, ptr @src, i64 %5057
  store i8 %5060, ptr %5061, align 4, !tbaa !10
  %5062 = or disjoint i64 %5057, 1
  %5063 = trunc i64 %5062 to i8
  %5064 = mul i8 %5063, 7
  %5065 = add i8 %5064, 3
  %5066 = getelementptr inbounds nuw i8, ptr @src, i64 %5062
  store i8 %5065, ptr %5066, align 1, !tbaa !10
  %5067 = or disjoint i64 %5057, 2
  %5068 = trunc i64 %5067 to i8
  %5069 = mul i8 %5068, 7
  %5070 = add i8 %5069, 3
  %5071 = getelementptr inbounds nuw i8, ptr @src, i64 %5067
  store i8 %5070, ptr %5071, align 2, !tbaa !10
  %5072 = or disjoint i64 %5057, 3
  %5073 = trunc i64 %5072 to i8
  %5074 = mul i8 %5073, 7
  %5075 = add i8 %5074, 3
  %5076 = getelementptr inbounds nuw i8, ptr @src, i64 %5072
  store i8 %5075, ptr %5076, align 1, !tbaa !10
  %5077 = add nuw nsw i64 %5057, 4
  %5078 = icmp eq i64 %5077, 1200
  br i1 %5078, label %5079, label %5056, !llvm.loop !11

5079:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr64()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 64)
  br label %5097

5080:
  %5081 = add nuw nsw i64 %5098, 1
  %5082 = getelementptr inbounds nuw i8, ptr @dst, i64 %5081
  %5083 = load i8, ptr %5082, align 1, !tbaa !10
  %5084 = getelementptr inbounds nuw i8, ptr @ref, i64 %5081
  %5085 = load i8, ptr %5084, align 1, !tbaa !10
  %5086 = icmp eq i8 %5083, %5085
  br i1 %5086, label %5087, label %5104

5087:
  %5088 = add nuw nsw i64 %5098, 2
  %5089 = getelementptr inbounds nuw i8, ptr @dst, i64 %5088
  %5090 = load i8, ptr %5089, align 1, !tbaa !10
  %5091 = getelementptr inbounds nuw i8, ptr @ref, i64 %5088
  %5092 = load i8, ptr %5091, align 1, !tbaa !10
  %5093 = icmp eq i8 %5090, %5092
  br i1 %5093, label %5094, label %5104

5094:
  %5095 = add nuw nsw i64 %5098, 3
  %5096 = icmp eq i64 %5095, 1200
  br i1 %5096, label %5105, label %5097, !llvm.loop !144

5097:
  %5098 = phi i64 [ 0, %5079 ], [ %5095, %5094 ]
  %5099 = getelementptr inbounds nuw i8, ptr @dst, i64 %5098
  %5100 = load i8, ptr %5099, align 1, !tbaa !10
  %5101 = getelementptr inbounds nuw i8, ptr @ref, i64 %5098
  %5102 = load i8, ptr %5101, align 1, !tbaa !10
  %5103 = icmp eq i8 %5100, %5102
  br i1 %5103, label %5080, label %5104

5104:
  tail call void @report(i64 noundef 64)
  store i1 true, ptr @bad, align 4
  br label %5106

5105:
  br label %5106

5106:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5107

5107:
  %5108 = phi i64 [ 0, %5106 ], [ %5128, %5107 ]
  %5109 = trunc i64 %5108 to i8
  %5110 = mul i8 %5109, 7
  %5111 = or disjoint i8 %5110, 3
  %5112 = getelementptr inbounds nuw i8, ptr @src, i64 %5108
  store i8 %5111, ptr %5112, align 4, !tbaa !10
  %5113 = or disjoint i64 %5108, 1
  %5114 = trunc i64 %5113 to i8
  %5115 = mul i8 %5114, 7
  %5116 = add i8 %5115, 3
  %5117 = getelementptr inbounds nuw i8, ptr @src, i64 %5113
  store i8 %5116, ptr %5117, align 1, !tbaa !10
  %5118 = or disjoint i64 %5108, 2
  %5119 = trunc i64 %5118 to i8
  %5120 = mul i8 %5119, 7
  %5121 = add i8 %5120, 3
  %5122 = getelementptr inbounds nuw i8, ptr @src, i64 %5118
  store i8 %5121, ptr %5122, align 2, !tbaa !10
  %5123 = or disjoint i64 %5108, 3
  %5124 = trunc i64 %5123 to i8
  %5125 = mul i8 %5124, 7
  %5126 = add i8 %5125, 3
  %5127 = getelementptr inbounds nuw i8, ptr @src, i64 %5123
  store i8 %5126, ptr %5127, align 1, !tbaa !10
  %5128 = add nuw nsw i64 %5108, 4
  %5129 = icmp eq i64 %5128, 1200
  br i1 %5129, label %5130, label %5107, !llvm.loop !11

5130:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv64(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 64)
  br label %5148

5131:
  %5132 = add nuw nsw i64 %5149, 1
  %5133 = getelementptr inbounds nuw i8, ptr @dst, i64 %5132
  %5134 = load i8, ptr %5133, align 1, !tbaa !10
  %5135 = getelementptr inbounds nuw i8, ptr @ref, i64 %5132
  %5136 = load i8, ptr %5135, align 1, !tbaa !10
  %5137 = icmp eq i8 %5134, %5136
  br i1 %5137, label %5138, label %5155

5138:
  %5139 = add nuw nsw i64 %5149, 2
  %5140 = getelementptr inbounds nuw i8, ptr @dst, i64 %5139
  %5141 = load i8, ptr %5140, align 1, !tbaa !10
  %5142 = getelementptr inbounds nuw i8, ptr @ref, i64 %5139
  %5143 = load i8, ptr %5142, align 1, !tbaa !10
  %5144 = icmp eq i8 %5141, %5143
  br i1 %5144, label %5145, label %5155

5145:
  %5146 = add nuw nsw i64 %5149, 3
  %5147 = icmp eq i64 %5146, 1200
  br i1 %5147, label %5156, label %5148, !llvm.loop !144

5148:
  %5149 = phi i64 [ 0, %5130 ], [ %5146, %5145 ]
  %5150 = getelementptr inbounds nuw i8, ptr @dst, i64 %5149
  %5151 = load i8, ptr %5150, align 1, !tbaa !10
  %5152 = getelementptr inbounds nuw i8, ptr @ref, i64 %5149
  %5153 = load i8, ptr %5152, align 1, !tbaa !10
  %5154 = icmp eq i8 %5151, %5153
  br i1 %5154, label %5131, label %5155

5155:
  tail call void @report(i64 noundef 64)
  store i1 true, ptr @bad, align 4
  br label %5157

5156:
  br label %5157

5157:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5158

5158:
  %5159 = phi i64 [ 0, %5157 ], [ %5179, %5158 ]
  %5160 = trunc i64 %5159 to i8
  %5161 = mul i8 %5160, 7
  %5162 = or disjoint i8 %5161, 3
  %5163 = getelementptr inbounds nuw i8, ptr @src, i64 %5159
  store i8 %5162, ptr %5163, align 4, !tbaa !10
  %5164 = or disjoint i64 %5159, 1
  %5165 = trunc i64 %5164 to i8
  %5166 = mul i8 %5165, 7
  %5167 = add i8 %5166, 3
  %5168 = getelementptr inbounds nuw i8, ptr @src, i64 %5164
  store i8 %5167, ptr %5168, align 1, !tbaa !10
  %5169 = or disjoint i64 %5159, 2
  %5170 = trunc i64 %5169 to i8
  %5171 = mul i8 %5170, 7
  %5172 = add i8 %5171, 3
  %5173 = getelementptr inbounds nuw i8, ptr @src, i64 %5169
  store i8 %5172, ptr %5173, align 2, !tbaa !10
  %5174 = or disjoint i64 %5159, 3
  %5175 = trunc i64 %5174 to i8
  %5176 = mul i8 %5175, 7
  %5177 = add i8 %5176, 3
  %5178 = getelementptr inbounds nuw i8, ptr @src, i64 %5174
  store i8 %5177, ptr %5178, align 1, !tbaa !10
  %5179 = add nuw nsw i64 %5159, 4
  %5180 = icmp eq i64 %5179, 1200
  br i1 %5180, label %5181, label %5158, !llvm.loop !11

5181:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv64(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 64)
  br label %5199

5182:
  %5183 = add nuw nsw i64 %5200, 1
  %5184 = getelementptr inbounds nuw i8, ptr @dst, i64 %5183
  %5185 = load i8, ptr %5184, align 1, !tbaa !10
  %5186 = getelementptr inbounds nuw i8, ptr @ref, i64 %5183
  %5187 = load i8, ptr %5186, align 1, !tbaa !10
  %5188 = icmp eq i8 %5185, %5187
  br i1 %5188, label %5189, label %5206

5189:
  %5190 = add nuw nsw i64 %5200, 2
  %5191 = getelementptr inbounds nuw i8, ptr @dst, i64 %5190
  %5192 = load i8, ptr %5191, align 1, !tbaa !10
  %5193 = getelementptr inbounds nuw i8, ptr @ref, i64 %5190
  %5194 = load i8, ptr %5193, align 1, !tbaa !10
  %5195 = icmp eq i8 %5192, %5194
  br i1 %5195, label %5196, label %5206

5196:
  %5197 = add nuw nsw i64 %5200, 3
  %5198 = icmp eq i64 %5197, 1200
  br i1 %5198, label %5207, label %5199, !llvm.loop !144

5199:
  %5200 = phi i64 [ 0, %5181 ], [ %5197, %5196 ]
  %5201 = getelementptr inbounds nuw i8, ptr @dst, i64 %5200
  %5202 = load i8, ptr %5201, align 1, !tbaa !10
  %5203 = getelementptr inbounds nuw i8, ptr @ref, i64 %5200
  %5204 = load i8, ptr %5203, align 1, !tbaa !10
  %5205 = icmp eq i8 %5202, %5204
  br i1 %5205, label %5182, label %5206

5206:
  tail call void @report(i64 noundef 64)
  store i1 true, ptr @bad, align 4
  br label %5208

5207:
  br label %5208

5208:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5209

5209:
  %5210 = phi i64 [ 0, %5208 ], [ %5230, %5209 ]
  %5211 = trunc i64 %5210 to i8
  %5212 = mul i8 %5211, 7
  %5213 = or disjoint i8 %5212, 3
  %5214 = getelementptr inbounds nuw i8, ptr @src, i64 %5210
  store i8 %5213, ptr %5214, align 4, !tbaa !10
  %5215 = or disjoint i64 %5210, 1
  %5216 = trunc i64 %5215 to i8
  %5217 = mul i8 %5216, 7
  %5218 = add i8 %5217, 3
  %5219 = getelementptr inbounds nuw i8, ptr @src, i64 %5215
  store i8 %5218, ptr %5219, align 1, !tbaa !10
  %5220 = or disjoint i64 %5210, 2
  %5221 = trunc i64 %5220 to i8
  %5222 = mul i8 %5221, 7
  %5223 = add i8 %5222, 3
  %5224 = getelementptr inbounds nuw i8, ptr @src, i64 %5220
  store i8 %5223, ptr %5224, align 2, !tbaa !10
  %5225 = or disjoint i64 %5210, 3
  %5226 = trunc i64 %5225 to i8
  %5227 = mul i8 %5226, 7
  %5228 = add i8 %5227, 3
  %5229 = getelementptr inbounds nuw i8, ptr @src, i64 %5225
  store i8 %5228, ptr %5229, align 1, !tbaa !10
  %5230 = add nuw nsw i64 %5210, 4
  %5231 = icmp eq i64 %5230, 1200
  br i1 %5231, label %5232, label %5209, !llvm.loop !11

5232:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp65()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 65)
  br label %5250

5233:
  %5234 = add nuw nsw i64 %5251, 1
  %5235 = getelementptr inbounds nuw i8, ptr @dst, i64 %5234
  %5236 = load i8, ptr %5235, align 1, !tbaa !10
  %5237 = getelementptr inbounds nuw i8, ptr @ref, i64 %5234
  %5238 = load i8, ptr %5237, align 1, !tbaa !10
  %5239 = icmp eq i8 %5236, %5238
  br i1 %5239, label %5240, label %5257

5240:
  %5241 = add nuw nsw i64 %5251, 2
  %5242 = getelementptr inbounds nuw i8, ptr @dst, i64 %5241
  %5243 = load i8, ptr %5242, align 1, !tbaa !10
  %5244 = getelementptr inbounds nuw i8, ptr @ref, i64 %5241
  %5245 = load i8, ptr %5244, align 1, !tbaa !10
  %5246 = icmp eq i8 %5243, %5245
  br i1 %5246, label %5247, label %5257

5247:
  %5248 = add nuw nsw i64 %5251, 3
  %5249 = icmp eq i64 %5248, 1200
  br i1 %5249, label %5258, label %5250, !llvm.loop !144

5250:
  %5251 = phi i64 [ 0, %5232 ], [ %5248, %5247 ]
  %5252 = getelementptr inbounds nuw i8, ptr @dst, i64 %5251
  %5253 = load i8, ptr %5252, align 1, !tbaa !10
  %5254 = getelementptr inbounds nuw i8, ptr @ref, i64 %5251
  %5255 = load i8, ptr %5254, align 1, !tbaa !10
  %5256 = icmp eq i8 %5253, %5255
  br i1 %5256, label %5233, label %5257

5257:
  tail call void @report(i64 noundef 65)
  store i1 true, ptr @bad, align 4
  br label %5259

5258:
  br label %5259

5259:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5260

5260:
  %5261 = phi i64 [ 0, %5259 ], [ %5281, %5260 ]
  %5262 = trunc i64 %5261 to i8
  %5263 = mul i8 %5262, 7
  %5264 = or disjoint i8 %5263, 3
  %5265 = getelementptr inbounds nuw i8, ptr @src, i64 %5261
  store i8 %5264, ptr %5265, align 4, !tbaa !10
  %5266 = or disjoint i64 %5261, 1
  %5267 = trunc i64 %5266 to i8
  %5268 = mul i8 %5267, 7
  %5269 = add i8 %5268, 3
  %5270 = getelementptr inbounds nuw i8, ptr @src, i64 %5266
  store i8 %5269, ptr %5270, align 1, !tbaa !10
  %5271 = or disjoint i64 %5261, 2
  %5272 = trunc i64 %5271 to i8
  %5273 = mul i8 %5272, 7
  %5274 = add i8 %5273, 3
  %5275 = getelementptr inbounds nuw i8, ptr @src, i64 %5271
  store i8 %5274, ptr %5275, align 2, !tbaa !10
  %5276 = or disjoint i64 %5261, 3
  %5277 = trunc i64 %5276 to i8
  %5278 = mul i8 %5277, 7
  %5279 = add i8 %5278, 3
  %5280 = getelementptr inbounds nuw i8, ptr @src, i64 %5276
  store i8 %5279, ptr %5280, align 1, !tbaa !10
  %5281 = add nuw nsw i64 %5261, 4
  %5282 = icmp eq i64 %5281, 1200
  br i1 %5282, label %5283, label %5260, !llvm.loop !11

5283:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr65()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 65)
  br label %5301

5284:
  %5285 = add nuw nsw i64 %5302, 1
  %5286 = getelementptr inbounds nuw i8, ptr @dst, i64 %5285
  %5287 = load i8, ptr %5286, align 1, !tbaa !10
  %5288 = getelementptr inbounds nuw i8, ptr @ref, i64 %5285
  %5289 = load i8, ptr %5288, align 1, !tbaa !10
  %5290 = icmp eq i8 %5287, %5289
  br i1 %5290, label %5291, label %5308

5291:
  %5292 = add nuw nsw i64 %5302, 2
  %5293 = getelementptr inbounds nuw i8, ptr @dst, i64 %5292
  %5294 = load i8, ptr %5293, align 1, !tbaa !10
  %5295 = getelementptr inbounds nuw i8, ptr @ref, i64 %5292
  %5296 = load i8, ptr %5295, align 1, !tbaa !10
  %5297 = icmp eq i8 %5294, %5296
  br i1 %5297, label %5298, label %5308

5298:
  %5299 = add nuw nsw i64 %5302, 3
  %5300 = icmp eq i64 %5299, 1200
  br i1 %5300, label %5309, label %5301, !llvm.loop !144

5301:
  %5302 = phi i64 [ 0, %5283 ], [ %5299, %5298 ]
  %5303 = getelementptr inbounds nuw i8, ptr @dst, i64 %5302
  %5304 = load i8, ptr %5303, align 1, !tbaa !10
  %5305 = getelementptr inbounds nuw i8, ptr @ref, i64 %5302
  %5306 = load i8, ptr %5305, align 1, !tbaa !10
  %5307 = icmp eq i8 %5304, %5306
  br i1 %5307, label %5284, label %5308

5308:
  tail call void @report(i64 noundef 65)
  store i1 true, ptr @bad, align 4
  br label %5310

5309:
  br label %5310

5310:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5311

5311:
  %5312 = phi i64 [ 0, %5310 ], [ %5332, %5311 ]
  %5313 = trunc i64 %5312 to i8
  %5314 = mul i8 %5313, 7
  %5315 = or disjoint i8 %5314, 3
  %5316 = getelementptr inbounds nuw i8, ptr @src, i64 %5312
  store i8 %5315, ptr %5316, align 4, !tbaa !10
  %5317 = or disjoint i64 %5312, 1
  %5318 = trunc i64 %5317 to i8
  %5319 = mul i8 %5318, 7
  %5320 = add i8 %5319, 3
  %5321 = getelementptr inbounds nuw i8, ptr @src, i64 %5317
  store i8 %5320, ptr %5321, align 1, !tbaa !10
  %5322 = or disjoint i64 %5312, 2
  %5323 = trunc i64 %5322 to i8
  %5324 = mul i8 %5323, 7
  %5325 = add i8 %5324, 3
  %5326 = getelementptr inbounds nuw i8, ptr @src, i64 %5322
  store i8 %5325, ptr %5326, align 2, !tbaa !10
  %5327 = or disjoint i64 %5312, 3
  %5328 = trunc i64 %5327 to i8
  %5329 = mul i8 %5328, 7
  %5330 = add i8 %5329, 3
  %5331 = getelementptr inbounds nuw i8, ptr @src, i64 %5327
  store i8 %5330, ptr %5331, align 1, !tbaa !10
  %5332 = add nuw nsw i64 %5312, 4
  %5333 = icmp eq i64 %5332, 1200
  br i1 %5333, label %5334, label %5311, !llvm.loop !11

5334:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c765()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 65)
  br label %5352

5335:
  %5336 = add nuw nsw i64 %5353, 1
  %5337 = getelementptr inbounds nuw i8, ptr @dst, i64 %5336
  %5338 = load i8, ptr %5337, align 1, !tbaa !10
  %5339 = getelementptr inbounds nuw i8, ptr @ref, i64 %5336
  %5340 = load i8, ptr %5339, align 1, !tbaa !10
  %5341 = icmp eq i8 %5338, %5340
  br i1 %5341, label %5342, label %5359

5342:
  %5343 = add nuw nsw i64 %5353, 2
  %5344 = getelementptr inbounds nuw i8, ptr @dst, i64 %5343
  %5345 = load i8, ptr %5344, align 1, !tbaa !10
  %5346 = getelementptr inbounds nuw i8, ptr @ref, i64 %5343
  %5347 = load i8, ptr %5346, align 1, !tbaa !10
  %5348 = icmp eq i8 %5345, %5347
  br i1 %5348, label %5349, label %5359

5349:
  %5350 = add nuw nsw i64 %5353, 3
  %5351 = icmp eq i64 %5350, 1200
  br i1 %5351, label %5360, label %5352, !llvm.loop !144

5352:
  %5353 = phi i64 [ 0, %5334 ], [ %5350, %5349 ]
  %5354 = getelementptr inbounds nuw i8, ptr @dst, i64 %5353
  %5355 = load i8, ptr %5354, align 1, !tbaa !10
  %5356 = getelementptr inbounds nuw i8, ptr @ref, i64 %5353
  %5357 = load i8, ptr %5356, align 1, !tbaa !10
  %5358 = icmp eq i8 %5355, %5357
  br i1 %5358, label %5335, label %5359

5359:
  tail call void @report(i64 noundef 65)
  store i1 true, ptr @bad, align 4
  br label %5361

5360:
  br label %5361

5361:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5362

5362:
  %5363 = phi i64 [ 0, %5361 ], [ %5383, %5362 ]
  %5364 = trunc i64 %5363 to i8
  %5365 = mul i8 %5364, 7
  %5366 = or disjoint i8 %5365, 3
  %5367 = getelementptr inbounds nuw i8, ptr @src, i64 %5363
  store i8 %5366, ptr %5367, align 4, !tbaa !10
  %5368 = or disjoint i64 %5363, 1
  %5369 = trunc i64 %5368 to i8
  %5370 = mul i8 %5369, 7
  %5371 = add i8 %5370, 3
  %5372 = getelementptr inbounds nuw i8, ptr @src, i64 %5368
  store i8 %5371, ptr %5372, align 1, !tbaa !10
  %5373 = or disjoint i64 %5363, 2
  %5374 = trunc i64 %5373 to i8
  %5375 = mul i8 %5374, 7
  %5376 = add i8 %5375, 3
  %5377 = getelementptr inbounds nuw i8, ptr @src, i64 %5373
  store i8 %5376, ptr %5377, align 2, !tbaa !10
  %5378 = or disjoint i64 %5363, 3
  %5379 = trunc i64 %5378 to i8
  %5380 = mul i8 %5379, 7
  %5381 = add i8 %5380, 3
  %5382 = getelementptr inbounds nuw i8, ptr @src, i64 %5378
  store i8 %5381, ptr %5382, align 1, !tbaa !10
  %5383 = add nuw nsw i64 %5363, 4
  %5384 = icmp eq i64 %5383, 1200
  br i1 %5384, label %5385, label %5362, !llvm.loop !11

5385:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr65()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 65)
  br label %5403

5386:
  %5387 = add nuw nsw i64 %5404, 1
  %5388 = getelementptr inbounds nuw i8, ptr @dst, i64 %5387
  %5389 = load i8, ptr %5388, align 1, !tbaa !10
  %5390 = getelementptr inbounds nuw i8, ptr @ref, i64 %5387
  %5391 = load i8, ptr %5390, align 1, !tbaa !10
  %5392 = icmp eq i8 %5389, %5391
  br i1 %5392, label %5393, label %5410

5393:
  %5394 = add nuw nsw i64 %5404, 2
  %5395 = getelementptr inbounds nuw i8, ptr @dst, i64 %5394
  %5396 = load i8, ptr %5395, align 1, !tbaa !10
  %5397 = getelementptr inbounds nuw i8, ptr @ref, i64 %5394
  %5398 = load i8, ptr %5397, align 1, !tbaa !10
  %5399 = icmp eq i8 %5396, %5398
  br i1 %5399, label %5400, label %5410

5400:
  %5401 = add nuw nsw i64 %5404, 3
  %5402 = icmp eq i64 %5401, 1200
  br i1 %5402, label %5411, label %5403, !llvm.loop !144

5403:
  %5404 = phi i64 [ 0, %5385 ], [ %5401, %5400 ]
  %5405 = getelementptr inbounds nuw i8, ptr @dst, i64 %5404
  %5406 = load i8, ptr %5405, align 1, !tbaa !10
  %5407 = getelementptr inbounds nuw i8, ptr @ref, i64 %5404
  %5408 = load i8, ptr %5407, align 1, !tbaa !10
  %5409 = icmp eq i8 %5406, %5408
  br i1 %5409, label %5386, label %5410

5410:
  tail call void @report(i64 noundef 65)
  store i1 true, ptr @bad, align 4
  br label %5412

5411:
  br label %5412

5412:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5413

5413:
  %5414 = phi i64 [ 0, %5412 ], [ %5434, %5413 ]
  %5415 = trunc i64 %5414 to i8
  %5416 = mul i8 %5415, 7
  %5417 = or disjoint i8 %5416, 3
  %5418 = getelementptr inbounds nuw i8, ptr @src, i64 %5414
  store i8 %5417, ptr %5418, align 4, !tbaa !10
  %5419 = or disjoint i64 %5414, 1
  %5420 = trunc i64 %5419 to i8
  %5421 = mul i8 %5420, 7
  %5422 = add i8 %5421, 3
  %5423 = getelementptr inbounds nuw i8, ptr @src, i64 %5419
  store i8 %5422, ptr %5423, align 1, !tbaa !10
  %5424 = or disjoint i64 %5414, 2
  %5425 = trunc i64 %5424 to i8
  %5426 = mul i8 %5425, 7
  %5427 = add i8 %5426, 3
  %5428 = getelementptr inbounds nuw i8, ptr @src, i64 %5424
  store i8 %5427, ptr %5428, align 2, !tbaa !10
  %5429 = or disjoint i64 %5414, 3
  %5430 = trunc i64 %5429 to i8
  %5431 = mul i8 %5430, 7
  %5432 = add i8 %5431, 3
  %5433 = getelementptr inbounds nuw i8, ptr @src, i64 %5429
  store i8 %5432, ptr %5433, align 1, !tbaa !10
  %5434 = add nuw nsw i64 %5414, 4
  %5435 = icmp eq i64 %5434, 1200
  br i1 %5435, label %5436, label %5413, !llvm.loop !11

5436:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv65(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 65)
  br label %5454

5437:
  %5438 = add nuw nsw i64 %5455, 1
  %5439 = getelementptr inbounds nuw i8, ptr @dst, i64 %5438
  %5440 = load i8, ptr %5439, align 1, !tbaa !10
  %5441 = getelementptr inbounds nuw i8, ptr @ref, i64 %5438
  %5442 = load i8, ptr %5441, align 1, !tbaa !10
  %5443 = icmp eq i8 %5440, %5442
  br i1 %5443, label %5444, label %5461

5444:
  %5445 = add nuw nsw i64 %5455, 2
  %5446 = getelementptr inbounds nuw i8, ptr @dst, i64 %5445
  %5447 = load i8, ptr %5446, align 1, !tbaa !10
  %5448 = getelementptr inbounds nuw i8, ptr @ref, i64 %5445
  %5449 = load i8, ptr %5448, align 1, !tbaa !10
  %5450 = icmp eq i8 %5447, %5449
  br i1 %5450, label %5451, label %5461

5451:
  %5452 = add nuw nsw i64 %5455, 3
  %5453 = icmp eq i64 %5452, 1200
  br i1 %5453, label %5462, label %5454, !llvm.loop !144

5454:
  %5455 = phi i64 [ 0, %5436 ], [ %5452, %5451 ]
  %5456 = getelementptr inbounds nuw i8, ptr @dst, i64 %5455
  %5457 = load i8, ptr %5456, align 1, !tbaa !10
  %5458 = getelementptr inbounds nuw i8, ptr @ref, i64 %5455
  %5459 = load i8, ptr %5458, align 1, !tbaa !10
  %5460 = icmp eq i8 %5457, %5459
  br i1 %5460, label %5437, label %5461

5461:
  tail call void @report(i64 noundef 65)
  store i1 true, ptr @bad, align 4
  br label %5463

5462:
  br label %5463

5463:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5464

5464:
  %5465 = phi i64 [ 0, %5463 ], [ %5485, %5464 ]
  %5466 = trunc i64 %5465 to i8
  %5467 = mul i8 %5466, 7
  %5468 = or disjoint i8 %5467, 3
  %5469 = getelementptr inbounds nuw i8, ptr @src, i64 %5465
  store i8 %5468, ptr %5469, align 4, !tbaa !10
  %5470 = or disjoint i64 %5465, 1
  %5471 = trunc i64 %5470 to i8
  %5472 = mul i8 %5471, 7
  %5473 = add i8 %5472, 3
  %5474 = getelementptr inbounds nuw i8, ptr @src, i64 %5470
  store i8 %5473, ptr %5474, align 1, !tbaa !10
  %5475 = or disjoint i64 %5465, 2
  %5476 = trunc i64 %5475 to i8
  %5477 = mul i8 %5476, 7
  %5478 = add i8 %5477, 3
  %5479 = getelementptr inbounds nuw i8, ptr @src, i64 %5475
  store i8 %5478, ptr %5479, align 2, !tbaa !10
  %5480 = or disjoint i64 %5465, 3
  %5481 = trunc i64 %5480 to i8
  %5482 = mul i8 %5481, 7
  %5483 = add i8 %5482, 3
  %5484 = getelementptr inbounds nuw i8, ptr @src, i64 %5480
  store i8 %5483, ptr %5484, align 1, !tbaa !10
  %5485 = add nuw nsw i64 %5465, 4
  %5486 = icmp eq i64 %5485, 1200
  br i1 %5486, label %5487, label %5464, !llvm.loop !11

5487:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv65(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 65)
  br label %5505

5488:
  %5489 = add nuw nsw i64 %5506, 1
  %5490 = getelementptr inbounds nuw i8, ptr @dst, i64 %5489
  %5491 = load i8, ptr %5490, align 1, !tbaa !10
  %5492 = getelementptr inbounds nuw i8, ptr @ref, i64 %5489
  %5493 = load i8, ptr %5492, align 1, !tbaa !10
  %5494 = icmp eq i8 %5491, %5493
  br i1 %5494, label %5495, label %5512

5495:
  %5496 = add nuw nsw i64 %5506, 2
  %5497 = getelementptr inbounds nuw i8, ptr @dst, i64 %5496
  %5498 = load i8, ptr %5497, align 1, !tbaa !10
  %5499 = getelementptr inbounds nuw i8, ptr @ref, i64 %5496
  %5500 = load i8, ptr %5499, align 1, !tbaa !10
  %5501 = icmp eq i8 %5498, %5500
  br i1 %5501, label %5502, label %5512

5502:
  %5503 = add nuw nsw i64 %5506, 3
  %5504 = icmp eq i64 %5503, 1200
  br i1 %5504, label %5513, label %5505, !llvm.loop !144

5505:
  %5506 = phi i64 [ 0, %5487 ], [ %5503, %5502 ]
  %5507 = getelementptr inbounds nuw i8, ptr @dst, i64 %5506
  %5508 = load i8, ptr %5507, align 1, !tbaa !10
  %5509 = getelementptr inbounds nuw i8, ptr @ref, i64 %5506
  %5510 = load i8, ptr %5509, align 1, !tbaa !10
  %5511 = icmp eq i8 %5508, %5510
  br i1 %5511, label %5488, label %5512

5512:
  tail call void @report(i64 noundef 65)
  store i1 true, ptr @bad, align 4
  br label %5514

5513:
  br label %5514

5514:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5515

5515:
  %5516 = phi i64 [ 0, %5514 ], [ %5536, %5515 ]
  %5517 = trunc i64 %5516 to i8
  %5518 = mul i8 %5517, 7
  %5519 = or disjoint i8 %5518, 3
  %5520 = getelementptr inbounds nuw i8, ptr @src, i64 %5516
  store i8 %5519, ptr %5520, align 4, !tbaa !10
  %5521 = or disjoint i64 %5516, 1
  %5522 = trunc i64 %5521 to i8
  %5523 = mul i8 %5522, 7
  %5524 = add i8 %5523, 3
  %5525 = getelementptr inbounds nuw i8, ptr @src, i64 %5521
  store i8 %5524, ptr %5525, align 1, !tbaa !10
  %5526 = or disjoint i64 %5516, 2
  %5527 = trunc i64 %5526 to i8
  %5528 = mul i8 %5527, 7
  %5529 = add i8 %5528, 3
  %5530 = getelementptr inbounds nuw i8, ptr @src, i64 %5526
  store i8 %5529, ptr %5530, align 2, !tbaa !10
  %5531 = or disjoint i64 %5516, 3
  %5532 = trunc i64 %5531 to i8
  %5533 = mul i8 %5532, 7
  %5534 = add i8 %5533, 3
  %5535 = getelementptr inbounds nuw i8, ptr @src, i64 %5531
  store i8 %5534, ptr %5535, align 1, !tbaa !10
  %5536 = add nuw nsw i64 %5516, 4
  %5537 = icmp eq i64 %5536, 1200
  br i1 %5537, label %5538, label %5515, !llvm.loop !11

5538:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp100()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 100)
  br label %5556

5539:
  %5540 = add nuw nsw i64 %5557, 1
  %5541 = getelementptr inbounds nuw i8, ptr @dst, i64 %5540
  %5542 = load i8, ptr %5541, align 1, !tbaa !10
  %5543 = getelementptr inbounds nuw i8, ptr @ref, i64 %5540
  %5544 = load i8, ptr %5543, align 1, !tbaa !10
  %5545 = icmp eq i8 %5542, %5544
  br i1 %5545, label %5546, label %5563

5546:
  %5547 = add nuw nsw i64 %5557, 2
  %5548 = getelementptr inbounds nuw i8, ptr @dst, i64 %5547
  %5549 = load i8, ptr %5548, align 1, !tbaa !10
  %5550 = getelementptr inbounds nuw i8, ptr @ref, i64 %5547
  %5551 = load i8, ptr %5550, align 1, !tbaa !10
  %5552 = icmp eq i8 %5549, %5551
  br i1 %5552, label %5553, label %5563

5553:
  %5554 = add nuw nsw i64 %5557, 3
  %5555 = icmp eq i64 %5554, 1200
  br i1 %5555, label %5564, label %5556, !llvm.loop !144

5556:
  %5557 = phi i64 [ 0, %5538 ], [ %5554, %5553 ]
  %5558 = getelementptr inbounds nuw i8, ptr @dst, i64 %5557
  %5559 = load i8, ptr %5558, align 1, !tbaa !10
  %5560 = getelementptr inbounds nuw i8, ptr @ref, i64 %5557
  %5561 = load i8, ptr %5560, align 1, !tbaa !10
  %5562 = icmp eq i8 %5559, %5561
  br i1 %5562, label %5539, label %5563

5563:
  tail call void @report(i64 noundef 100)
  store i1 true, ptr @bad, align 4
  br label %5565

5564:
  br label %5565

5565:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5566

5566:
  %5567 = phi i64 [ 0, %5565 ], [ %5587, %5566 ]
  %5568 = trunc i64 %5567 to i8
  %5569 = mul i8 %5568, 7
  %5570 = or disjoint i8 %5569, 3
  %5571 = getelementptr inbounds nuw i8, ptr @src, i64 %5567
  store i8 %5570, ptr %5571, align 4, !tbaa !10
  %5572 = or disjoint i64 %5567, 1
  %5573 = trunc i64 %5572 to i8
  %5574 = mul i8 %5573, 7
  %5575 = add i8 %5574, 3
  %5576 = getelementptr inbounds nuw i8, ptr @src, i64 %5572
  store i8 %5575, ptr %5576, align 1, !tbaa !10
  %5577 = or disjoint i64 %5567, 2
  %5578 = trunc i64 %5577 to i8
  %5579 = mul i8 %5578, 7
  %5580 = add i8 %5579, 3
  %5581 = getelementptr inbounds nuw i8, ptr @src, i64 %5577
  store i8 %5580, ptr %5581, align 2, !tbaa !10
  %5582 = or disjoint i64 %5567, 3
  %5583 = trunc i64 %5582 to i8
  %5584 = mul i8 %5583, 7
  %5585 = add i8 %5584, 3
  %5586 = getelementptr inbounds nuw i8, ptr @src, i64 %5582
  store i8 %5585, ptr %5586, align 1, !tbaa !10
  %5587 = add nuw nsw i64 %5567, 4
  %5588 = icmp eq i64 %5587, 1200
  br i1 %5588, label %5589, label %5566, !llvm.loop !11

5589:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr100()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 100)
  br label %5607

5590:
  %5591 = add nuw nsw i64 %5608, 1
  %5592 = getelementptr inbounds nuw i8, ptr @dst, i64 %5591
  %5593 = load i8, ptr %5592, align 1, !tbaa !10
  %5594 = getelementptr inbounds nuw i8, ptr @ref, i64 %5591
  %5595 = load i8, ptr %5594, align 1, !tbaa !10
  %5596 = icmp eq i8 %5593, %5595
  br i1 %5596, label %5597, label %5614

5597:
  %5598 = add nuw nsw i64 %5608, 2
  %5599 = getelementptr inbounds nuw i8, ptr @dst, i64 %5598
  %5600 = load i8, ptr %5599, align 1, !tbaa !10
  %5601 = getelementptr inbounds nuw i8, ptr @ref, i64 %5598
  %5602 = load i8, ptr %5601, align 1, !tbaa !10
  %5603 = icmp eq i8 %5600, %5602
  br i1 %5603, label %5604, label %5614

5604:
  %5605 = add nuw nsw i64 %5608, 3
  %5606 = icmp eq i64 %5605, 1200
  br i1 %5606, label %5615, label %5607, !llvm.loop !144

5607:
  %5608 = phi i64 [ 0, %5589 ], [ %5605, %5604 ]
  %5609 = getelementptr inbounds nuw i8, ptr @dst, i64 %5608
  %5610 = load i8, ptr %5609, align 1, !tbaa !10
  %5611 = getelementptr inbounds nuw i8, ptr @ref, i64 %5608
  %5612 = load i8, ptr %5611, align 1, !tbaa !10
  %5613 = icmp eq i8 %5610, %5612
  br i1 %5613, label %5590, label %5614

5614:
  tail call void @report(i64 noundef 100)
  store i1 true, ptr @bad, align 4
  br label %5616

5615:
  br label %5616

5616:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5617

5617:
  %5618 = phi i64 [ 0, %5616 ], [ %5638, %5617 ]
  %5619 = trunc i64 %5618 to i8
  %5620 = mul i8 %5619, 7
  %5621 = or disjoint i8 %5620, 3
  %5622 = getelementptr inbounds nuw i8, ptr @src, i64 %5618
  store i8 %5621, ptr %5622, align 4, !tbaa !10
  %5623 = or disjoint i64 %5618, 1
  %5624 = trunc i64 %5623 to i8
  %5625 = mul i8 %5624, 7
  %5626 = add i8 %5625, 3
  %5627 = getelementptr inbounds nuw i8, ptr @src, i64 %5623
  store i8 %5626, ptr %5627, align 1, !tbaa !10
  %5628 = or disjoint i64 %5618, 2
  %5629 = trunc i64 %5628 to i8
  %5630 = mul i8 %5629, 7
  %5631 = add i8 %5630, 3
  %5632 = getelementptr inbounds nuw i8, ptr @src, i64 %5628
  store i8 %5631, ptr %5632, align 2, !tbaa !10
  %5633 = or disjoint i64 %5618, 3
  %5634 = trunc i64 %5633 to i8
  %5635 = mul i8 %5634, 7
  %5636 = add i8 %5635, 3
  %5637 = getelementptr inbounds nuw i8, ptr @src, i64 %5633
  store i8 %5636, ptr %5637, align 1, !tbaa !10
  %5638 = add nuw nsw i64 %5618, 4
  %5639 = icmp eq i64 %5638, 1200
  br i1 %5639, label %5640, label %5617, !llvm.loop !11

5640:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7100()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 100)
  br label %5658

5641:
  %5642 = add nuw nsw i64 %5659, 1
  %5643 = getelementptr inbounds nuw i8, ptr @dst, i64 %5642
  %5644 = load i8, ptr %5643, align 1, !tbaa !10
  %5645 = getelementptr inbounds nuw i8, ptr @ref, i64 %5642
  %5646 = load i8, ptr %5645, align 1, !tbaa !10
  %5647 = icmp eq i8 %5644, %5646
  br i1 %5647, label %5648, label %5665

5648:
  %5649 = add nuw nsw i64 %5659, 2
  %5650 = getelementptr inbounds nuw i8, ptr @dst, i64 %5649
  %5651 = load i8, ptr %5650, align 1, !tbaa !10
  %5652 = getelementptr inbounds nuw i8, ptr @ref, i64 %5649
  %5653 = load i8, ptr %5652, align 1, !tbaa !10
  %5654 = icmp eq i8 %5651, %5653
  br i1 %5654, label %5655, label %5665

5655:
  %5656 = add nuw nsw i64 %5659, 3
  %5657 = icmp eq i64 %5656, 1200
  br i1 %5657, label %5666, label %5658, !llvm.loop !144

5658:
  %5659 = phi i64 [ 0, %5640 ], [ %5656, %5655 ]
  %5660 = getelementptr inbounds nuw i8, ptr @dst, i64 %5659
  %5661 = load i8, ptr %5660, align 1, !tbaa !10
  %5662 = getelementptr inbounds nuw i8, ptr @ref, i64 %5659
  %5663 = load i8, ptr %5662, align 1, !tbaa !10
  %5664 = icmp eq i8 %5661, %5663
  br i1 %5664, label %5641, label %5665

5665:
  tail call void @report(i64 noundef 100)
  store i1 true, ptr @bad, align 4
  br label %5667

5666:
  br label %5667

5667:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5668

5668:
  %5669 = phi i64 [ 0, %5667 ], [ %5689, %5668 ]
  %5670 = trunc i64 %5669 to i8
  %5671 = mul i8 %5670, 7
  %5672 = or disjoint i8 %5671, 3
  %5673 = getelementptr inbounds nuw i8, ptr @src, i64 %5669
  store i8 %5672, ptr %5673, align 4, !tbaa !10
  %5674 = or disjoint i64 %5669, 1
  %5675 = trunc i64 %5674 to i8
  %5676 = mul i8 %5675, 7
  %5677 = add i8 %5676, 3
  %5678 = getelementptr inbounds nuw i8, ptr @src, i64 %5674
  store i8 %5677, ptr %5678, align 1, !tbaa !10
  %5679 = or disjoint i64 %5669, 2
  %5680 = trunc i64 %5679 to i8
  %5681 = mul i8 %5680, 7
  %5682 = add i8 %5681, 3
  %5683 = getelementptr inbounds nuw i8, ptr @src, i64 %5679
  store i8 %5682, ptr %5683, align 2, !tbaa !10
  %5684 = or disjoint i64 %5669, 3
  %5685 = trunc i64 %5684 to i8
  %5686 = mul i8 %5685, 7
  %5687 = add i8 %5686, 3
  %5688 = getelementptr inbounds nuw i8, ptr @src, i64 %5684
  store i8 %5687, ptr %5688, align 1, !tbaa !10
  %5689 = add nuw nsw i64 %5669, 4
  %5690 = icmp eq i64 %5689, 1200
  br i1 %5690, label %5691, label %5668, !llvm.loop !11

5691:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr100()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 100)
  br label %5709

5692:
  %5693 = add nuw nsw i64 %5710, 1
  %5694 = getelementptr inbounds nuw i8, ptr @dst, i64 %5693
  %5695 = load i8, ptr %5694, align 1, !tbaa !10
  %5696 = getelementptr inbounds nuw i8, ptr @ref, i64 %5693
  %5697 = load i8, ptr %5696, align 1, !tbaa !10
  %5698 = icmp eq i8 %5695, %5697
  br i1 %5698, label %5699, label %5716

5699:
  %5700 = add nuw nsw i64 %5710, 2
  %5701 = getelementptr inbounds nuw i8, ptr @dst, i64 %5700
  %5702 = load i8, ptr %5701, align 1, !tbaa !10
  %5703 = getelementptr inbounds nuw i8, ptr @ref, i64 %5700
  %5704 = load i8, ptr %5703, align 1, !tbaa !10
  %5705 = icmp eq i8 %5702, %5704
  br i1 %5705, label %5706, label %5716

5706:
  %5707 = add nuw nsw i64 %5710, 3
  %5708 = icmp eq i64 %5707, 1200
  br i1 %5708, label %5717, label %5709, !llvm.loop !144

5709:
  %5710 = phi i64 [ 0, %5691 ], [ %5707, %5706 ]
  %5711 = getelementptr inbounds nuw i8, ptr @dst, i64 %5710
  %5712 = load i8, ptr %5711, align 1, !tbaa !10
  %5713 = getelementptr inbounds nuw i8, ptr @ref, i64 %5710
  %5714 = load i8, ptr %5713, align 1, !tbaa !10
  %5715 = icmp eq i8 %5712, %5714
  br i1 %5715, label %5692, label %5716

5716:
  tail call void @report(i64 noundef 100)
  store i1 true, ptr @bad, align 4
  br label %5718

5717:
  br label %5718

5718:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5719

5719:
  %5720 = phi i64 [ 0, %5718 ], [ %5740, %5719 ]
  %5721 = trunc i64 %5720 to i8
  %5722 = mul i8 %5721, 7
  %5723 = or disjoint i8 %5722, 3
  %5724 = getelementptr inbounds nuw i8, ptr @src, i64 %5720
  store i8 %5723, ptr %5724, align 4, !tbaa !10
  %5725 = or disjoint i64 %5720, 1
  %5726 = trunc i64 %5725 to i8
  %5727 = mul i8 %5726, 7
  %5728 = add i8 %5727, 3
  %5729 = getelementptr inbounds nuw i8, ptr @src, i64 %5725
  store i8 %5728, ptr %5729, align 1, !tbaa !10
  %5730 = or disjoint i64 %5720, 2
  %5731 = trunc i64 %5730 to i8
  %5732 = mul i8 %5731, 7
  %5733 = add i8 %5732, 3
  %5734 = getelementptr inbounds nuw i8, ptr @src, i64 %5730
  store i8 %5733, ptr %5734, align 2, !tbaa !10
  %5735 = or disjoint i64 %5720, 3
  %5736 = trunc i64 %5735 to i8
  %5737 = mul i8 %5736, 7
  %5738 = add i8 %5737, 3
  %5739 = getelementptr inbounds nuw i8, ptr @src, i64 %5735
  store i8 %5738, ptr %5739, align 1, !tbaa !10
  %5740 = add nuw nsw i64 %5720, 4
  %5741 = icmp eq i64 %5740, 1200
  br i1 %5741, label %5742, label %5719, !llvm.loop !11

5742:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv100(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 100)
  br label %5760

5743:
  %5744 = add nuw nsw i64 %5761, 1
  %5745 = getelementptr inbounds nuw i8, ptr @dst, i64 %5744
  %5746 = load i8, ptr %5745, align 1, !tbaa !10
  %5747 = getelementptr inbounds nuw i8, ptr @ref, i64 %5744
  %5748 = load i8, ptr %5747, align 1, !tbaa !10
  %5749 = icmp eq i8 %5746, %5748
  br i1 %5749, label %5750, label %5767

5750:
  %5751 = add nuw nsw i64 %5761, 2
  %5752 = getelementptr inbounds nuw i8, ptr @dst, i64 %5751
  %5753 = load i8, ptr %5752, align 1, !tbaa !10
  %5754 = getelementptr inbounds nuw i8, ptr @ref, i64 %5751
  %5755 = load i8, ptr %5754, align 1, !tbaa !10
  %5756 = icmp eq i8 %5753, %5755
  br i1 %5756, label %5757, label %5767

5757:
  %5758 = add nuw nsw i64 %5761, 3
  %5759 = icmp eq i64 %5758, 1200
  br i1 %5759, label %5768, label %5760, !llvm.loop !144

5760:
  %5761 = phi i64 [ 0, %5742 ], [ %5758, %5757 ]
  %5762 = getelementptr inbounds nuw i8, ptr @dst, i64 %5761
  %5763 = load i8, ptr %5762, align 1, !tbaa !10
  %5764 = getelementptr inbounds nuw i8, ptr @ref, i64 %5761
  %5765 = load i8, ptr %5764, align 1, !tbaa !10
  %5766 = icmp eq i8 %5763, %5765
  br i1 %5766, label %5743, label %5767

5767:
  tail call void @report(i64 noundef 100)
  store i1 true, ptr @bad, align 4
  br label %5769

5768:
  br label %5769

5769:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5770

5770:
  %5771 = phi i64 [ 0, %5769 ], [ %5791, %5770 ]
  %5772 = trunc i64 %5771 to i8
  %5773 = mul i8 %5772, 7
  %5774 = or disjoint i8 %5773, 3
  %5775 = getelementptr inbounds nuw i8, ptr @src, i64 %5771
  store i8 %5774, ptr %5775, align 4, !tbaa !10
  %5776 = or disjoint i64 %5771, 1
  %5777 = trunc i64 %5776 to i8
  %5778 = mul i8 %5777, 7
  %5779 = add i8 %5778, 3
  %5780 = getelementptr inbounds nuw i8, ptr @src, i64 %5776
  store i8 %5779, ptr %5780, align 1, !tbaa !10
  %5781 = or disjoint i64 %5771, 2
  %5782 = trunc i64 %5781 to i8
  %5783 = mul i8 %5782, 7
  %5784 = add i8 %5783, 3
  %5785 = getelementptr inbounds nuw i8, ptr @src, i64 %5781
  store i8 %5784, ptr %5785, align 2, !tbaa !10
  %5786 = or disjoint i64 %5771, 3
  %5787 = trunc i64 %5786 to i8
  %5788 = mul i8 %5787, 7
  %5789 = add i8 %5788, 3
  %5790 = getelementptr inbounds nuw i8, ptr @src, i64 %5786
  store i8 %5789, ptr %5790, align 1, !tbaa !10
  %5791 = add nuw nsw i64 %5771, 4
  %5792 = icmp eq i64 %5791, 1200
  br i1 %5792, label %5793, label %5770, !llvm.loop !11

5793:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv100(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 100)
  br label %5811

5794:
  %5795 = add nuw nsw i64 %5812, 1
  %5796 = getelementptr inbounds nuw i8, ptr @dst, i64 %5795
  %5797 = load i8, ptr %5796, align 1, !tbaa !10
  %5798 = getelementptr inbounds nuw i8, ptr @ref, i64 %5795
  %5799 = load i8, ptr %5798, align 1, !tbaa !10
  %5800 = icmp eq i8 %5797, %5799
  br i1 %5800, label %5801, label %5818

5801:
  %5802 = add nuw nsw i64 %5812, 2
  %5803 = getelementptr inbounds nuw i8, ptr @dst, i64 %5802
  %5804 = load i8, ptr %5803, align 1, !tbaa !10
  %5805 = getelementptr inbounds nuw i8, ptr @ref, i64 %5802
  %5806 = load i8, ptr %5805, align 1, !tbaa !10
  %5807 = icmp eq i8 %5804, %5806
  br i1 %5807, label %5808, label %5818

5808:
  %5809 = add nuw nsw i64 %5812, 3
  %5810 = icmp eq i64 %5809, 1200
  br i1 %5810, label %5819, label %5811, !llvm.loop !144

5811:
  %5812 = phi i64 [ 0, %5793 ], [ %5809, %5808 ]
  %5813 = getelementptr inbounds nuw i8, ptr @dst, i64 %5812
  %5814 = load i8, ptr %5813, align 1, !tbaa !10
  %5815 = getelementptr inbounds nuw i8, ptr @ref, i64 %5812
  %5816 = load i8, ptr %5815, align 1, !tbaa !10
  %5817 = icmp eq i8 %5814, %5816
  br i1 %5817, label %5794, label %5818

5818:
  tail call void @report(i64 noundef 100)
  store i1 true, ptr @bad, align 4
  br label %5820

5819:
  br label %5820

5820:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5821

5821:
  %5822 = phi i64 [ 0, %5820 ], [ %5842, %5821 ]
  %5823 = trunc i64 %5822 to i8
  %5824 = mul i8 %5823, 7
  %5825 = or disjoint i8 %5824, 3
  %5826 = getelementptr inbounds nuw i8, ptr @src, i64 %5822
  store i8 %5825, ptr %5826, align 4, !tbaa !10
  %5827 = or disjoint i64 %5822, 1
  %5828 = trunc i64 %5827 to i8
  %5829 = mul i8 %5828, 7
  %5830 = add i8 %5829, 3
  %5831 = getelementptr inbounds nuw i8, ptr @src, i64 %5827
  store i8 %5830, ptr %5831, align 1, !tbaa !10
  %5832 = or disjoint i64 %5822, 2
  %5833 = trunc i64 %5832 to i8
  %5834 = mul i8 %5833, 7
  %5835 = add i8 %5834, 3
  %5836 = getelementptr inbounds nuw i8, ptr @src, i64 %5832
  store i8 %5835, ptr %5836, align 2, !tbaa !10
  %5837 = or disjoint i64 %5822, 3
  %5838 = trunc i64 %5837 to i8
  %5839 = mul i8 %5838, 7
  %5840 = add i8 %5839, 3
  %5841 = getelementptr inbounds nuw i8, ptr @src, i64 %5837
  store i8 %5840, ptr %5841, align 1, !tbaa !10
  %5842 = add nuw nsw i64 %5822, 4
  %5843 = icmp eq i64 %5842, 1200
  br i1 %5843, label %5844, label %5821, !llvm.loop !11

5844:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp127()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 127)
  br label %5862

5845:
  %5846 = add nuw nsw i64 %5863, 1
  %5847 = getelementptr inbounds nuw i8, ptr @dst, i64 %5846
  %5848 = load i8, ptr %5847, align 1, !tbaa !10
  %5849 = getelementptr inbounds nuw i8, ptr @ref, i64 %5846
  %5850 = load i8, ptr %5849, align 1, !tbaa !10
  %5851 = icmp eq i8 %5848, %5850
  br i1 %5851, label %5852, label %5869

5852:
  %5853 = add nuw nsw i64 %5863, 2
  %5854 = getelementptr inbounds nuw i8, ptr @dst, i64 %5853
  %5855 = load i8, ptr %5854, align 1, !tbaa !10
  %5856 = getelementptr inbounds nuw i8, ptr @ref, i64 %5853
  %5857 = load i8, ptr %5856, align 1, !tbaa !10
  %5858 = icmp eq i8 %5855, %5857
  br i1 %5858, label %5859, label %5869

5859:
  %5860 = add nuw nsw i64 %5863, 3
  %5861 = icmp eq i64 %5860, 1200
  br i1 %5861, label %5870, label %5862, !llvm.loop !144

5862:
  %5863 = phi i64 [ 0, %5844 ], [ %5860, %5859 ]
  %5864 = getelementptr inbounds nuw i8, ptr @dst, i64 %5863
  %5865 = load i8, ptr %5864, align 1, !tbaa !10
  %5866 = getelementptr inbounds nuw i8, ptr @ref, i64 %5863
  %5867 = load i8, ptr %5866, align 1, !tbaa !10
  %5868 = icmp eq i8 %5865, %5867
  br i1 %5868, label %5845, label %5869

5869:
  tail call void @report(i64 noundef 127)
  store i1 true, ptr @bad, align 4
  br label %5871

5870:
  br label %5871

5871:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5872

5872:
  %5873 = phi i64 [ 0, %5871 ], [ %5893, %5872 ]
  %5874 = trunc i64 %5873 to i8
  %5875 = mul i8 %5874, 7
  %5876 = or disjoint i8 %5875, 3
  %5877 = getelementptr inbounds nuw i8, ptr @src, i64 %5873
  store i8 %5876, ptr %5877, align 4, !tbaa !10
  %5878 = or disjoint i64 %5873, 1
  %5879 = trunc i64 %5878 to i8
  %5880 = mul i8 %5879, 7
  %5881 = add i8 %5880, 3
  %5882 = getelementptr inbounds nuw i8, ptr @src, i64 %5878
  store i8 %5881, ptr %5882, align 1, !tbaa !10
  %5883 = or disjoint i64 %5873, 2
  %5884 = trunc i64 %5883 to i8
  %5885 = mul i8 %5884, 7
  %5886 = add i8 %5885, 3
  %5887 = getelementptr inbounds nuw i8, ptr @src, i64 %5883
  store i8 %5886, ptr %5887, align 2, !tbaa !10
  %5888 = or disjoint i64 %5873, 3
  %5889 = trunc i64 %5888 to i8
  %5890 = mul i8 %5889, 7
  %5891 = add i8 %5890, 3
  %5892 = getelementptr inbounds nuw i8, ptr @src, i64 %5888
  store i8 %5891, ptr %5892, align 1, !tbaa !10
  %5893 = add nuw nsw i64 %5873, 4
  %5894 = icmp eq i64 %5893, 1200
  br i1 %5894, label %5895, label %5872, !llvm.loop !11

5895:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr127()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 127)
  br label %5913

5896:
  %5897 = add nuw nsw i64 %5914, 1
  %5898 = getelementptr inbounds nuw i8, ptr @dst, i64 %5897
  %5899 = load i8, ptr %5898, align 1, !tbaa !10
  %5900 = getelementptr inbounds nuw i8, ptr @ref, i64 %5897
  %5901 = load i8, ptr %5900, align 1, !tbaa !10
  %5902 = icmp eq i8 %5899, %5901
  br i1 %5902, label %5903, label %5920

5903:
  %5904 = add nuw nsw i64 %5914, 2
  %5905 = getelementptr inbounds nuw i8, ptr @dst, i64 %5904
  %5906 = load i8, ptr %5905, align 1, !tbaa !10
  %5907 = getelementptr inbounds nuw i8, ptr @ref, i64 %5904
  %5908 = load i8, ptr %5907, align 1, !tbaa !10
  %5909 = icmp eq i8 %5906, %5908
  br i1 %5909, label %5910, label %5920

5910:
  %5911 = add nuw nsw i64 %5914, 3
  %5912 = icmp eq i64 %5911, 1200
  br i1 %5912, label %5921, label %5913, !llvm.loop !144

5913:
  %5914 = phi i64 [ 0, %5895 ], [ %5911, %5910 ]
  %5915 = getelementptr inbounds nuw i8, ptr @dst, i64 %5914
  %5916 = load i8, ptr %5915, align 1, !tbaa !10
  %5917 = getelementptr inbounds nuw i8, ptr @ref, i64 %5914
  %5918 = load i8, ptr %5917, align 1, !tbaa !10
  %5919 = icmp eq i8 %5916, %5918
  br i1 %5919, label %5896, label %5920

5920:
  tail call void @report(i64 noundef 127)
  store i1 true, ptr @bad, align 4
  br label %5922

5921:
  br label %5922

5922:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5923

5923:
  %5924 = phi i64 [ 0, %5922 ], [ %5944, %5923 ]
  %5925 = trunc i64 %5924 to i8
  %5926 = mul i8 %5925, 7
  %5927 = or disjoint i8 %5926, 3
  %5928 = getelementptr inbounds nuw i8, ptr @src, i64 %5924
  store i8 %5927, ptr %5928, align 4, !tbaa !10
  %5929 = or disjoint i64 %5924, 1
  %5930 = trunc i64 %5929 to i8
  %5931 = mul i8 %5930, 7
  %5932 = add i8 %5931, 3
  %5933 = getelementptr inbounds nuw i8, ptr @src, i64 %5929
  store i8 %5932, ptr %5933, align 1, !tbaa !10
  %5934 = or disjoint i64 %5924, 2
  %5935 = trunc i64 %5934 to i8
  %5936 = mul i8 %5935, 7
  %5937 = add i8 %5936, 3
  %5938 = getelementptr inbounds nuw i8, ptr @src, i64 %5934
  store i8 %5937, ptr %5938, align 2, !tbaa !10
  %5939 = or disjoint i64 %5924, 3
  %5940 = trunc i64 %5939 to i8
  %5941 = mul i8 %5940, 7
  %5942 = add i8 %5941, 3
  %5943 = getelementptr inbounds nuw i8, ptr @src, i64 %5939
  store i8 %5942, ptr %5943, align 1, !tbaa !10
  %5944 = add nuw nsw i64 %5924, 4
  %5945 = icmp eq i64 %5944, 1200
  br i1 %5945, label %5946, label %5923, !llvm.loop !11

5946:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7127()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 127)
  br label %5964

5947:
  %5948 = add nuw nsw i64 %5965, 1
  %5949 = getelementptr inbounds nuw i8, ptr @dst, i64 %5948
  %5950 = load i8, ptr %5949, align 1, !tbaa !10
  %5951 = getelementptr inbounds nuw i8, ptr @ref, i64 %5948
  %5952 = load i8, ptr %5951, align 1, !tbaa !10
  %5953 = icmp eq i8 %5950, %5952
  br i1 %5953, label %5954, label %5971

5954:
  %5955 = add nuw nsw i64 %5965, 2
  %5956 = getelementptr inbounds nuw i8, ptr @dst, i64 %5955
  %5957 = load i8, ptr %5956, align 1, !tbaa !10
  %5958 = getelementptr inbounds nuw i8, ptr @ref, i64 %5955
  %5959 = load i8, ptr %5958, align 1, !tbaa !10
  %5960 = icmp eq i8 %5957, %5959
  br i1 %5960, label %5961, label %5971

5961:
  %5962 = add nuw nsw i64 %5965, 3
  %5963 = icmp eq i64 %5962, 1200
  br i1 %5963, label %5972, label %5964, !llvm.loop !144

5964:
  %5965 = phi i64 [ 0, %5946 ], [ %5962, %5961 ]
  %5966 = getelementptr inbounds nuw i8, ptr @dst, i64 %5965
  %5967 = load i8, ptr %5966, align 1, !tbaa !10
  %5968 = getelementptr inbounds nuw i8, ptr @ref, i64 %5965
  %5969 = load i8, ptr %5968, align 1, !tbaa !10
  %5970 = icmp eq i8 %5967, %5969
  br i1 %5970, label %5947, label %5971

5971:
  tail call void @report(i64 noundef 127)
  store i1 true, ptr @bad, align 4
  br label %5973

5972:
  br label %5973

5973:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %5974

5974:
  %5975 = phi i64 [ 0, %5973 ], [ %5995, %5974 ]
  %5976 = trunc i64 %5975 to i8
  %5977 = mul i8 %5976, 7
  %5978 = or disjoint i8 %5977, 3
  %5979 = getelementptr inbounds nuw i8, ptr @src, i64 %5975
  store i8 %5978, ptr %5979, align 4, !tbaa !10
  %5980 = or disjoint i64 %5975, 1
  %5981 = trunc i64 %5980 to i8
  %5982 = mul i8 %5981, 7
  %5983 = add i8 %5982, 3
  %5984 = getelementptr inbounds nuw i8, ptr @src, i64 %5980
  store i8 %5983, ptr %5984, align 1, !tbaa !10
  %5985 = or disjoint i64 %5975, 2
  %5986 = trunc i64 %5985 to i8
  %5987 = mul i8 %5986, 7
  %5988 = add i8 %5987, 3
  %5989 = getelementptr inbounds nuw i8, ptr @src, i64 %5985
  store i8 %5988, ptr %5989, align 2, !tbaa !10
  %5990 = or disjoint i64 %5975, 3
  %5991 = trunc i64 %5990 to i8
  %5992 = mul i8 %5991, 7
  %5993 = add i8 %5992, 3
  %5994 = getelementptr inbounds nuw i8, ptr @src, i64 %5990
  store i8 %5993, ptr %5994, align 1, !tbaa !10
  %5995 = add nuw nsw i64 %5975, 4
  %5996 = icmp eq i64 %5995, 1200
  br i1 %5996, label %5997, label %5974, !llvm.loop !11

5997:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr127()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 127)
  br label %6015

5998:
  %5999 = add nuw nsw i64 %6016, 1
  %6000 = getelementptr inbounds nuw i8, ptr @dst, i64 %5999
  %6001 = load i8, ptr %6000, align 1, !tbaa !10
  %6002 = getelementptr inbounds nuw i8, ptr @ref, i64 %5999
  %6003 = load i8, ptr %6002, align 1, !tbaa !10
  %6004 = icmp eq i8 %6001, %6003
  br i1 %6004, label %6005, label %6022

6005:
  %6006 = add nuw nsw i64 %6016, 2
  %6007 = getelementptr inbounds nuw i8, ptr @dst, i64 %6006
  %6008 = load i8, ptr %6007, align 1, !tbaa !10
  %6009 = getelementptr inbounds nuw i8, ptr @ref, i64 %6006
  %6010 = load i8, ptr %6009, align 1, !tbaa !10
  %6011 = icmp eq i8 %6008, %6010
  br i1 %6011, label %6012, label %6022

6012:
  %6013 = add nuw nsw i64 %6016, 3
  %6014 = icmp eq i64 %6013, 1200
  br i1 %6014, label %6023, label %6015, !llvm.loop !144

6015:
  %6016 = phi i64 [ 0, %5997 ], [ %6013, %6012 ]
  %6017 = getelementptr inbounds nuw i8, ptr @dst, i64 %6016
  %6018 = load i8, ptr %6017, align 1, !tbaa !10
  %6019 = getelementptr inbounds nuw i8, ptr @ref, i64 %6016
  %6020 = load i8, ptr %6019, align 1, !tbaa !10
  %6021 = icmp eq i8 %6018, %6020
  br i1 %6021, label %5998, label %6022

6022:
  tail call void @report(i64 noundef 127)
  store i1 true, ptr @bad, align 4
  br label %6024

6023:
  br label %6024

6024:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6025

6025:
  %6026 = phi i64 [ 0, %6024 ], [ %6046, %6025 ]
  %6027 = trunc i64 %6026 to i8
  %6028 = mul i8 %6027, 7
  %6029 = or disjoint i8 %6028, 3
  %6030 = getelementptr inbounds nuw i8, ptr @src, i64 %6026
  store i8 %6029, ptr %6030, align 4, !tbaa !10
  %6031 = or disjoint i64 %6026, 1
  %6032 = trunc i64 %6031 to i8
  %6033 = mul i8 %6032, 7
  %6034 = add i8 %6033, 3
  %6035 = getelementptr inbounds nuw i8, ptr @src, i64 %6031
  store i8 %6034, ptr %6035, align 1, !tbaa !10
  %6036 = or disjoint i64 %6026, 2
  %6037 = trunc i64 %6036 to i8
  %6038 = mul i8 %6037, 7
  %6039 = add i8 %6038, 3
  %6040 = getelementptr inbounds nuw i8, ptr @src, i64 %6036
  store i8 %6039, ptr %6040, align 2, !tbaa !10
  %6041 = or disjoint i64 %6026, 3
  %6042 = trunc i64 %6041 to i8
  %6043 = mul i8 %6042, 7
  %6044 = add i8 %6043, 3
  %6045 = getelementptr inbounds nuw i8, ptr @src, i64 %6041
  store i8 %6044, ptr %6045, align 1, !tbaa !10
  %6046 = add nuw nsw i64 %6026, 4
  %6047 = icmp eq i64 %6046, 1200
  br i1 %6047, label %6048, label %6025, !llvm.loop !11

6048:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv127(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 127)
  br label %6066

6049:
  %6050 = add nuw nsw i64 %6067, 1
  %6051 = getelementptr inbounds nuw i8, ptr @dst, i64 %6050
  %6052 = load i8, ptr %6051, align 1, !tbaa !10
  %6053 = getelementptr inbounds nuw i8, ptr @ref, i64 %6050
  %6054 = load i8, ptr %6053, align 1, !tbaa !10
  %6055 = icmp eq i8 %6052, %6054
  br i1 %6055, label %6056, label %6073

6056:
  %6057 = add nuw nsw i64 %6067, 2
  %6058 = getelementptr inbounds nuw i8, ptr @dst, i64 %6057
  %6059 = load i8, ptr %6058, align 1, !tbaa !10
  %6060 = getelementptr inbounds nuw i8, ptr @ref, i64 %6057
  %6061 = load i8, ptr %6060, align 1, !tbaa !10
  %6062 = icmp eq i8 %6059, %6061
  br i1 %6062, label %6063, label %6073

6063:
  %6064 = add nuw nsw i64 %6067, 3
  %6065 = icmp eq i64 %6064, 1200
  br i1 %6065, label %6074, label %6066, !llvm.loop !144

6066:
  %6067 = phi i64 [ 0, %6048 ], [ %6064, %6063 ]
  %6068 = getelementptr inbounds nuw i8, ptr @dst, i64 %6067
  %6069 = load i8, ptr %6068, align 1, !tbaa !10
  %6070 = getelementptr inbounds nuw i8, ptr @ref, i64 %6067
  %6071 = load i8, ptr %6070, align 1, !tbaa !10
  %6072 = icmp eq i8 %6069, %6071
  br i1 %6072, label %6049, label %6073

6073:
  tail call void @report(i64 noundef 127)
  store i1 true, ptr @bad, align 4
  br label %6075

6074:
  br label %6075

6075:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6076

6076:
  %6077 = phi i64 [ 0, %6075 ], [ %6097, %6076 ]
  %6078 = trunc i64 %6077 to i8
  %6079 = mul i8 %6078, 7
  %6080 = or disjoint i8 %6079, 3
  %6081 = getelementptr inbounds nuw i8, ptr @src, i64 %6077
  store i8 %6080, ptr %6081, align 4, !tbaa !10
  %6082 = or disjoint i64 %6077, 1
  %6083 = trunc i64 %6082 to i8
  %6084 = mul i8 %6083, 7
  %6085 = add i8 %6084, 3
  %6086 = getelementptr inbounds nuw i8, ptr @src, i64 %6082
  store i8 %6085, ptr %6086, align 1, !tbaa !10
  %6087 = or disjoint i64 %6077, 2
  %6088 = trunc i64 %6087 to i8
  %6089 = mul i8 %6088, 7
  %6090 = add i8 %6089, 3
  %6091 = getelementptr inbounds nuw i8, ptr @src, i64 %6087
  store i8 %6090, ptr %6091, align 2, !tbaa !10
  %6092 = or disjoint i64 %6077, 3
  %6093 = trunc i64 %6092 to i8
  %6094 = mul i8 %6093, 7
  %6095 = add i8 %6094, 3
  %6096 = getelementptr inbounds nuw i8, ptr @src, i64 %6092
  store i8 %6095, ptr %6096, align 1, !tbaa !10
  %6097 = add nuw nsw i64 %6077, 4
  %6098 = icmp eq i64 %6097, 1200
  br i1 %6098, label %6099, label %6076, !llvm.loop !11

6099:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv127(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 127)
  br label %6117

6100:
  %6101 = add nuw nsw i64 %6118, 1
  %6102 = getelementptr inbounds nuw i8, ptr @dst, i64 %6101
  %6103 = load i8, ptr %6102, align 1, !tbaa !10
  %6104 = getelementptr inbounds nuw i8, ptr @ref, i64 %6101
  %6105 = load i8, ptr %6104, align 1, !tbaa !10
  %6106 = icmp eq i8 %6103, %6105
  br i1 %6106, label %6107, label %6124

6107:
  %6108 = add nuw nsw i64 %6118, 2
  %6109 = getelementptr inbounds nuw i8, ptr @dst, i64 %6108
  %6110 = load i8, ptr %6109, align 1, !tbaa !10
  %6111 = getelementptr inbounds nuw i8, ptr @ref, i64 %6108
  %6112 = load i8, ptr %6111, align 1, !tbaa !10
  %6113 = icmp eq i8 %6110, %6112
  br i1 %6113, label %6114, label %6124

6114:
  %6115 = add nuw nsw i64 %6118, 3
  %6116 = icmp eq i64 %6115, 1200
  br i1 %6116, label %6125, label %6117, !llvm.loop !144

6117:
  %6118 = phi i64 [ 0, %6099 ], [ %6115, %6114 ]
  %6119 = getelementptr inbounds nuw i8, ptr @dst, i64 %6118
  %6120 = load i8, ptr %6119, align 1, !tbaa !10
  %6121 = getelementptr inbounds nuw i8, ptr @ref, i64 %6118
  %6122 = load i8, ptr %6121, align 1, !tbaa !10
  %6123 = icmp eq i8 %6120, %6122
  br i1 %6123, label %6100, label %6124

6124:
  tail call void @report(i64 noundef 127)
  store i1 true, ptr @bad, align 4
  br label %6126

6125:
  br label %6126

6126:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6127

6127:
  %6128 = phi i64 [ 0, %6126 ], [ %6148, %6127 ]
  %6129 = trunc i64 %6128 to i8
  %6130 = mul i8 %6129, 7
  %6131 = or disjoint i8 %6130, 3
  %6132 = getelementptr inbounds nuw i8, ptr @src, i64 %6128
  store i8 %6131, ptr %6132, align 4, !tbaa !10
  %6133 = or disjoint i64 %6128, 1
  %6134 = trunc i64 %6133 to i8
  %6135 = mul i8 %6134, 7
  %6136 = add i8 %6135, 3
  %6137 = getelementptr inbounds nuw i8, ptr @src, i64 %6133
  store i8 %6136, ptr %6137, align 1, !tbaa !10
  %6138 = or disjoint i64 %6128, 2
  %6139 = trunc i64 %6138 to i8
  %6140 = mul i8 %6139, 7
  %6141 = add i8 %6140, 3
  %6142 = getelementptr inbounds nuw i8, ptr @src, i64 %6138
  store i8 %6141, ptr %6142, align 2, !tbaa !10
  %6143 = or disjoint i64 %6128, 3
  %6144 = trunc i64 %6143 to i8
  %6145 = mul i8 %6144, 7
  %6146 = add i8 %6145, 3
  %6147 = getelementptr inbounds nuw i8, ptr @src, i64 %6143
  store i8 %6146, ptr %6147, align 1, !tbaa !10
  %6148 = add nuw nsw i64 %6128, 4
  %6149 = icmp eq i64 %6148, 1200
  br i1 %6149, label %6150, label %6127, !llvm.loop !11

6150:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp128()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 128)
  br label %6168

6151:
  %6152 = add nuw nsw i64 %6169, 1
  %6153 = getelementptr inbounds nuw i8, ptr @dst, i64 %6152
  %6154 = load i8, ptr %6153, align 1, !tbaa !10
  %6155 = getelementptr inbounds nuw i8, ptr @ref, i64 %6152
  %6156 = load i8, ptr %6155, align 1, !tbaa !10
  %6157 = icmp eq i8 %6154, %6156
  br i1 %6157, label %6158, label %6175

6158:
  %6159 = add nuw nsw i64 %6169, 2
  %6160 = getelementptr inbounds nuw i8, ptr @dst, i64 %6159
  %6161 = load i8, ptr %6160, align 1, !tbaa !10
  %6162 = getelementptr inbounds nuw i8, ptr @ref, i64 %6159
  %6163 = load i8, ptr %6162, align 1, !tbaa !10
  %6164 = icmp eq i8 %6161, %6163
  br i1 %6164, label %6165, label %6175

6165:
  %6166 = add nuw nsw i64 %6169, 3
  %6167 = icmp eq i64 %6166, 1200
  br i1 %6167, label %6176, label %6168, !llvm.loop !144

6168:
  %6169 = phi i64 [ 0, %6150 ], [ %6166, %6165 ]
  %6170 = getelementptr inbounds nuw i8, ptr @dst, i64 %6169
  %6171 = load i8, ptr %6170, align 1, !tbaa !10
  %6172 = getelementptr inbounds nuw i8, ptr @ref, i64 %6169
  %6173 = load i8, ptr %6172, align 1, !tbaa !10
  %6174 = icmp eq i8 %6171, %6173
  br i1 %6174, label %6151, label %6175

6175:
  tail call void @report(i64 noundef 128)
  store i1 true, ptr @bad, align 4
  br label %6177

6176:
  br label %6177

6177:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6178

6178:
  %6179 = phi i64 [ 0, %6177 ], [ %6199, %6178 ]
  %6180 = trunc i64 %6179 to i8
  %6181 = mul i8 %6180, 7
  %6182 = or disjoint i8 %6181, 3
  %6183 = getelementptr inbounds nuw i8, ptr @src, i64 %6179
  store i8 %6182, ptr %6183, align 4, !tbaa !10
  %6184 = or disjoint i64 %6179, 1
  %6185 = trunc i64 %6184 to i8
  %6186 = mul i8 %6185, 7
  %6187 = add i8 %6186, 3
  %6188 = getelementptr inbounds nuw i8, ptr @src, i64 %6184
  store i8 %6187, ptr %6188, align 1, !tbaa !10
  %6189 = or disjoint i64 %6179, 2
  %6190 = trunc i64 %6189 to i8
  %6191 = mul i8 %6190, 7
  %6192 = add i8 %6191, 3
  %6193 = getelementptr inbounds nuw i8, ptr @src, i64 %6189
  store i8 %6192, ptr %6193, align 2, !tbaa !10
  %6194 = or disjoint i64 %6179, 3
  %6195 = trunc i64 %6194 to i8
  %6196 = mul i8 %6195, 7
  %6197 = add i8 %6196, 3
  %6198 = getelementptr inbounds nuw i8, ptr @src, i64 %6194
  store i8 %6197, ptr %6198, align 1, !tbaa !10
  %6199 = add nuw nsw i64 %6179, 4
  %6200 = icmp eq i64 %6199, 1200
  br i1 %6200, label %6201, label %6178, !llvm.loop !11

6201:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr128()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 128)
  br label %6219

6202:
  %6203 = add nuw nsw i64 %6220, 1
  %6204 = getelementptr inbounds nuw i8, ptr @dst, i64 %6203
  %6205 = load i8, ptr %6204, align 1, !tbaa !10
  %6206 = getelementptr inbounds nuw i8, ptr @ref, i64 %6203
  %6207 = load i8, ptr %6206, align 1, !tbaa !10
  %6208 = icmp eq i8 %6205, %6207
  br i1 %6208, label %6209, label %6226

6209:
  %6210 = add nuw nsw i64 %6220, 2
  %6211 = getelementptr inbounds nuw i8, ptr @dst, i64 %6210
  %6212 = load i8, ptr %6211, align 1, !tbaa !10
  %6213 = getelementptr inbounds nuw i8, ptr @ref, i64 %6210
  %6214 = load i8, ptr %6213, align 1, !tbaa !10
  %6215 = icmp eq i8 %6212, %6214
  br i1 %6215, label %6216, label %6226

6216:
  %6217 = add nuw nsw i64 %6220, 3
  %6218 = icmp eq i64 %6217, 1200
  br i1 %6218, label %6227, label %6219, !llvm.loop !144

6219:
  %6220 = phi i64 [ 0, %6201 ], [ %6217, %6216 ]
  %6221 = getelementptr inbounds nuw i8, ptr @dst, i64 %6220
  %6222 = load i8, ptr %6221, align 1, !tbaa !10
  %6223 = getelementptr inbounds nuw i8, ptr @ref, i64 %6220
  %6224 = load i8, ptr %6223, align 1, !tbaa !10
  %6225 = icmp eq i8 %6222, %6224
  br i1 %6225, label %6202, label %6226

6226:
  tail call void @report(i64 noundef 128)
  store i1 true, ptr @bad, align 4
  br label %6228

6227:
  br label %6228

6228:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6229

6229:
  %6230 = phi i64 [ 0, %6228 ], [ %6250, %6229 ]
  %6231 = trunc i64 %6230 to i8
  %6232 = mul i8 %6231, 7
  %6233 = or disjoint i8 %6232, 3
  %6234 = getelementptr inbounds nuw i8, ptr @src, i64 %6230
  store i8 %6233, ptr %6234, align 4, !tbaa !10
  %6235 = or disjoint i64 %6230, 1
  %6236 = trunc i64 %6235 to i8
  %6237 = mul i8 %6236, 7
  %6238 = add i8 %6237, 3
  %6239 = getelementptr inbounds nuw i8, ptr @src, i64 %6235
  store i8 %6238, ptr %6239, align 1, !tbaa !10
  %6240 = or disjoint i64 %6230, 2
  %6241 = trunc i64 %6240 to i8
  %6242 = mul i8 %6241, 7
  %6243 = add i8 %6242, 3
  %6244 = getelementptr inbounds nuw i8, ptr @src, i64 %6240
  store i8 %6243, ptr %6244, align 2, !tbaa !10
  %6245 = or disjoint i64 %6230, 3
  %6246 = trunc i64 %6245 to i8
  %6247 = mul i8 %6246, 7
  %6248 = add i8 %6247, 3
  %6249 = getelementptr inbounds nuw i8, ptr @src, i64 %6245
  store i8 %6248, ptr %6249, align 1, !tbaa !10
  %6250 = add nuw nsw i64 %6230, 4
  %6251 = icmp eq i64 %6250, 1200
  br i1 %6251, label %6252, label %6229, !llvm.loop !11

6252:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7128()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 128)
  br label %6270

6253:
  %6254 = add nuw nsw i64 %6271, 1
  %6255 = getelementptr inbounds nuw i8, ptr @dst, i64 %6254
  %6256 = load i8, ptr %6255, align 1, !tbaa !10
  %6257 = getelementptr inbounds nuw i8, ptr @ref, i64 %6254
  %6258 = load i8, ptr %6257, align 1, !tbaa !10
  %6259 = icmp eq i8 %6256, %6258
  br i1 %6259, label %6260, label %6277

6260:
  %6261 = add nuw nsw i64 %6271, 2
  %6262 = getelementptr inbounds nuw i8, ptr @dst, i64 %6261
  %6263 = load i8, ptr %6262, align 1, !tbaa !10
  %6264 = getelementptr inbounds nuw i8, ptr @ref, i64 %6261
  %6265 = load i8, ptr %6264, align 1, !tbaa !10
  %6266 = icmp eq i8 %6263, %6265
  br i1 %6266, label %6267, label %6277

6267:
  %6268 = add nuw nsw i64 %6271, 3
  %6269 = icmp eq i64 %6268, 1200
  br i1 %6269, label %6278, label %6270, !llvm.loop !144

6270:
  %6271 = phi i64 [ 0, %6252 ], [ %6268, %6267 ]
  %6272 = getelementptr inbounds nuw i8, ptr @dst, i64 %6271
  %6273 = load i8, ptr %6272, align 1, !tbaa !10
  %6274 = getelementptr inbounds nuw i8, ptr @ref, i64 %6271
  %6275 = load i8, ptr %6274, align 1, !tbaa !10
  %6276 = icmp eq i8 %6273, %6275
  br i1 %6276, label %6253, label %6277

6277:
  tail call void @report(i64 noundef 128)
  store i1 true, ptr @bad, align 4
  br label %6279

6278:
  br label %6279

6279:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6280

6280:
  %6281 = phi i64 [ 0, %6279 ], [ %6301, %6280 ]
  %6282 = trunc i64 %6281 to i8
  %6283 = mul i8 %6282, 7
  %6284 = or disjoint i8 %6283, 3
  %6285 = getelementptr inbounds nuw i8, ptr @src, i64 %6281
  store i8 %6284, ptr %6285, align 4, !tbaa !10
  %6286 = or disjoint i64 %6281, 1
  %6287 = trunc i64 %6286 to i8
  %6288 = mul i8 %6287, 7
  %6289 = add i8 %6288, 3
  %6290 = getelementptr inbounds nuw i8, ptr @src, i64 %6286
  store i8 %6289, ptr %6290, align 1, !tbaa !10
  %6291 = or disjoint i64 %6281, 2
  %6292 = trunc i64 %6291 to i8
  %6293 = mul i8 %6292, 7
  %6294 = add i8 %6293, 3
  %6295 = getelementptr inbounds nuw i8, ptr @src, i64 %6291
  store i8 %6294, ptr %6295, align 2, !tbaa !10
  %6296 = or disjoint i64 %6281, 3
  %6297 = trunc i64 %6296 to i8
  %6298 = mul i8 %6297, 7
  %6299 = add i8 %6298, 3
  %6300 = getelementptr inbounds nuw i8, ptr @src, i64 %6296
  store i8 %6299, ptr %6300, align 1, !tbaa !10
  %6301 = add nuw nsw i64 %6281, 4
  %6302 = icmp eq i64 %6301, 1200
  br i1 %6302, label %6303, label %6280, !llvm.loop !11

6303:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr128()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 128)
  br label %6321

6304:
  %6305 = add nuw nsw i64 %6322, 1
  %6306 = getelementptr inbounds nuw i8, ptr @dst, i64 %6305
  %6307 = load i8, ptr %6306, align 1, !tbaa !10
  %6308 = getelementptr inbounds nuw i8, ptr @ref, i64 %6305
  %6309 = load i8, ptr %6308, align 1, !tbaa !10
  %6310 = icmp eq i8 %6307, %6309
  br i1 %6310, label %6311, label %6328

6311:
  %6312 = add nuw nsw i64 %6322, 2
  %6313 = getelementptr inbounds nuw i8, ptr @dst, i64 %6312
  %6314 = load i8, ptr %6313, align 1, !tbaa !10
  %6315 = getelementptr inbounds nuw i8, ptr @ref, i64 %6312
  %6316 = load i8, ptr %6315, align 1, !tbaa !10
  %6317 = icmp eq i8 %6314, %6316
  br i1 %6317, label %6318, label %6328

6318:
  %6319 = add nuw nsw i64 %6322, 3
  %6320 = icmp eq i64 %6319, 1200
  br i1 %6320, label %6329, label %6321, !llvm.loop !144

6321:
  %6322 = phi i64 [ 0, %6303 ], [ %6319, %6318 ]
  %6323 = getelementptr inbounds nuw i8, ptr @dst, i64 %6322
  %6324 = load i8, ptr %6323, align 1, !tbaa !10
  %6325 = getelementptr inbounds nuw i8, ptr @ref, i64 %6322
  %6326 = load i8, ptr %6325, align 1, !tbaa !10
  %6327 = icmp eq i8 %6324, %6326
  br i1 %6327, label %6304, label %6328

6328:
  tail call void @report(i64 noundef 128)
  store i1 true, ptr @bad, align 4
  br label %6330

6329:
  br label %6330

6330:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6331

6331:
  %6332 = phi i64 [ 0, %6330 ], [ %6352, %6331 ]
  %6333 = trunc i64 %6332 to i8
  %6334 = mul i8 %6333, 7
  %6335 = or disjoint i8 %6334, 3
  %6336 = getelementptr inbounds nuw i8, ptr @src, i64 %6332
  store i8 %6335, ptr %6336, align 4, !tbaa !10
  %6337 = or disjoint i64 %6332, 1
  %6338 = trunc i64 %6337 to i8
  %6339 = mul i8 %6338, 7
  %6340 = add i8 %6339, 3
  %6341 = getelementptr inbounds nuw i8, ptr @src, i64 %6337
  store i8 %6340, ptr %6341, align 1, !tbaa !10
  %6342 = or disjoint i64 %6332, 2
  %6343 = trunc i64 %6342 to i8
  %6344 = mul i8 %6343, 7
  %6345 = add i8 %6344, 3
  %6346 = getelementptr inbounds nuw i8, ptr @src, i64 %6342
  store i8 %6345, ptr %6346, align 2, !tbaa !10
  %6347 = or disjoint i64 %6332, 3
  %6348 = trunc i64 %6347 to i8
  %6349 = mul i8 %6348, 7
  %6350 = add i8 %6349, 3
  %6351 = getelementptr inbounds nuw i8, ptr @src, i64 %6347
  store i8 %6350, ptr %6351, align 1, !tbaa !10
  %6352 = add nuw nsw i64 %6332, 4
  %6353 = icmp eq i64 %6352, 1200
  br i1 %6353, label %6354, label %6331, !llvm.loop !11

6354:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv128(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 128)
  br label %6372

6355:
  %6356 = add nuw nsw i64 %6373, 1
  %6357 = getelementptr inbounds nuw i8, ptr @dst, i64 %6356
  %6358 = load i8, ptr %6357, align 1, !tbaa !10
  %6359 = getelementptr inbounds nuw i8, ptr @ref, i64 %6356
  %6360 = load i8, ptr %6359, align 1, !tbaa !10
  %6361 = icmp eq i8 %6358, %6360
  br i1 %6361, label %6362, label %6379

6362:
  %6363 = add nuw nsw i64 %6373, 2
  %6364 = getelementptr inbounds nuw i8, ptr @dst, i64 %6363
  %6365 = load i8, ptr %6364, align 1, !tbaa !10
  %6366 = getelementptr inbounds nuw i8, ptr @ref, i64 %6363
  %6367 = load i8, ptr %6366, align 1, !tbaa !10
  %6368 = icmp eq i8 %6365, %6367
  br i1 %6368, label %6369, label %6379

6369:
  %6370 = add nuw nsw i64 %6373, 3
  %6371 = icmp eq i64 %6370, 1200
  br i1 %6371, label %6380, label %6372, !llvm.loop !144

6372:
  %6373 = phi i64 [ 0, %6354 ], [ %6370, %6369 ]
  %6374 = getelementptr inbounds nuw i8, ptr @dst, i64 %6373
  %6375 = load i8, ptr %6374, align 1, !tbaa !10
  %6376 = getelementptr inbounds nuw i8, ptr @ref, i64 %6373
  %6377 = load i8, ptr %6376, align 1, !tbaa !10
  %6378 = icmp eq i8 %6375, %6377
  br i1 %6378, label %6355, label %6379

6379:
  tail call void @report(i64 noundef 128)
  store i1 true, ptr @bad, align 4
  br label %6381

6380:
  br label %6381

6381:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6382

6382:
  %6383 = phi i64 [ 0, %6381 ], [ %6403, %6382 ]
  %6384 = trunc i64 %6383 to i8
  %6385 = mul i8 %6384, 7
  %6386 = or disjoint i8 %6385, 3
  %6387 = getelementptr inbounds nuw i8, ptr @src, i64 %6383
  store i8 %6386, ptr %6387, align 4, !tbaa !10
  %6388 = or disjoint i64 %6383, 1
  %6389 = trunc i64 %6388 to i8
  %6390 = mul i8 %6389, 7
  %6391 = add i8 %6390, 3
  %6392 = getelementptr inbounds nuw i8, ptr @src, i64 %6388
  store i8 %6391, ptr %6392, align 1, !tbaa !10
  %6393 = or disjoint i64 %6383, 2
  %6394 = trunc i64 %6393 to i8
  %6395 = mul i8 %6394, 7
  %6396 = add i8 %6395, 3
  %6397 = getelementptr inbounds nuw i8, ptr @src, i64 %6393
  store i8 %6396, ptr %6397, align 2, !tbaa !10
  %6398 = or disjoint i64 %6383, 3
  %6399 = trunc i64 %6398 to i8
  %6400 = mul i8 %6399, 7
  %6401 = add i8 %6400, 3
  %6402 = getelementptr inbounds nuw i8, ptr @src, i64 %6398
  store i8 %6401, ptr %6402, align 1, !tbaa !10
  %6403 = add nuw nsw i64 %6383, 4
  %6404 = icmp eq i64 %6403, 1200
  br i1 %6404, label %6405, label %6382, !llvm.loop !11

6405:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv128(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 128)
  br label %6423

6406:
  %6407 = add nuw nsw i64 %6424, 1
  %6408 = getelementptr inbounds nuw i8, ptr @dst, i64 %6407
  %6409 = load i8, ptr %6408, align 1, !tbaa !10
  %6410 = getelementptr inbounds nuw i8, ptr @ref, i64 %6407
  %6411 = load i8, ptr %6410, align 1, !tbaa !10
  %6412 = icmp eq i8 %6409, %6411
  br i1 %6412, label %6413, label %6430

6413:
  %6414 = add nuw nsw i64 %6424, 2
  %6415 = getelementptr inbounds nuw i8, ptr @dst, i64 %6414
  %6416 = load i8, ptr %6415, align 1, !tbaa !10
  %6417 = getelementptr inbounds nuw i8, ptr @ref, i64 %6414
  %6418 = load i8, ptr %6417, align 1, !tbaa !10
  %6419 = icmp eq i8 %6416, %6418
  br i1 %6419, label %6420, label %6430

6420:
  %6421 = add nuw nsw i64 %6424, 3
  %6422 = icmp eq i64 %6421, 1200
  br i1 %6422, label %6431, label %6423, !llvm.loop !144

6423:
  %6424 = phi i64 [ 0, %6405 ], [ %6421, %6420 ]
  %6425 = getelementptr inbounds nuw i8, ptr @dst, i64 %6424
  %6426 = load i8, ptr %6425, align 1, !tbaa !10
  %6427 = getelementptr inbounds nuw i8, ptr @ref, i64 %6424
  %6428 = load i8, ptr %6427, align 1, !tbaa !10
  %6429 = icmp eq i8 %6426, %6428
  br i1 %6429, label %6406, label %6430

6430:
  tail call void @report(i64 noundef 128)
  store i1 true, ptr @bad, align 4
  br label %6432

6431:
  br label %6432

6432:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6433

6433:
  %6434 = phi i64 [ 0, %6432 ], [ %6454, %6433 ]
  %6435 = trunc i64 %6434 to i8
  %6436 = mul i8 %6435, 7
  %6437 = or disjoint i8 %6436, 3
  %6438 = getelementptr inbounds nuw i8, ptr @src, i64 %6434
  store i8 %6437, ptr %6438, align 4, !tbaa !10
  %6439 = or disjoint i64 %6434, 1
  %6440 = trunc i64 %6439 to i8
  %6441 = mul i8 %6440, 7
  %6442 = add i8 %6441, 3
  %6443 = getelementptr inbounds nuw i8, ptr @src, i64 %6439
  store i8 %6442, ptr %6443, align 1, !tbaa !10
  %6444 = or disjoint i64 %6434, 2
  %6445 = trunc i64 %6444 to i8
  %6446 = mul i8 %6445, 7
  %6447 = add i8 %6446, 3
  %6448 = getelementptr inbounds nuw i8, ptr @src, i64 %6444
  store i8 %6447, ptr %6448, align 2, !tbaa !10
  %6449 = or disjoint i64 %6434, 3
  %6450 = trunc i64 %6449 to i8
  %6451 = mul i8 %6450, 7
  %6452 = add i8 %6451, 3
  %6453 = getelementptr inbounds nuw i8, ptr @src, i64 %6449
  store i8 %6452, ptr %6453, align 1, !tbaa !10
  %6454 = add nuw nsw i64 %6434, 4
  %6455 = icmp eq i64 %6454, 1200
  br i1 %6455, label %6456, label %6433, !llvm.loop !11

6456:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp129()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 129)
  br label %6474

6457:
  %6458 = add nuw nsw i64 %6475, 1
  %6459 = getelementptr inbounds nuw i8, ptr @dst, i64 %6458
  %6460 = load i8, ptr %6459, align 1, !tbaa !10
  %6461 = getelementptr inbounds nuw i8, ptr @ref, i64 %6458
  %6462 = load i8, ptr %6461, align 1, !tbaa !10
  %6463 = icmp eq i8 %6460, %6462
  br i1 %6463, label %6464, label %6481

6464:
  %6465 = add nuw nsw i64 %6475, 2
  %6466 = getelementptr inbounds nuw i8, ptr @dst, i64 %6465
  %6467 = load i8, ptr %6466, align 1, !tbaa !10
  %6468 = getelementptr inbounds nuw i8, ptr @ref, i64 %6465
  %6469 = load i8, ptr %6468, align 1, !tbaa !10
  %6470 = icmp eq i8 %6467, %6469
  br i1 %6470, label %6471, label %6481

6471:
  %6472 = add nuw nsw i64 %6475, 3
  %6473 = icmp eq i64 %6472, 1200
  br i1 %6473, label %6482, label %6474, !llvm.loop !144

6474:
  %6475 = phi i64 [ 0, %6456 ], [ %6472, %6471 ]
  %6476 = getelementptr inbounds nuw i8, ptr @dst, i64 %6475
  %6477 = load i8, ptr %6476, align 1, !tbaa !10
  %6478 = getelementptr inbounds nuw i8, ptr @ref, i64 %6475
  %6479 = load i8, ptr %6478, align 1, !tbaa !10
  %6480 = icmp eq i8 %6477, %6479
  br i1 %6480, label %6457, label %6481

6481:
  tail call void @report(i64 noundef 129)
  store i1 true, ptr @bad, align 4
  br label %6483

6482:
  br label %6483

6483:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6484

6484:
  %6485 = phi i64 [ 0, %6483 ], [ %6505, %6484 ]
  %6486 = trunc i64 %6485 to i8
  %6487 = mul i8 %6486, 7
  %6488 = or disjoint i8 %6487, 3
  %6489 = getelementptr inbounds nuw i8, ptr @src, i64 %6485
  store i8 %6488, ptr %6489, align 4, !tbaa !10
  %6490 = or disjoint i64 %6485, 1
  %6491 = trunc i64 %6490 to i8
  %6492 = mul i8 %6491, 7
  %6493 = add i8 %6492, 3
  %6494 = getelementptr inbounds nuw i8, ptr @src, i64 %6490
  store i8 %6493, ptr %6494, align 1, !tbaa !10
  %6495 = or disjoint i64 %6485, 2
  %6496 = trunc i64 %6495 to i8
  %6497 = mul i8 %6496, 7
  %6498 = add i8 %6497, 3
  %6499 = getelementptr inbounds nuw i8, ptr @src, i64 %6495
  store i8 %6498, ptr %6499, align 2, !tbaa !10
  %6500 = or disjoint i64 %6485, 3
  %6501 = trunc i64 %6500 to i8
  %6502 = mul i8 %6501, 7
  %6503 = add i8 %6502, 3
  %6504 = getelementptr inbounds nuw i8, ptr @src, i64 %6500
  store i8 %6503, ptr %6504, align 1, !tbaa !10
  %6505 = add nuw nsw i64 %6485, 4
  %6506 = icmp eq i64 %6505, 1200
  br i1 %6506, label %6507, label %6484, !llvm.loop !11

6507:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr129()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 129)
  br label %6525

6508:
  %6509 = add nuw nsw i64 %6526, 1
  %6510 = getelementptr inbounds nuw i8, ptr @dst, i64 %6509
  %6511 = load i8, ptr %6510, align 1, !tbaa !10
  %6512 = getelementptr inbounds nuw i8, ptr @ref, i64 %6509
  %6513 = load i8, ptr %6512, align 1, !tbaa !10
  %6514 = icmp eq i8 %6511, %6513
  br i1 %6514, label %6515, label %6532

6515:
  %6516 = add nuw nsw i64 %6526, 2
  %6517 = getelementptr inbounds nuw i8, ptr @dst, i64 %6516
  %6518 = load i8, ptr %6517, align 1, !tbaa !10
  %6519 = getelementptr inbounds nuw i8, ptr @ref, i64 %6516
  %6520 = load i8, ptr %6519, align 1, !tbaa !10
  %6521 = icmp eq i8 %6518, %6520
  br i1 %6521, label %6522, label %6532

6522:
  %6523 = add nuw nsw i64 %6526, 3
  %6524 = icmp eq i64 %6523, 1200
  br i1 %6524, label %6533, label %6525, !llvm.loop !144

6525:
  %6526 = phi i64 [ 0, %6507 ], [ %6523, %6522 ]
  %6527 = getelementptr inbounds nuw i8, ptr @dst, i64 %6526
  %6528 = load i8, ptr %6527, align 1, !tbaa !10
  %6529 = getelementptr inbounds nuw i8, ptr @ref, i64 %6526
  %6530 = load i8, ptr %6529, align 1, !tbaa !10
  %6531 = icmp eq i8 %6528, %6530
  br i1 %6531, label %6508, label %6532

6532:
  tail call void @report(i64 noundef 129)
  store i1 true, ptr @bad, align 4
  br label %6534

6533:
  br label %6534

6534:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6535

6535:
  %6536 = phi i64 [ 0, %6534 ], [ %6556, %6535 ]
  %6537 = trunc i64 %6536 to i8
  %6538 = mul i8 %6537, 7
  %6539 = or disjoint i8 %6538, 3
  %6540 = getelementptr inbounds nuw i8, ptr @src, i64 %6536
  store i8 %6539, ptr %6540, align 4, !tbaa !10
  %6541 = or disjoint i64 %6536, 1
  %6542 = trunc i64 %6541 to i8
  %6543 = mul i8 %6542, 7
  %6544 = add i8 %6543, 3
  %6545 = getelementptr inbounds nuw i8, ptr @src, i64 %6541
  store i8 %6544, ptr %6545, align 1, !tbaa !10
  %6546 = or disjoint i64 %6536, 2
  %6547 = trunc i64 %6546 to i8
  %6548 = mul i8 %6547, 7
  %6549 = add i8 %6548, 3
  %6550 = getelementptr inbounds nuw i8, ptr @src, i64 %6546
  store i8 %6549, ptr %6550, align 2, !tbaa !10
  %6551 = or disjoint i64 %6536, 3
  %6552 = trunc i64 %6551 to i8
  %6553 = mul i8 %6552, 7
  %6554 = add i8 %6553, 3
  %6555 = getelementptr inbounds nuw i8, ptr @src, i64 %6551
  store i8 %6554, ptr %6555, align 1, !tbaa !10
  %6556 = add nuw nsw i64 %6536, 4
  %6557 = icmp eq i64 %6556, 1200
  br i1 %6557, label %6558, label %6535, !llvm.loop !11

6558:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7129()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 129)
  br label %6576

6559:
  %6560 = add nuw nsw i64 %6577, 1
  %6561 = getelementptr inbounds nuw i8, ptr @dst, i64 %6560
  %6562 = load i8, ptr %6561, align 1, !tbaa !10
  %6563 = getelementptr inbounds nuw i8, ptr @ref, i64 %6560
  %6564 = load i8, ptr %6563, align 1, !tbaa !10
  %6565 = icmp eq i8 %6562, %6564
  br i1 %6565, label %6566, label %6583

6566:
  %6567 = add nuw nsw i64 %6577, 2
  %6568 = getelementptr inbounds nuw i8, ptr @dst, i64 %6567
  %6569 = load i8, ptr %6568, align 1, !tbaa !10
  %6570 = getelementptr inbounds nuw i8, ptr @ref, i64 %6567
  %6571 = load i8, ptr %6570, align 1, !tbaa !10
  %6572 = icmp eq i8 %6569, %6571
  br i1 %6572, label %6573, label %6583

6573:
  %6574 = add nuw nsw i64 %6577, 3
  %6575 = icmp eq i64 %6574, 1200
  br i1 %6575, label %6584, label %6576, !llvm.loop !144

6576:
  %6577 = phi i64 [ 0, %6558 ], [ %6574, %6573 ]
  %6578 = getelementptr inbounds nuw i8, ptr @dst, i64 %6577
  %6579 = load i8, ptr %6578, align 1, !tbaa !10
  %6580 = getelementptr inbounds nuw i8, ptr @ref, i64 %6577
  %6581 = load i8, ptr %6580, align 1, !tbaa !10
  %6582 = icmp eq i8 %6579, %6581
  br i1 %6582, label %6559, label %6583

6583:
  tail call void @report(i64 noundef 129)
  store i1 true, ptr @bad, align 4
  br label %6585

6584:
  br label %6585

6585:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6586

6586:
  %6587 = phi i64 [ 0, %6585 ], [ %6607, %6586 ]
  %6588 = trunc i64 %6587 to i8
  %6589 = mul i8 %6588, 7
  %6590 = or disjoint i8 %6589, 3
  %6591 = getelementptr inbounds nuw i8, ptr @src, i64 %6587
  store i8 %6590, ptr %6591, align 4, !tbaa !10
  %6592 = or disjoint i64 %6587, 1
  %6593 = trunc i64 %6592 to i8
  %6594 = mul i8 %6593, 7
  %6595 = add i8 %6594, 3
  %6596 = getelementptr inbounds nuw i8, ptr @src, i64 %6592
  store i8 %6595, ptr %6596, align 1, !tbaa !10
  %6597 = or disjoint i64 %6587, 2
  %6598 = trunc i64 %6597 to i8
  %6599 = mul i8 %6598, 7
  %6600 = add i8 %6599, 3
  %6601 = getelementptr inbounds nuw i8, ptr @src, i64 %6597
  store i8 %6600, ptr %6601, align 2, !tbaa !10
  %6602 = or disjoint i64 %6587, 3
  %6603 = trunc i64 %6602 to i8
  %6604 = mul i8 %6603, 7
  %6605 = add i8 %6604, 3
  %6606 = getelementptr inbounds nuw i8, ptr @src, i64 %6602
  store i8 %6605, ptr %6606, align 1, !tbaa !10
  %6607 = add nuw nsw i64 %6587, 4
  %6608 = icmp eq i64 %6607, 1200
  br i1 %6608, label %6609, label %6586, !llvm.loop !11

6609:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr129()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 129)
  br label %6627

6610:
  %6611 = add nuw nsw i64 %6628, 1
  %6612 = getelementptr inbounds nuw i8, ptr @dst, i64 %6611
  %6613 = load i8, ptr %6612, align 1, !tbaa !10
  %6614 = getelementptr inbounds nuw i8, ptr @ref, i64 %6611
  %6615 = load i8, ptr %6614, align 1, !tbaa !10
  %6616 = icmp eq i8 %6613, %6615
  br i1 %6616, label %6617, label %6634

6617:
  %6618 = add nuw nsw i64 %6628, 2
  %6619 = getelementptr inbounds nuw i8, ptr @dst, i64 %6618
  %6620 = load i8, ptr %6619, align 1, !tbaa !10
  %6621 = getelementptr inbounds nuw i8, ptr @ref, i64 %6618
  %6622 = load i8, ptr %6621, align 1, !tbaa !10
  %6623 = icmp eq i8 %6620, %6622
  br i1 %6623, label %6624, label %6634

6624:
  %6625 = add nuw nsw i64 %6628, 3
  %6626 = icmp eq i64 %6625, 1200
  br i1 %6626, label %6635, label %6627, !llvm.loop !144

6627:
  %6628 = phi i64 [ 0, %6609 ], [ %6625, %6624 ]
  %6629 = getelementptr inbounds nuw i8, ptr @dst, i64 %6628
  %6630 = load i8, ptr %6629, align 1, !tbaa !10
  %6631 = getelementptr inbounds nuw i8, ptr @ref, i64 %6628
  %6632 = load i8, ptr %6631, align 1, !tbaa !10
  %6633 = icmp eq i8 %6630, %6632
  br i1 %6633, label %6610, label %6634

6634:
  tail call void @report(i64 noundef 129)
  store i1 true, ptr @bad, align 4
  br label %6636

6635:
  br label %6636

6636:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6637

6637:
  %6638 = phi i64 [ 0, %6636 ], [ %6658, %6637 ]
  %6639 = trunc i64 %6638 to i8
  %6640 = mul i8 %6639, 7
  %6641 = or disjoint i8 %6640, 3
  %6642 = getelementptr inbounds nuw i8, ptr @src, i64 %6638
  store i8 %6641, ptr %6642, align 4, !tbaa !10
  %6643 = or disjoint i64 %6638, 1
  %6644 = trunc i64 %6643 to i8
  %6645 = mul i8 %6644, 7
  %6646 = add i8 %6645, 3
  %6647 = getelementptr inbounds nuw i8, ptr @src, i64 %6643
  store i8 %6646, ptr %6647, align 1, !tbaa !10
  %6648 = or disjoint i64 %6638, 2
  %6649 = trunc i64 %6648 to i8
  %6650 = mul i8 %6649, 7
  %6651 = add i8 %6650, 3
  %6652 = getelementptr inbounds nuw i8, ptr @src, i64 %6648
  store i8 %6651, ptr %6652, align 2, !tbaa !10
  %6653 = or disjoint i64 %6638, 3
  %6654 = trunc i64 %6653 to i8
  %6655 = mul i8 %6654, 7
  %6656 = add i8 %6655, 3
  %6657 = getelementptr inbounds nuw i8, ptr @src, i64 %6653
  store i8 %6656, ptr %6657, align 1, !tbaa !10
  %6658 = add nuw nsw i64 %6638, 4
  %6659 = icmp eq i64 %6658, 1200
  br i1 %6659, label %6660, label %6637, !llvm.loop !11

6660:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv129(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 129)
  br label %6678

6661:
  %6662 = add nuw nsw i64 %6679, 1
  %6663 = getelementptr inbounds nuw i8, ptr @dst, i64 %6662
  %6664 = load i8, ptr %6663, align 1, !tbaa !10
  %6665 = getelementptr inbounds nuw i8, ptr @ref, i64 %6662
  %6666 = load i8, ptr %6665, align 1, !tbaa !10
  %6667 = icmp eq i8 %6664, %6666
  br i1 %6667, label %6668, label %6685

6668:
  %6669 = add nuw nsw i64 %6679, 2
  %6670 = getelementptr inbounds nuw i8, ptr @dst, i64 %6669
  %6671 = load i8, ptr %6670, align 1, !tbaa !10
  %6672 = getelementptr inbounds nuw i8, ptr @ref, i64 %6669
  %6673 = load i8, ptr %6672, align 1, !tbaa !10
  %6674 = icmp eq i8 %6671, %6673
  br i1 %6674, label %6675, label %6685

6675:
  %6676 = add nuw nsw i64 %6679, 3
  %6677 = icmp eq i64 %6676, 1200
  br i1 %6677, label %6686, label %6678, !llvm.loop !144

6678:
  %6679 = phi i64 [ 0, %6660 ], [ %6676, %6675 ]
  %6680 = getelementptr inbounds nuw i8, ptr @dst, i64 %6679
  %6681 = load i8, ptr %6680, align 1, !tbaa !10
  %6682 = getelementptr inbounds nuw i8, ptr @ref, i64 %6679
  %6683 = load i8, ptr %6682, align 1, !tbaa !10
  %6684 = icmp eq i8 %6681, %6683
  br i1 %6684, label %6661, label %6685

6685:
  tail call void @report(i64 noundef 129)
  store i1 true, ptr @bad, align 4
  br label %6687

6686:
  br label %6687

6687:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6688

6688:
  %6689 = phi i64 [ 0, %6687 ], [ %6709, %6688 ]
  %6690 = trunc i64 %6689 to i8
  %6691 = mul i8 %6690, 7
  %6692 = or disjoint i8 %6691, 3
  %6693 = getelementptr inbounds nuw i8, ptr @src, i64 %6689
  store i8 %6692, ptr %6693, align 4, !tbaa !10
  %6694 = or disjoint i64 %6689, 1
  %6695 = trunc i64 %6694 to i8
  %6696 = mul i8 %6695, 7
  %6697 = add i8 %6696, 3
  %6698 = getelementptr inbounds nuw i8, ptr @src, i64 %6694
  store i8 %6697, ptr %6698, align 1, !tbaa !10
  %6699 = or disjoint i64 %6689, 2
  %6700 = trunc i64 %6699 to i8
  %6701 = mul i8 %6700, 7
  %6702 = add i8 %6701, 3
  %6703 = getelementptr inbounds nuw i8, ptr @src, i64 %6699
  store i8 %6702, ptr %6703, align 2, !tbaa !10
  %6704 = or disjoint i64 %6689, 3
  %6705 = trunc i64 %6704 to i8
  %6706 = mul i8 %6705, 7
  %6707 = add i8 %6706, 3
  %6708 = getelementptr inbounds nuw i8, ptr @src, i64 %6704
  store i8 %6707, ptr %6708, align 1, !tbaa !10
  %6709 = add nuw nsw i64 %6689, 4
  %6710 = icmp eq i64 %6709, 1200
  br i1 %6710, label %6711, label %6688, !llvm.loop !11

6711:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv129(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 129)
  br label %6729

6712:
  %6713 = add nuw nsw i64 %6730, 1
  %6714 = getelementptr inbounds nuw i8, ptr @dst, i64 %6713
  %6715 = load i8, ptr %6714, align 1, !tbaa !10
  %6716 = getelementptr inbounds nuw i8, ptr @ref, i64 %6713
  %6717 = load i8, ptr %6716, align 1, !tbaa !10
  %6718 = icmp eq i8 %6715, %6717
  br i1 %6718, label %6719, label %6736

6719:
  %6720 = add nuw nsw i64 %6730, 2
  %6721 = getelementptr inbounds nuw i8, ptr @dst, i64 %6720
  %6722 = load i8, ptr %6721, align 1, !tbaa !10
  %6723 = getelementptr inbounds nuw i8, ptr @ref, i64 %6720
  %6724 = load i8, ptr %6723, align 1, !tbaa !10
  %6725 = icmp eq i8 %6722, %6724
  br i1 %6725, label %6726, label %6736

6726:
  %6727 = add nuw nsw i64 %6730, 3
  %6728 = icmp eq i64 %6727, 1200
  br i1 %6728, label %6737, label %6729, !llvm.loop !144

6729:
  %6730 = phi i64 [ 0, %6711 ], [ %6727, %6726 ]
  %6731 = getelementptr inbounds nuw i8, ptr @dst, i64 %6730
  %6732 = load i8, ptr %6731, align 1, !tbaa !10
  %6733 = getelementptr inbounds nuw i8, ptr @ref, i64 %6730
  %6734 = load i8, ptr %6733, align 1, !tbaa !10
  %6735 = icmp eq i8 %6732, %6734
  br i1 %6735, label %6712, label %6736

6736:
  tail call void @report(i64 noundef 129)
  store i1 true, ptr @bad, align 4
  br label %6738

6737:
  br label %6738

6738:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6739

6739:
  %6740 = phi i64 [ 0, %6738 ], [ %6760, %6739 ]
  %6741 = trunc i64 %6740 to i8
  %6742 = mul i8 %6741, 7
  %6743 = or disjoint i8 %6742, 3
  %6744 = getelementptr inbounds nuw i8, ptr @src, i64 %6740
  store i8 %6743, ptr %6744, align 4, !tbaa !10
  %6745 = or disjoint i64 %6740, 1
  %6746 = trunc i64 %6745 to i8
  %6747 = mul i8 %6746, 7
  %6748 = add i8 %6747, 3
  %6749 = getelementptr inbounds nuw i8, ptr @src, i64 %6745
  store i8 %6748, ptr %6749, align 1, !tbaa !10
  %6750 = or disjoint i64 %6740, 2
  %6751 = trunc i64 %6750 to i8
  %6752 = mul i8 %6751, 7
  %6753 = add i8 %6752, 3
  %6754 = getelementptr inbounds nuw i8, ptr @src, i64 %6750
  store i8 %6753, ptr %6754, align 2, !tbaa !10
  %6755 = or disjoint i64 %6740, 3
  %6756 = trunc i64 %6755 to i8
  %6757 = mul i8 %6756, 7
  %6758 = add i8 %6757, 3
  %6759 = getelementptr inbounds nuw i8, ptr @src, i64 %6755
  store i8 %6758, ptr %6759, align 1, !tbaa !10
  %6760 = add nuw nsw i64 %6740, 4
  %6761 = icmp eq i64 %6760, 1200
  br i1 %6761, label %6762, label %6739, !llvm.loop !11

6762:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp200()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 200)
  br label %6780

6763:
  %6764 = add nuw nsw i64 %6781, 1
  %6765 = getelementptr inbounds nuw i8, ptr @dst, i64 %6764
  %6766 = load i8, ptr %6765, align 1, !tbaa !10
  %6767 = getelementptr inbounds nuw i8, ptr @ref, i64 %6764
  %6768 = load i8, ptr %6767, align 1, !tbaa !10
  %6769 = icmp eq i8 %6766, %6768
  br i1 %6769, label %6770, label %6787

6770:
  %6771 = add nuw nsw i64 %6781, 2
  %6772 = getelementptr inbounds nuw i8, ptr @dst, i64 %6771
  %6773 = load i8, ptr %6772, align 1, !tbaa !10
  %6774 = getelementptr inbounds nuw i8, ptr @ref, i64 %6771
  %6775 = load i8, ptr %6774, align 1, !tbaa !10
  %6776 = icmp eq i8 %6773, %6775
  br i1 %6776, label %6777, label %6787

6777:
  %6778 = add nuw nsw i64 %6781, 3
  %6779 = icmp eq i64 %6778, 1200
  br i1 %6779, label %6788, label %6780, !llvm.loop !144

6780:
  %6781 = phi i64 [ 0, %6762 ], [ %6778, %6777 ]
  %6782 = getelementptr inbounds nuw i8, ptr @dst, i64 %6781
  %6783 = load i8, ptr %6782, align 1, !tbaa !10
  %6784 = getelementptr inbounds nuw i8, ptr @ref, i64 %6781
  %6785 = load i8, ptr %6784, align 1, !tbaa !10
  %6786 = icmp eq i8 %6783, %6785
  br i1 %6786, label %6763, label %6787

6787:
  tail call void @report(i64 noundef 200)
  store i1 true, ptr @bad, align 4
  br label %6789

6788:
  br label %6789

6789:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6790

6790:
  %6791 = phi i64 [ 0, %6789 ], [ %6811, %6790 ]
  %6792 = trunc i64 %6791 to i8
  %6793 = mul i8 %6792, 7
  %6794 = or disjoint i8 %6793, 3
  %6795 = getelementptr inbounds nuw i8, ptr @src, i64 %6791
  store i8 %6794, ptr %6795, align 4, !tbaa !10
  %6796 = or disjoint i64 %6791, 1
  %6797 = trunc i64 %6796 to i8
  %6798 = mul i8 %6797, 7
  %6799 = add i8 %6798, 3
  %6800 = getelementptr inbounds nuw i8, ptr @src, i64 %6796
  store i8 %6799, ptr %6800, align 1, !tbaa !10
  %6801 = or disjoint i64 %6791, 2
  %6802 = trunc i64 %6801 to i8
  %6803 = mul i8 %6802, 7
  %6804 = add i8 %6803, 3
  %6805 = getelementptr inbounds nuw i8, ptr @src, i64 %6801
  store i8 %6804, ptr %6805, align 2, !tbaa !10
  %6806 = or disjoint i64 %6791, 3
  %6807 = trunc i64 %6806 to i8
  %6808 = mul i8 %6807, 7
  %6809 = add i8 %6808, 3
  %6810 = getelementptr inbounds nuw i8, ptr @src, i64 %6806
  store i8 %6809, ptr %6810, align 1, !tbaa !10
  %6811 = add nuw nsw i64 %6791, 4
  %6812 = icmp eq i64 %6811, 1200
  br i1 %6812, label %6813, label %6790, !llvm.loop !11

6813:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr200()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 200)
  br label %6831

6814:
  %6815 = add nuw nsw i64 %6832, 1
  %6816 = getelementptr inbounds nuw i8, ptr @dst, i64 %6815
  %6817 = load i8, ptr %6816, align 1, !tbaa !10
  %6818 = getelementptr inbounds nuw i8, ptr @ref, i64 %6815
  %6819 = load i8, ptr %6818, align 1, !tbaa !10
  %6820 = icmp eq i8 %6817, %6819
  br i1 %6820, label %6821, label %6838

6821:
  %6822 = add nuw nsw i64 %6832, 2
  %6823 = getelementptr inbounds nuw i8, ptr @dst, i64 %6822
  %6824 = load i8, ptr %6823, align 1, !tbaa !10
  %6825 = getelementptr inbounds nuw i8, ptr @ref, i64 %6822
  %6826 = load i8, ptr %6825, align 1, !tbaa !10
  %6827 = icmp eq i8 %6824, %6826
  br i1 %6827, label %6828, label %6838

6828:
  %6829 = add nuw nsw i64 %6832, 3
  %6830 = icmp eq i64 %6829, 1200
  br i1 %6830, label %6839, label %6831, !llvm.loop !144

6831:
  %6832 = phi i64 [ 0, %6813 ], [ %6829, %6828 ]
  %6833 = getelementptr inbounds nuw i8, ptr @dst, i64 %6832
  %6834 = load i8, ptr %6833, align 1, !tbaa !10
  %6835 = getelementptr inbounds nuw i8, ptr @ref, i64 %6832
  %6836 = load i8, ptr %6835, align 1, !tbaa !10
  %6837 = icmp eq i8 %6834, %6836
  br i1 %6837, label %6814, label %6838

6838:
  tail call void @report(i64 noundef 200)
  store i1 true, ptr @bad, align 4
  br label %6840

6839:
  br label %6840

6840:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6841

6841:
  %6842 = phi i64 [ 0, %6840 ], [ %6862, %6841 ]
  %6843 = trunc i64 %6842 to i8
  %6844 = mul i8 %6843, 7
  %6845 = or disjoint i8 %6844, 3
  %6846 = getelementptr inbounds nuw i8, ptr @src, i64 %6842
  store i8 %6845, ptr %6846, align 4, !tbaa !10
  %6847 = or disjoint i64 %6842, 1
  %6848 = trunc i64 %6847 to i8
  %6849 = mul i8 %6848, 7
  %6850 = add i8 %6849, 3
  %6851 = getelementptr inbounds nuw i8, ptr @src, i64 %6847
  store i8 %6850, ptr %6851, align 1, !tbaa !10
  %6852 = or disjoint i64 %6842, 2
  %6853 = trunc i64 %6852 to i8
  %6854 = mul i8 %6853, 7
  %6855 = add i8 %6854, 3
  %6856 = getelementptr inbounds nuw i8, ptr @src, i64 %6852
  store i8 %6855, ptr %6856, align 2, !tbaa !10
  %6857 = or disjoint i64 %6842, 3
  %6858 = trunc i64 %6857 to i8
  %6859 = mul i8 %6858, 7
  %6860 = add i8 %6859, 3
  %6861 = getelementptr inbounds nuw i8, ptr @src, i64 %6857
  store i8 %6860, ptr %6861, align 1, !tbaa !10
  %6862 = add nuw nsw i64 %6842, 4
  %6863 = icmp eq i64 %6862, 1200
  br i1 %6863, label %6864, label %6841, !llvm.loop !11

6864:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7200()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 200)
  br label %6882

6865:
  %6866 = add nuw nsw i64 %6883, 1
  %6867 = getelementptr inbounds nuw i8, ptr @dst, i64 %6866
  %6868 = load i8, ptr %6867, align 1, !tbaa !10
  %6869 = getelementptr inbounds nuw i8, ptr @ref, i64 %6866
  %6870 = load i8, ptr %6869, align 1, !tbaa !10
  %6871 = icmp eq i8 %6868, %6870
  br i1 %6871, label %6872, label %6889

6872:
  %6873 = add nuw nsw i64 %6883, 2
  %6874 = getelementptr inbounds nuw i8, ptr @dst, i64 %6873
  %6875 = load i8, ptr %6874, align 1, !tbaa !10
  %6876 = getelementptr inbounds nuw i8, ptr @ref, i64 %6873
  %6877 = load i8, ptr %6876, align 1, !tbaa !10
  %6878 = icmp eq i8 %6875, %6877
  br i1 %6878, label %6879, label %6889

6879:
  %6880 = add nuw nsw i64 %6883, 3
  %6881 = icmp eq i64 %6880, 1200
  br i1 %6881, label %6890, label %6882, !llvm.loop !144

6882:
  %6883 = phi i64 [ 0, %6864 ], [ %6880, %6879 ]
  %6884 = getelementptr inbounds nuw i8, ptr @dst, i64 %6883
  %6885 = load i8, ptr %6884, align 1, !tbaa !10
  %6886 = getelementptr inbounds nuw i8, ptr @ref, i64 %6883
  %6887 = load i8, ptr %6886, align 1, !tbaa !10
  %6888 = icmp eq i8 %6885, %6887
  br i1 %6888, label %6865, label %6889

6889:
  tail call void @report(i64 noundef 200)
  store i1 true, ptr @bad, align 4
  br label %6891

6890:
  br label %6891

6891:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6892

6892:
  %6893 = phi i64 [ 0, %6891 ], [ %6913, %6892 ]
  %6894 = trunc i64 %6893 to i8
  %6895 = mul i8 %6894, 7
  %6896 = or disjoint i8 %6895, 3
  %6897 = getelementptr inbounds nuw i8, ptr @src, i64 %6893
  store i8 %6896, ptr %6897, align 4, !tbaa !10
  %6898 = or disjoint i64 %6893, 1
  %6899 = trunc i64 %6898 to i8
  %6900 = mul i8 %6899, 7
  %6901 = add i8 %6900, 3
  %6902 = getelementptr inbounds nuw i8, ptr @src, i64 %6898
  store i8 %6901, ptr %6902, align 1, !tbaa !10
  %6903 = or disjoint i64 %6893, 2
  %6904 = trunc i64 %6903 to i8
  %6905 = mul i8 %6904, 7
  %6906 = add i8 %6905, 3
  %6907 = getelementptr inbounds nuw i8, ptr @src, i64 %6903
  store i8 %6906, ptr %6907, align 2, !tbaa !10
  %6908 = or disjoint i64 %6893, 3
  %6909 = trunc i64 %6908 to i8
  %6910 = mul i8 %6909, 7
  %6911 = add i8 %6910, 3
  %6912 = getelementptr inbounds nuw i8, ptr @src, i64 %6908
  store i8 %6911, ptr %6912, align 1, !tbaa !10
  %6913 = add nuw nsw i64 %6893, 4
  %6914 = icmp eq i64 %6913, 1200
  br i1 %6914, label %6915, label %6892, !llvm.loop !11

6915:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr200()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 200)
  br label %6933

6916:
  %6917 = add nuw nsw i64 %6934, 1
  %6918 = getelementptr inbounds nuw i8, ptr @dst, i64 %6917
  %6919 = load i8, ptr %6918, align 1, !tbaa !10
  %6920 = getelementptr inbounds nuw i8, ptr @ref, i64 %6917
  %6921 = load i8, ptr %6920, align 1, !tbaa !10
  %6922 = icmp eq i8 %6919, %6921
  br i1 %6922, label %6923, label %6940

6923:
  %6924 = add nuw nsw i64 %6934, 2
  %6925 = getelementptr inbounds nuw i8, ptr @dst, i64 %6924
  %6926 = load i8, ptr %6925, align 1, !tbaa !10
  %6927 = getelementptr inbounds nuw i8, ptr @ref, i64 %6924
  %6928 = load i8, ptr %6927, align 1, !tbaa !10
  %6929 = icmp eq i8 %6926, %6928
  br i1 %6929, label %6930, label %6940

6930:
  %6931 = add nuw nsw i64 %6934, 3
  %6932 = icmp eq i64 %6931, 1200
  br i1 %6932, label %6941, label %6933, !llvm.loop !144

6933:
  %6934 = phi i64 [ 0, %6915 ], [ %6931, %6930 ]
  %6935 = getelementptr inbounds nuw i8, ptr @dst, i64 %6934
  %6936 = load i8, ptr %6935, align 1, !tbaa !10
  %6937 = getelementptr inbounds nuw i8, ptr @ref, i64 %6934
  %6938 = load i8, ptr %6937, align 1, !tbaa !10
  %6939 = icmp eq i8 %6936, %6938
  br i1 %6939, label %6916, label %6940

6940:
  tail call void @report(i64 noundef 200)
  store i1 true, ptr @bad, align 4
  br label %6942

6941:
  br label %6942

6942:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6943

6943:
  %6944 = phi i64 [ 0, %6942 ], [ %6964, %6943 ]
  %6945 = trunc i64 %6944 to i8
  %6946 = mul i8 %6945, 7
  %6947 = or disjoint i8 %6946, 3
  %6948 = getelementptr inbounds nuw i8, ptr @src, i64 %6944
  store i8 %6947, ptr %6948, align 4, !tbaa !10
  %6949 = or disjoint i64 %6944, 1
  %6950 = trunc i64 %6949 to i8
  %6951 = mul i8 %6950, 7
  %6952 = add i8 %6951, 3
  %6953 = getelementptr inbounds nuw i8, ptr @src, i64 %6949
  store i8 %6952, ptr %6953, align 1, !tbaa !10
  %6954 = or disjoint i64 %6944, 2
  %6955 = trunc i64 %6954 to i8
  %6956 = mul i8 %6955, 7
  %6957 = add i8 %6956, 3
  %6958 = getelementptr inbounds nuw i8, ptr @src, i64 %6954
  store i8 %6957, ptr %6958, align 2, !tbaa !10
  %6959 = or disjoint i64 %6944, 3
  %6960 = trunc i64 %6959 to i8
  %6961 = mul i8 %6960, 7
  %6962 = add i8 %6961, 3
  %6963 = getelementptr inbounds nuw i8, ptr @src, i64 %6959
  store i8 %6962, ptr %6963, align 1, !tbaa !10
  %6964 = add nuw nsw i64 %6944, 4
  %6965 = icmp eq i64 %6964, 1200
  br i1 %6965, label %6966, label %6943, !llvm.loop !11

6966:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv200(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 200)
  br label %6984

6967:
  %6968 = add nuw nsw i64 %6985, 1
  %6969 = getelementptr inbounds nuw i8, ptr @dst, i64 %6968
  %6970 = load i8, ptr %6969, align 1, !tbaa !10
  %6971 = getelementptr inbounds nuw i8, ptr @ref, i64 %6968
  %6972 = load i8, ptr %6971, align 1, !tbaa !10
  %6973 = icmp eq i8 %6970, %6972
  br i1 %6973, label %6974, label %6991

6974:
  %6975 = add nuw nsw i64 %6985, 2
  %6976 = getelementptr inbounds nuw i8, ptr @dst, i64 %6975
  %6977 = load i8, ptr %6976, align 1, !tbaa !10
  %6978 = getelementptr inbounds nuw i8, ptr @ref, i64 %6975
  %6979 = load i8, ptr %6978, align 1, !tbaa !10
  %6980 = icmp eq i8 %6977, %6979
  br i1 %6980, label %6981, label %6991

6981:
  %6982 = add nuw nsw i64 %6985, 3
  %6983 = icmp eq i64 %6982, 1200
  br i1 %6983, label %6992, label %6984, !llvm.loop !144

6984:
  %6985 = phi i64 [ 0, %6966 ], [ %6982, %6981 ]
  %6986 = getelementptr inbounds nuw i8, ptr @dst, i64 %6985
  %6987 = load i8, ptr %6986, align 1, !tbaa !10
  %6988 = getelementptr inbounds nuw i8, ptr @ref, i64 %6985
  %6989 = load i8, ptr %6988, align 1, !tbaa !10
  %6990 = icmp eq i8 %6987, %6989
  br i1 %6990, label %6967, label %6991

6991:
  tail call void @report(i64 noundef 200)
  store i1 true, ptr @bad, align 4
  br label %6993

6992:
  br label %6993

6993:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %6994

6994:
  %6995 = phi i64 [ 0, %6993 ], [ %7015, %6994 ]
  %6996 = trunc i64 %6995 to i8
  %6997 = mul i8 %6996, 7
  %6998 = or disjoint i8 %6997, 3
  %6999 = getelementptr inbounds nuw i8, ptr @src, i64 %6995
  store i8 %6998, ptr %6999, align 4, !tbaa !10
  %7000 = or disjoint i64 %6995, 1
  %7001 = trunc i64 %7000 to i8
  %7002 = mul i8 %7001, 7
  %7003 = add i8 %7002, 3
  %7004 = getelementptr inbounds nuw i8, ptr @src, i64 %7000
  store i8 %7003, ptr %7004, align 1, !tbaa !10
  %7005 = or disjoint i64 %6995, 2
  %7006 = trunc i64 %7005 to i8
  %7007 = mul i8 %7006, 7
  %7008 = add i8 %7007, 3
  %7009 = getelementptr inbounds nuw i8, ptr @src, i64 %7005
  store i8 %7008, ptr %7009, align 2, !tbaa !10
  %7010 = or disjoint i64 %6995, 3
  %7011 = trunc i64 %7010 to i8
  %7012 = mul i8 %7011, 7
  %7013 = add i8 %7012, 3
  %7014 = getelementptr inbounds nuw i8, ptr @src, i64 %7010
  store i8 %7013, ptr %7014, align 1, !tbaa !10
  %7015 = add nuw nsw i64 %6995, 4
  %7016 = icmp eq i64 %7015, 1200
  br i1 %7016, label %7017, label %6994, !llvm.loop !11

7017:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv200(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 200)
  br label %7035

7018:
  %7019 = add nuw nsw i64 %7036, 1
  %7020 = getelementptr inbounds nuw i8, ptr @dst, i64 %7019
  %7021 = load i8, ptr %7020, align 1, !tbaa !10
  %7022 = getelementptr inbounds nuw i8, ptr @ref, i64 %7019
  %7023 = load i8, ptr %7022, align 1, !tbaa !10
  %7024 = icmp eq i8 %7021, %7023
  br i1 %7024, label %7025, label %7042

7025:
  %7026 = add nuw nsw i64 %7036, 2
  %7027 = getelementptr inbounds nuw i8, ptr @dst, i64 %7026
  %7028 = load i8, ptr %7027, align 1, !tbaa !10
  %7029 = getelementptr inbounds nuw i8, ptr @ref, i64 %7026
  %7030 = load i8, ptr %7029, align 1, !tbaa !10
  %7031 = icmp eq i8 %7028, %7030
  br i1 %7031, label %7032, label %7042

7032:
  %7033 = add nuw nsw i64 %7036, 3
  %7034 = icmp eq i64 %7033, 1200
  br i1 %7034, label %7043, label %7035, !llvm.loop !144

7035:
  %7036 = phi i64 [ 0, %7017 ], [ %7033, %7032 ]
  %7037 = getelementptr inbounds nuw i8, ptr @dst, i64 %7036
  %7038 = load i8, ptr %7037, align 1, !tbaa !10
  %7039 = getelementptr inbounds nuw i8, ptr @ref, i64 %7036
  %7040 = load i8, ptr %7039, align 1, !tbaa !10
  %7041 = icmp eq i8 %7038, %7040
  br i1 %7041, label %7018, label %7042

7042:
  tail call void @report(i64 noundef 200)
  store i1 true, ptr @bad, align 4
  br label %7044

7043:
  br label %7044

7044:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7045

7045:
  %7046 = phi i64 [ 0, %7044 ], [ %7066, %7045 ]
  %7047 = trunc i64 %7046 to i8
  %7048 = mul i8 %7047, 7
  %7049 = or disjoint i8 %7048, 3
  %7050 = getelementptr inbounds nuw i8, ptr @src, i64 %7046
  store i8 %7049, ptr %7050, align 4, !tbaa !10
  %7051 = or disjoint i64 %7046, 1
  %7052 = trunc i64 %7051 to i8
  %7053 = mul i8 %7052, 7
  %7054 = add i8 %7053, 3
  %7055 = getelementptr inbounds nuw i8, ptr @src, i64 %7051
  store i8 %7054, ptr %7055, align 1, !tbaa !10
  %7056 = or disjoint i64 %7046, 2
  %7057 = trunc i64 %7056 to i8
  %7058 = mul i8 %7057, 7
  %7059 = add i8 %7058, 3
  %7060 = getelementptr inbounds nuw i8, ptr @src, i64 %7056
  store i8 %7059, ptr %7060, align 2, !tbaa !10
  %7061 = or disjoint i64 %7046, 3
  %7062 = trunc i64 %7061 to i8
  %7063 = mul i8 %7062, 7
  %7064 = add i8 %7063, 3
  %7065 = getelementptr inbounds nuw i8, ptr @src, i64 %7061
  store i8 %7064, ptr %7065, align 1, !tbaa !10
  %7066 = add nuw nsw i64 %7046, 4
  %7067 = icmp eq i64 %7066, 1200
  br i1 %7067, label %7068, label %7045, !llvm.loop !11

7068:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp255()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 255)
  br label %7086

7069:
  %7070 = add nuw nsw i64 %7087, 1
  %7071 = getelementptr inbounds nuw i8, ptr @dst, i64 %7070
  %7072 = load i8, ptr %7071, align 1, !tbaa !10
  %7073 = getelementptr inbounds nuw i8, ptr @ref, i64 %7070
  %7074 = load i8, ptr %7073, align 1, !tbaa !10
  %7075 = icmp eq i8 %7072, %7074
  br i1 %7075, label %7076, label %7093

7076:
  %7077 = add nuw nsw i64 %7087, 2
  %7078 = getelementptr inbounds nuw i8, ptr @dst, i64 %7077
  %7079 = load i8, ptr %7078, align 1, !tbaa !10
  %7080 = getelementptr inbounds nuw i8, ptr @ref, i64 %7077
  %7081 = load i8, ptr %7080, align 1, !tbaa !10
  %7082 = icmp eq i8 %7079, %7081
  br i1 %7082, label %7083, label %7093

7083:
  %7084 = add nuw nsw i64 %7087, 3
  %7085 = icmp eq i64 %7084, 1200
  br i1 %7085, label %7094, label %7086, !llvm.loop !144

7086:
  %7087 = phi i64 [ 0, %7068 ], [ %7084, %7083 ]
  %7088 = getelementptr inbounds nuw i8, ptr @dst, i64 %7087
  %7089 = load i8, ptr %7088, align 1, !tbaa !10
  %7090 = getelementptr inbounds nuw i8, ptr @ref, i64 %7087
  %7091 = load i8, ptr %7090, align 1, !tbaa !10
  %7092 = icmp eq i8 %7089, %7091
  br i1 %7092, label %7069, label %7093

7093:
  tail call void @report(i64 noundef 255)
  store i1 true, ptr @bad, align 4
  br label %7095

7094:
  br label %7095

7095:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7096

7096:
  %7097 = phi i64 [ 0, %7095 ], [ %7117, %7096 ]
  %7098 = trunc i64 %7097 to i8
  %7099 = mul i8 %7098, 7
  %7100 = or disjoint i8 %7099, 3
  %7101 = getelementptr inbounds nuw i8, ptr @src, i64 %7097
  store i8 %7100, ptr %7101, align 4, !tbaa !10
  %7102 = or disjoint i64 %7097, 1
  %7103 = trunc i64 %7102 to i8
  %7104 = mul i8 %7103, 7
  %7105 = add i8 %7104, 3
  %7106 = getelementptr inbounds nuw i8, ptr @src, i64 %7102
  store i8 %7105, ptr %7106, align 1, !tbaa !10
  %7107 = or disjoint i64 %7097, 2
  %7108 = trunc i64 %7107 to i8
  %7109 = mul i8 %7108, 7
  %7110 = add i8 %7109, 3
  %7111 = getelementptr inbounds nuw i8, ptr @src, i64 %7107
  store i8 %7110, ptr %7111, align 2, !tbaa !10
  %7112 = or disjoint i64 %7097, 3
  %7113 = trunc i64 %7112 to i8
  %7114 = mul i8 %7113, 7
  %7115 = add i8 %7114, 3
  %7116 = getelementptr inbounds nuw i8, ptr @src, i64 %7112
  store i8 %7115, ptr %7116, align 1, !tbaa !10
  %7117 = add nuw nsw i64 %7097, 4
  %7118 = icmp eq i64 %7117, 1200
  br i1 %7118, label %7119, label %7096, !llvm.loop !11

7119:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr255()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 255)
  br label %7137

7120:
  %7121 = add nuw nsw i64 %7138, 1
  %7122 = getelementptr inbounds nuw i8, ptr @dst, i64 %7121
  %7123 = load i8, ptr %7122, align 1, !tbaa !10
  %7124 = getelementptr inbounds nuw i8, ptr @ref, i64 %7121
  %7125 = load i8, ptr %7124, align 1, !tbaa !10
  %7126 = icmp eq i8 %7123, %7125
  br i1 %7126, label %7127, label %7144

7127:
  %7128 = add nuw nsw i64 %7138, 2
  %7129 = getelementptr inbounds nuw i8, ptr @dst, i64 %7128
  %7130 = load i8, ptr %7129, align 1, !tbaa !10
  %7131 = getelementptr inbounds nuw i8, ptr @ref, i64 %7128
  %7132 = load i8, ptr %7131, align 1, !tbaa !10
  %7133 = icmp eq i8 %7130, %7132
  br i1 %7133, label %7134, label %7144

7134:
  %7135 = add nuw nsw i64 %7138, 3
  %7136 = icmp eq i64 %7135, 1200
  br i1 %7136, label %7145, label %7137, !llvm.loop !144

7137:
  %7138 = phi i64 [ 0, %7119 ], [ %7135, %7134 ]
  %7139 = getelementptr inbounds nuw i8, ptr @dst, i64 %7138
  %7140 = load i8, ptr %7139, align 1, !tbaa !10
  %7141 = getelementptr inbounds nuw i8, ptr @ref, i64 %7138
  %7142 = load i8, ptr %7141, align 1, !tbaa !10
  %7143 = icmp eq i8 %7140, %7142
  br i1 %7143, label %7120, label %7144

7144:
  tail call void @report(i64 noundef 255)
  store i1 true, ptr @bad, align 4
  br label %7146

7145:
  br label %7146

7146:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7147

7147:
  %7148 = phi i64 [ 0, %7146 ], [ %7168, %7147 ]
  %7149 = trunc i64 %7148 to i8
  %7150 = mul i8 %7149, 7
  %7151 = or disjoint i8 %7150, 3
  %7152 = getelementptr inbounds nuw i8, ptr @src, i64 %7148
  store i8 %7151, ptr %7152, align 4, !tbaa !10
  %7153 = or disjoint i64 %7148, 1
  %7154 = trunc i64 %7153 to i8
  %7155 = mul i8 %7154, 7
  %7156 = add i8 %7155, 3
  %7157 = getelementptr inbounds nuw i8, ptr @src, i64 %7153
  store i8 %7156, ptr %7157, align 1, !tbaa !10
  %7158 = or disjoint i64 %7148, 2
  %7159 = trunc i64 %7158 to i8
  %7160 = mul i8 %7159, 7
  %7161 = add i8 %7160, 3
  %7162 = getelementptr inbounds nuw i8, ptr @src, i64 %7158
  store i8 %7161, ptr %7162, align 2, !tbaa !10
  %7163 = or disjoint i64 %7148, 3
  %7164 = trunc i64 %7163 to i8
  %7165 = mul i8 %7164, 7
  %7166 = add i8 %7165, 3
  %7167 = getelementptr inbounds nuw i8, ptr @src, i64 %7163
  store i8 %7166, ptr %7167, align 1, !tbaa !10
  %7168 = add nuw nsw i64 %7148, 4
  %7169 = icmp eq i64 %7168, 1200
  br i1 %7169, label %7170, label %7147, !llvm.loop !11

7170:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7255()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 255)
  br label %7188

7171:
  %7172 = add nuw nsw i64 %7189, 1
  %7173 = getelementptr inbounds nuw i8, ptr @dst, i64 %7172
  %7174 = load i8, ptr %7173, align 1, !tbaa !10
  %7175 = getelementptr inbounds nuw i8, ptr @ref, i64 %7172
  %7176 = load i8, ptr %7175, align 1, !tbaa !10
  %7177 = icmp eq i8 %7174, %7176
  br i1 %7177, label %7178, label %7195

7178:
  %7179 = add nuw nsw i64 %7189, 2
  %7180 = getelementptr inbounds nuw i8, ptr @dst, i64 %7179
  %7181 = load i8, ptr %7180, align 1, !tbaa !10
  %7182 = getelementptr inbounds nuw i8, ptr @ref, i64 %7179
  %7183 = load i8, ptr %7182, align 1, !tbaa !10
  %7184 = icmp eq i8 %7181, %7183
  br i1 %7184, label %7185, label %7195

7185:
  %7186 = add nuw nsw i64 %7189, 3
  %7187 = icmp eq i64 %7186, 1200
  br i1 %7187, label %7196, label %7188, !llvm.loop !144

7188:
  %7189 = phi i64 [ 0, %7170 ], [ %7186, %7185 ]
  %7190 = getelementptr inbounds nuw i8, ptr @dst, i64 %7189
  %7191 = load i8, ptr %7190, align 1, !tbaa !10
  %7192 = getelementptr inbounds nuw i8, ptr @ref, i64 %7189
  %7193 = load i8, ptr %7192, align 1, !tbaa !10
  %7194 = icmp eq i8 %7191, %7193
  br i1 %7194, label %7171, label %7195

7195:
  tail call void @report(i64 noundef 255)
  store i1 true, ptr @bad, align 4
  br label %7197

7196:
  br label %7197

7197:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7198

7198:
  %7199 = phi i64 [ 0, %7197 ], [ %7219, %7198 ]
  %7200 = trunc i64 %7199 to i8
  %7201 = mul i8 %7200, 7
  %7202 = or disjoint i8 %7201, 3
  %7203 = getelementptr inbounds nuw i8, ptr @src, i64 %7199
  store i8 %7202, ptr %7203, align 4, !tbaa !10
  %7204 = or disjoint i64 %7199, 1
  %7205 = trunc i64 %7204 to i8
  %7206 = mul i8 %7205, 7
  %7207 = add i8 %7206, 3
  %7208 = getelementptr inbounds nuw i8, ptr @src, i64 %7204
  store i8 %7207, ptr %7208, align 1, !tbaa !10
  %7209 = or disjoint i64 %7199, 2
  %7210 = trunc i64 %7209 to i8
  %7211 = mul i8 %7210, 7
  %7212 = add i8 %7211, 3
  %7213 = getelementptr inbounds nuw i8, ptr @src, i64 %7209
  store i8 %7212, ptr %7213, align 2, !tbaa !10
  %7214 = or disjoint i64 %7199, 3
  %7215 = trunc i64 %7214 to i8
  %7216 = mul i8 %7215, 7
  %7217 = add i8 %7216, 3
  %7218 = getelementptr inbounds nuw i8, ptr @src, i64 %7214
  store i8 %7217, ptr %7218, align 1, !tbaa !10
  %7219 = add nuw nsw i64 %7199, 4
  %7220 = icmp eq i64 %7219, 1200
  br i1 %7220, label %7221, label %7198, !llvm.loop !11

7221:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr255()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 255)
  br label %7239

7222:
  %7223 = add nuw nsw i64 %7240, 1
  %7224 = getelementptr inbounds nuw i8, ptr @dst, i64 %7223
  %7225 = load i8, ptr %7224, align 1, !tbaa !10
  %7226 = getelementptr inbounds nuw i8, ptr @ref, i64 %7223
  %7227 = load i8, ptr %7226, align 1, !tbaa !10
  %7228 = icmp eq i8 %7225, %7227
  br i1 %7228, label %7229, label %7246

7229:
  %7230 = add nuw nsw i64 %7240, 2
  %7231 = getelementptr inbounds nuw i8, ptr @dst, i64 %7230
  %7232 = load i8, ptr %7231, align 1, !tbaa !10
  %7233 = getelementptr inbounds nuw i8, ptr @ref, i64 %7230
  %7234 = load i8, ptr %7233, align 1, !tbaa !10
  %7235 = icmp eq i8 %7232, %7234
  br i1 %7235, label %7236, label %7246

7236:
  %7237 = add nuw nsw i64 %7240, 3
  %7238 = icmp eq i64 %7237, 1200
  br i1 %7238, label %7247, label %7239, !llvm.loop !144

7239:
  %7240 = phi i64 [ 0, %7221 ], [ %7237, %7236 ]
  %7241 = getelementptr inbounds nuw i8, ptr @dst, i64 %7240
  %7242 = load i8, ptr %7241, align 1, !tbaa !10
  %7243 = getelementptr inbounds nuw i8, ptr @ref, i64 %7240
  %7244 = load i8, ptr %7243, align 1, !tbaa !10
  %7245 = icmp eq i8 %7242, %7244
  br i1 %7245, label %7222, label %7246

7246:
  tail call void @report(i64 noundef 255)
  store i1 true, ptr @bad, align 4
  br label %7248

7247:
  br label %7248

7248:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7249

7249:
  %7250 = phi i64 [ 0, %7248 ], [ %7270, %7249 ]
  %7251 = trunc i64 %7250 to i8
  %7252 = mul i8 %7251, 7
  %7253 = or disjoint i8 %7252, 3
  %7254 = getelementptr inbounds nuw i8, ptr @src, i64 %7250
  store i8 %7253, ptr %7254, align 4, !tbaa !10
  %7255 = or disjoint i64 %7250, 1
  %7256 = trunc i64 %7255 to i8
  %7257 = mul i8 %7256, 7
  %7258 = add i8 %7257, 3
  %7259 = getelementptr inbounds nuw i8, ptr @src, i64 %7255
  store i8 %7258, ptr %7259, align 1, !tbaa !10
  %7260 = or disjoint i64 %7250, 2
  %7261 = trunc i64 %7260 to i8
  %7262 = mul i8 %7261, 7
  %7263 = add i8 %7262, 3
  %7264 = getelementptr inbounds nuw i8, ptr @src, i64 %7260
  store i8 %7263, ptr %7264, align 2, !tbaa !10
  %7265 = or disjoint i64 %7250, 3
  %7266 = trunc i64 %7265 to i8
  %7267 = mul i8 %7266, 7
  %7268 = add i8 %7267, 3
  %7269 = getelementptr inbounds nuw i8, ptr @src, i64 %7265
  store i8 %7268, ptr %7269, align 1, !tbaa !10
  %7270 = add nuw nsw i64 %7250, 4
  %7271 = icmp eq i64 %7270, 1200
  br i1 %7271, label %7272, label %7249, !llvm.loop !11

7272:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv255(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 255)
  br label %7290

7273:
  %7274 = add nuw nsw i64 %7291, 1
  %7275 = getelementptr inbounds nuw i8, ptr @dst, i64 %7274
  %7276 = load i8, ptr %7275, align 1, !tbaa !10
  %7277 = getelementptr inbounds nuw i8, ptr @ref, i64 %7274
  %7278 = load i8, ptr %7277, align 1, !tbaa !10
  %7279 = icmp eq i8 %7276, %7278
  br i1 %7279, label %7280, label %7297

7280:
  %7281 = add nuw nsw i64 %7291, 2
  %7282 = getelementptr inbounds nuw i8, ptr @dst, i64 %7281
  %7283 = load i8, ptr %7282, align 1, !tbaa !10
  %7284 = getelementptr inbounds nuw i8, ptr @ref, i64 %7281
  %7285 = load i8, ptr %7284, align 1, !tbaa !10
  %7286 = icmp eq i8 %7283, %7285
  br i1 %7286, label %7287, label %7297

7287:
  %7288 = add nuw nsw i64 %7291, 3
  %7289 = icmp eq i64 %7288, 1200
  br i1 %7289, label %7298, label %7290, !llvm.loop !144

7290:
  %7291 = phi i64 [ 0, %7272 ], [ %7288, %7287 ]
  %7292 = getelementptr inbounds nuw i8, ptr @dst, i64 %7291
  %7293 = load i8, ptr %7292, align 1, !tbaa !10
  %7294 = getelementptr inbounds nuw i8, ptr @ref, i64 %7291
  %7295 = load i8, ptr %7294, align 1, !tbaa !10
  %7296 = icmp eq i8 %7293, %7295
  br i1 %7296, label %7273, label %7297

7297:
  tail call void @report(i64 noundef 255)
  store i1 true, ptr @bad, align 4
  br label %7299

7298:
  br label %7299

7299:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7300

7300:
  %7301 = phi i64 [ 0, %7299 ], [ %7321, %7300 ]
  %7302 = trunc i64 %7301 to i8
  %7303 = mul i8 %7302, 7
  %7304 = or disjoint i8 %7303, 3
  %7305 = getelementptr inbounds nuw i8, ptr @src, i64 %7301
  store i8 %7304, ptr %7305, align 4, !tbaa !10
  %7306 = or disjoint i64 %7301, 1
  %7307 = trunc i64 %7306 to i8
  %7308 = mul i8 %7307, 7
  %7309 = add i8 %7308, 3
  %7310 = getelementptr inbounds nuw i8, ptr @src, i64 %7306
  store i8 %7309, ptr %7310, align 1, !tbaa !10
  %7311 = or disjoint i64 %7301, 2
  %7312 = trunc i64 %7311 to i8
  %7313 = mul i8 %7312, 7
  %7314 = add i8 %7313, 3
  %7315 = getelementptr inbounds nuw i8, ptr @src, i64 %7311
  store i8 %7314, ptr %7315, align 2, !tbaa !10
  %7316 = or disjoint i64 %7301, 3
  %7317 = trunc i64 %7316 to i8
  %7318 = mul i8 %7317, 7
  %7319 = add i8 %7318, 3
  %7320 = getelementptr inbounds nuw i8, ptr @src, i64 %7316
  store i8 %7319, ptr %7320, align 1, !tbaa !10
  %7321 = add nuw nsw i64 %7301, 4
  %7322 = icmp eq i64 %7321, 1200
  br i1 %7322, label %7323, label %7300, !llvm.loop !11

7323:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv255(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 255)
  br label %7341

7324:
  %7325 = add nuw nsw i64 %7342, 1
  %7326 = getelementptr inbounds nuw i8, ptr @dst, i64 %7325
  %7327 = load i8, ptr %7326, align 1, !tbaa !10
  %7328 = getelementptr inbounds nuw i8, ptr @ref, i64 %7325
  %7329 = load i8, ptr %7328, align 1, !tbaa !10
  %7330 = icmp eq i8 %7327, %7329
  br i1 %7330, label %7331, label %7348

7331:
  %7332 = add nuw nsw i64 %7342, 2
  %7333 = getelementptr inbounds nuw i8, ptr @dst, i64 %7332
  %7334 = load i8, ptr %7333, align 1, !tbaa !10
  %7335 = getelementptr inbounds nuw i8, ptr @ref, i64 %7332
  %7336 = load i8, ptr %7335, align 1, !tbaa !10
  %7337 = icmp eq i8 %7334, %7336
  br i1 %7337, label %7338, label %7348

7338:
  %7339 = add nuw nsw i64 %7342, 3
  %7340 = icmp eq i64 %7339, 1200
  br i1 %7340, label %7349, label %7341, !llvm.loop !144

7341:
  %7342 = phi i64 [ 0, %7323 ], [ %7339, %7338 ]
  %7343 = getelementptr inbounds nuw i8, ptr @dst, i64 %7342
  %7344 = load i8, ptr %7343, align 1, !tbaa !10
  %7345 = getelementptr inbounds nuw i8, ptr @ref, i64 %7342
  %7346 = load i8, ptr %7345, align 1, !tbaa !10
  %7347 = icmp eq i8 %7344, %7346
  br i1 %7347, label %7324, label %7348

7348:
  tail call void @report(i64 noundef 255)
  store i1 true, ptr @bad, align 4
  br label %7350

7349:
  br label %7350

7350:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7351

7351:
  %7352 = phi i64 [ 0, %7350 ], [ %7372, %7351 ]
  %7353 = trunc i64 %7352 to i8
  %7354 = mul i8 %7353, 7
  %7355 = or disjoint i8 %7354, 3
  %7356 = getelementptr inbounds nuw i8, ptr @src, i64 %7352
  store i8 %7355, ptr %7356, align 4, !tbaa !10
  %7357 = or disjoint i64 %7352, 1
  %7358 = trunc i64 %7357 to i8
  %7359 = mul i8 %7358, 7
  %7360 = add i8 %7359, 3
  %7361 = getelementptr inbounds nuw i8, ptr @src, i64 %7357
  store i8 %7360, ptr %7361, align 1, !tbaa !10
  %7362 = or disjoint i64 %7352, 2
  %7363 = trunc i64 %7362 to i8
  %7364 = mul i8 %7363, 7
  %7365 = add i8 %7364, 3
  %7366 = getelementptr inbounds nuw i8, ptr @src, i64 %7362
  store i8 %7365, ptr %7366, align 2, !tbaa !10
  %7367 = or disjoint i64 %7352, 3
  %7368 = trunc i64 %7367 to i8
  %7369 = mul i8 %7368, 7
  %7370 = add i8 %7369, 3
  %7371 = getelementptr inbounds nuw i8, ptr @src, i64 %7367
  store i8 %7370, ptr %7371, align 1, !tbaa !10
  %7372 = add nuw nsw i64 %7352, 4
  %7373 = icmp eq i64 %7372, 1200
  br i1 %7373, label %7374, label %7351, !llvm.loop !11

7374:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp256()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 256)
  br label %7392

7375:
  %7376 = add nuw nsw i64 %7393, 1
  %7377 = getelementptr inbounds nuw i8, ptr @dst, i64 %7376
  %7378 = load i8, ptr %7377, align 1, !tbaa !10
  %7379 = getelementptr inbounds nuw i8, ptr @ref, i64 %7376
  %7380 = load i8, ptr %7379, align 1, !tbaa !10
  %7381 = icmp eq i8 %7378, %7380
  br i1 %7381, label %7382, label %7399

7382:
  %7383 = add nuw nsw i64 %7393, 2
  %7384 = getelementptr inbounds nuw i8, ptr @dst, i64 %7383
  %7385 = load i8, ptr %7384, align 1, !tbaa !10
  %7386 = getelementptr inbounds nuw i8, ptr @ref, i64 %7383
  %7387 = load i8, ptr %7386, align 1, !tbaa !10
  %7388 = icmp eq i8 %7385, %7387
  br i1 %7388, label %7389, label %7399

7389:
  %7390 = add nuw nsw i64 %7393, 3
  %7391 = icmp eq i64 %7390, 1200
  br i1 %7391, label %7400, label %7392, !llvm.loop !144

7392:
  %7393 = phi i64 [ 0, %7374 ], [ %7390, %7389 ]
  %7394 = getelementptr inbounds nuw i8, ptr @dst, i64 %7393
  %7395 = load i8, ptr %7394, align 1, !tbaa !10
  %7396 = getelementptr inbounds nuw i8, ptr @ref, i64 %7393
  %7397 = load i8, ptr %7396, align 1, !tbaa !10
  %7398 = icmp eq i8 %7395, %7397
  br i1 %7398, label %7375, label %7399

7399:
  tail call void @report(i64 noundef 256)
  store i1 true, ptr @bad, align 4
  br label %7401

7400:
  br label %7401

7401:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7402

7402:
  %7403 = phi i64 [ 0, %7401 ], [ %7423, %7402 ]
  %7404 = trunc i64 %7403 to i8
  %7405 = mul i8 %7404, 7
  %7406 = or disjoint i8 %7405, 3
  %7407 = getelementptr inbounds nuw i8, ptr @src, i64 %7403
  store i8 %7406, ptr %7407, align 4, !tbaa !10
  %7408 = or disjoint i64 %7403, 1
  %7409 = trunc i64 %7408 to i8
  %7410 = mul i8 %7409, 7
  %7411 = add i8 %7410, 3
  %7412 = getelementptr inbounds nuw i8, ptr @src, i64 %7408
  store i8 %7411, ptr %7412, align 1, !tbaa !10
  %7413 = or disjoint i64 %7403, 2
  %7414 = trunc i64 %7413 to i8
  %7415 = mul i8 %7414, 7
  %7416 = add i8 %7415, 3
  %7417 = getelementptr inbounds nuw i8, ptr @src, i64 %7413
  store i8 %7416, ptr %7417, align 2, !tbaa !10
  %7418 = or disjoint i64 %7403, 3
  %7419 = trunc i64 %7418 to i8
  %7420 = mul i8 %7419, 7
  %7421 = add i8 %7420, 3
  %7422 = getelementptr inbounds nuw i8, ptr @src, i64 %7418
  store i8 %7421, ptr %7422, align 1, !tbaa !10
  %7423 = add nuw nsw i64 %7403, 4
  %7424 = icmp eq i64 %7423, 1200
  br i1 %7424, label %7425, label %7402, !llvm.loop !11

7425:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr256()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 256)
  br label %7443

7426:
  %7427 = add nuw nsw i64 %7444, 1
  %7428 = getelementptr inbounds nuw i8, ptr @dst, i64 %7427
  %7429 = load i8, ptr %7428, align 1, !tbaa !10
  %7430 = getelementptr inbounds nuw i8, ptr @ref, i64 %7427
  %7431 = load i8, ptr %7430, align 1, !tbaa !10
  %7432 = icmp eq i8 %7429, %7431
  br i1 %7432, label %7433, label %7450

7433:
  %7434 = add nuw nsw i64 %7444, 2
  %7435 = getelementptr inbounds nuw i8, ptr @dst, i64 %7434
  %7436 = load i8, ptr %7435, align 1, !tbaa !10
  %7437 = getelementptr inbounds nuw i8, ptr @ref, i64 %7434
  %7438 = load i8, ptr %7437, align 1, !tbaa !10
  %7439 = icmp eq i8 %7436, %7438
  br i1 %7439, label %7440, label %7450

7440:
  %7441 = add nuw nsw i64 %7444, 3
  %7442 = icmp eq i64 %7441, 1200
  br i1 %7442, label %7451, label %7443, !llvm.loop !144

7443:
  %7444 = phi i64 [ 0, %7425 ], [ %7441, %7440 ]
  %7445 = getelementptr inbounds nuw i8, ptr @dst, i64 %7444
  %7446 = load i8, ptr %7445, align 1, !tbaa !10
  %7447 = getelementptr inbounds nuw i8, ptr @ref, i64 %7444
  %7448 = load i8, ptr %7447, align 1, !tbaa !10
  %7449 = icmp eq i8 %7446, %7448
  br i1 %7449, label %7426, label %7450

7450:
  tail call void @report(i64 noundef 256)
  store i1 true, ptr @bad, align 4
  br label %7452

7451:
  br label %7452

7452:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7453

7453:
  %7454 = phi i64 [ 0, %7452 ], [ %7474, %7453 ]
  %7455 = trunc i64 %7454 to i8
  %7456 = mul i8 %7455, 7
  %7457 = or disjoint i8 %7456, 3
  %7458 = getelementptr inbounds nuw i8, ptr @src, i64 %7454
  store i8 %7457, ptr %7458, align 4, !tbaa !10
  %7459 = or disjoint i64 %7454, 1
  %7460 = trunc i64 %7459 to i8
  %7461 = mul i8 %7460, 7
  %7462 = add i8 %7461, 3
  %7463 = getelementptr inbounds nuw i8, ptr @src, i64 %7459
  store i8 %7462, ptr %7463, align 1, !tbaa !10
  %7464 = or disjoint i64 %7454, 2
  %7465 = trunc i64 %7464 to i8
  %7466 = mul i8 %7465, 7
  %7467 = add i8 %7466, 3
  %7468 = getelementptr inbounds nuw i8, ptr @src, i64 %7464
  store i8 %7467, ptr %7468, align 2, !tbaa !10
  %7469 = or disjoint i64 %7454, 3
  %7470 = trunc i64 %7469 to i8
  %7471 = mul i8 %7470, 7
  %7472 = add i8 %7471, 3
  %7473 = getelementptr inbounds nuw i8, ptr @src, i64 %7469
  store i8 %7472, ptr %7473, align 1, !tbaa !10
  %7474 = add nuw nsw i64 %7454, 4
  %7475 = icmp eq i64 %7474, 1200
  br i1 %7475, label %7476, label %7453, !llvm.loop !11

7476:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7256()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 256)
  br label %7494

7477:
  %7478 = add nuw nsw i64 %7495, 1
  %7479 = getelementptr inbounds nuw i8, ptr @dst, i64 %7478
  %7480 = load i8, ptr %7479, align 1, !tbaa !10
  %7481 = getelementptr inbounds nuw i8, ptr @ref, i64 %7478
  %7482 = load i8, ptr %7481, align 1, !tbaa !10
  %7483 = icmp eq i8 %7480, %7482
  br i1 %7483, label %7484, label %7501

7484:
  %7485 = add nuw nsw i64 %7495, 2
  %7486 = getelementptr inbounds nuw i8, ptr @dst, i64 %7485
  %7487 = load i8, ptr %7486, align 1, !tbaa !10
  %7488 = getelementptr inbounds nuw i8, ptr @ref, i64 %7485
  %7489 = load i8, ptr %7488, align 1, !tbaa !10
  %7490 = icmp eq i8 %7487, %7489
  br i1 %7490, label %7491, label %7501

7491:
  %7492 = add nuw nsw i64 %7495, 3
  %7493 = icmp eq i64 %7492, 1200
  br i1 %7493, label %7502, label %7494, !llvm.loop !144

7494:
  %7495 = phi i64 [ 0, %7476 ], [ %7492, %7491 ]
  %7496 = getelementptr inbounds nuw i8, ptr @dst, i64 %7495
  %7497 = load i8, ptr %7496, align 1, !tbaa !10
  %7498 = getelementptr inbounds nuw i8, ptr @ref, i64 %7495
  %7499 = load i8, ptr %7498, align 1, !tbaa !10
  %7500 = icmp eq i8 %7497, %7499
  br i1 %7500, label %7477, label %7501

7501:
  tail call void @report(i64 noundef 256)
  store i1 true, ptr @bad, align 4
  br label %7503

7502:
  br label %7503

7503:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7504

7504:
  %7505 = phi i64 [ 0, %7503 ], [ %7525, %7504 ]
  %7506 = trunc i64 %7505 to i8
  %7507 = mul i8 %7506, 7
  %7508 = or disjoint i8 %7507, 3
  %7509 = getelementptr inbounds nuw i8, ptr @src, i64 %7505
  store i8 %7508, ptr %7509, align 4, !tbaa !10
  %7510 = or disjoint i64 %7505, 1
  %7511 = trunc i64 %7510 to i8
  %7512 = mul i8 %7511, 7
  %7513 = add i8 %7512, 3
  %7514 = getelementptr inbounds nuw i8, ptr @src, i64 %7510
  store i8 %7513, ptr %7514, align 1, !tbaa !10
  %7515 = or disjoint i64 %7505, 2
  %7516 = trunc i64 %7515 to i8
  %7517 = mul i8 %7516, 7
  %7518 = add i8 %7517, 3
  %7519 = getelementptr inbounds nuw i8, ptr @src, i64 %7515
  store i8 %7518, ptr %7519, align 2, !tbaa !10
  %7520 = or disjoint i64 %7505, 3
  %7521 = trunc i64 %7520 to i8
  %7522 = mul i8 %7521, 7
  %7523 = add i8 %7522, 3
  %7524 = getelementptr inbounds nuw i8, ptr @src, i64 %7520
  store i8 %7523, ptr %7524, align 1, !tbaa !10
  %7525 = add nuw nsw i64 %7505, 4
  %7526 = icmp eq i64 %7525, 1200
  br i1 %7526, label %7527, label %7504, !llvm.loop !11

7527:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr256()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 256)
  br label %7545

7528:
  %7529 = add nuw nsw i64 %7546, 1
  %7530 = getelementptr inbounds nuw i8, ptr @dst, i64 %7529
  %7531 = load i8, ptr %7530, align 1, !tbaa !10
  %7532 = getelementptr inbounds nuw i8, ptr @ref, i64 %7529
  %7533 = load i8, ptr %7532, align 1, !tbaa !10
  %7534 = icmp eq i8 %7531, %7533
  br i1 %7534, label %7535, label %7552

7535:
  %7536 = add nuw nsw i64 %7546, 2
  %7537 = getelementptr inbounds nuw i8, ptr @dst, i64 %7536
  %7538 = load i8, ptr %7537, align 1, !tbaa !10
  %7539 = getelementptr inbounds nuw i8, ptr @ref, i64 %7536
  %7540 = load i8, ptr %7539, align 1, !tbaa !10
  %7541 = icmp eq i8 %7538, %7540
  br i1 %7541, label %7542, label %7552

7542:
  %7543 = add nuw nsw i64 %7546, 3
  %7544 = icmp eq i64 %7543, 1200
  br i1 %7544, label %7553, label %7545, !llvm.loop !144

7545:
  %7546 = phi i64 [ 0, %7527 ], [ %7543, %7542 ]
  %7547 = getelementptr inbounds nuw i8, ptr @dst, i64 %7546
  %7548 = load i8, ptr %7547, align 1, !tbaa !10
  %7549 = getelementptr inbounds nuw i8, ptr @ref, i64 %7546
  %7550 = load i8, ptr %7549, align 1, !tbaa !10
  %7551 = icmp eq i8 %7548, %7550
  br i1 %7551, label %7528, label %7552

7552:
  tail call void @report(i64 noundef 256)
  store i1 true, ptr @bad, align 4
  br label %7554

7553:
  br label %7554

7554:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7555

7555:
  %7556 = phi i64 [ 0, %7554 ], [ %7576, %7555 ]
  %7557 = trunc i64 %7556 to i8
  %7558 = mul i8 %7557, 7
  %7559 = or disjoint i8 %7558, 3
  %7560 = getelementptr inbounds nuw i8, ptr @src, i64 %7556
  store i8 %7559, ptr %7560, align 4, !tbaa !10
  %7561 = or disjoint i64 %7556, 1
  %7562 = trunc i64 %7561 to i8
  %7563 = mul i8 %7562, 7
  %7564 = add i8 %7563, 3
  %7565 = getelementptr inbounds nuw i8, ptr @src, i64 %7561
  store i8 %7564, ptr %7565, align 1, !tbaa !10
  %7566 = or disjoint i64 %7556, 2
  %7567 = trunc i64 %7566 to i8
  %7568 = mul i8 %7567, 7
  %7569 = add i8 %7568, 3
  %7570 = getelementptr inbounds nuw i8, ptr @src, i64 %7566
  store i8 %7569, ptr %7570, align 2, !tbaa !10
  %7571 = or disjoint i64 %7556, 3
  %7572 = trunc i64 %7571 to i8
  %7573 = mul i8 %7572, 7
  %7574 = add i8 %7573, 3
  %7575 = getelementptr inbounds nuw i8, ptr @src, i64 %7571
  store i8 %7574, ptr %7575, align 1, !tbaa !10
  %7576 = add nuw nsw i64 %7556, 4
  %7577 = icmp eq i64 %7576, 1200
  br i1 %7577, label %7578, label %7555, !llvm.loop !11

7578:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv256(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 256)
  br label %7596

7579:
  %7580 = add nuw nsw i64 %7597, 1
  %7581 = getelementptr inbounds nuw i8, ptr @dst, i64 %7580
  %7582 = load i8, ptr %7581, align 1, !tbaa !10
  %7583 = getelementptr inbounds nuw i8, ptr @ref, i64 %7580
  %7584 = load i8, ptr %7583, align 1, !tbaa !10
  %7585 = icmp eq i8 %7582, %7584
  br i1 %7585, label %7586, label %7603

7586:
  %7587 = add nuw nsw i64 %7597, 2
  %7588 = getelementptr inbounds nuw i8, ptr @dst, i64 %7587
  %7589 = load i8, ptr %7588, align 1, !tbaa !10
  %7590 = getelementptr inbounds nuw i8, ptr @ref, i64 %7587
  %7591 = load i8, ptr %7590, align 1, !tbaa !10
  %7592 = icmp eq i8 %7589, %7591
  br i1 %7592, label %7593, label %7603

7593:
  %7594 = add nuw nsw i64 %7597, 3
  %7595 = icmp eq i64 %7594, 1200
  br i1 %7595, label %7604, label %7596, !llvm.loop !144

7596:
  %7597 = phi i64 [ 0, %7578 ], [ %7594, %7593 ]
  %7598 = getelementptr inbounds nuw i8, ptr @dst, i64 %7597
  %7599 = load i8, ptr %7598, align 1, !tbaa !10
  %7600 = getelementptr inbounds nuw i8, ptr @ref, i64 %7597
  %7601 = load i8, ptr %7600, align 1, !tbaa !10
  %7602 = icmp eq i8 %7599, %7601
  br i1 %7602, label %7579, label %7603

7603:
  tail call void @report(i64 noundef 256)
  store i1 true, ptr @bad, align 4
  br label %7605

7604:
  br label %7605

7605:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7606

7606:
  %7607 = phi i64 [ 0, %7605 ], [ %7627, %7606 ]
  %7608 = trunc i64 %7607 to i8
  %7609 = mul i8 %7608, 7
  %7610 = or disjoint i8 %7609, 3
  %7611 = getelementptr inbounds nuw i8, ptr @src, i64 %7607
  store i8 %7610, ptr %7611, align 4, !tbaa !10
  %7612 = or disjoint i64 %7607, 1
  %7613 = trunc i64 %7612 to i8
  %7614 = mul i8 %7613, 7
  %7615 = add i8 %7614, 3
  %7616 = getelementptr inbounds nuw i8, ptr @src, i64 %7612
  store i8 %7615, ptr %7616, align 1, !tbaa !10
  %7617 = or disjoint i64 %7607, 2
  %7618 = trunc i64 %7617 to i8
  %7619 = mul i8 %7618, 7
  %7620 = add i8 %7619, 3
  %7621 = getelementptr inbounds nuw i8, ptr @src, i64 %7617
  store i8 %7620, ptr %7621, align 2, !tbaa !10
  %7622 = or disjoint i64 %7607, 3
  %7623 = trunc i64 %7622 to i8
  %7624 = mul i8 %7623, 7
  %7625 = add i8 %7624, 3
  %7626 = getelementptr inbounds nuw i8, ptr @src, i64 %7622
  store i8 %7625, ptr %7626, align 1, !tbaa !10
  %7627 = add nuw nsw i64 %7607, 4
  %7628 = icmp eq i64 %7627, 1200
  br i1 %7628, label %7629, label %7606, !llvm.loop !11

7629:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv256(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 256)
  br label %7647

7630:
  %7631 = add nuw nsw i64 %7648, 1
  %7632 = getelementptr inbounds nuw i8, ptr @dst, i64 %7631
  %7633 = load i8, ptr %7632, align 1, !tbaa !10
  %7634 = getelementptr inbounds nuw i8, ptr @ref, i64 %7631
  %7635 = load i8, ptr %7634, align 1, !tbaa !10
  %7636 = icmp eq i8 %7633, %7635
  br i1 %7636, label %7637, label %7654

7637:
  %7638 = add nuw nsw i64 %7648, 2
  %7639 = getelementptr inbounds nuw i8, ptr @dst, i64 %7638
  %7640 = load i8, ptr %7639, align 1, !tbaa !10
  %7641 = getelementptr inbounds nuw i8, ptr @ref, i64 %7638
  %7642 = load i8, ptr %7641, align 1, !tbaa !10
  %7643 = icmp eq i8 %7640, %7642
  br i1 %7643, label %7644, label %7654

7644:
  %7645 = add nuw nsw i64 %7648, 3
  %7646 = icmp eq i64 %7645, 1200
  br i1 %7646, label %7655, label %7647, !llvm.loop !144

7647:
  %7648 = phi i64 [ 0, %7629 ], [ %7645, %7644 ]
  %7649 = getelementptr inbounds nuw i8, ptr @dst, i64 %7648
  %7650 = load i8, ptr %7649, align 1, !tbaa !10
  %7651 = getelementptr inbounds nuw i8, ptr @ref, i64 %7648
  %7652 = load i8, ptr %7651, align 1, !tbaa !10
  %7653 = icmp eq i8 %7650, %7652
  br i1 %7653, label %7630, label %7654

7654:
  tail call void @report(i64 noundef 256)
  store i1 true, ptr @bad, align 4
  br label %7656

7655:
  br label %7656

7656:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7657

7657:
  %7658 = phi i64 [ 0, %7656 ], [ %7678, %7657 ]
  %7659 = trunc i64 %7658 to i8
  %7660 = mul i8 %7659, 7
  %7661 = or disjoint i8 %7660, 3
  %7662 = getelementptr inbounds nuw i8, ptr @src, i64 %7658
  store i8 %7661, ptr %7662, align 4, !tbaa !10
  %7663 = or disjoint i64 %7658, 1
  %7664 = trunc i64 %7663 to i8
  %7665 = mul i8 %7664, 7
  %7666 = add i8 %7665, 3
  %7667 = getelementptr inbounds nuw i8, ptr @src, i64 %7663
  store i8 %7666, ptr %7667, align 1, !tbaa !10
  %7668 = or disjoint i64 %7658, 2
  %7669 = trunc i64 %7668 to i8
  %7670 = mul i8 %7669, 7
  %7671 = add i8 %7670, 3
  %7672 = getelementptr inbounds nuw i8, ptr @src, i64 %7668
  store i8 %7671, ptr %7672, align 2, !tbaa !10
  %7673 = or disjoint i64 %7658, 3
  %7674 = trunc i64 %7673 to i8
  %7675 = mul i8 %7674, 7
  %7676 = add i8 %7675, 3
  %7677 = getelementptr inbounds nuw i8, ptr @src, i64 %7673
  store i8 %7676, ptr %7677, align 1, !tbaa !10
  %7678 = add nuw nsw i64 %7658, 4
  %7679 = icmp eq i64 %7678, 1200
  br i1 %7679, label %7680, label %7657, !llvm.loop !11

7680:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp257()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 257)
  br label %7698

7681:
  %7682 = add nuw nsw i64 %7699, 1
  %7683 = getelementptr inbounds nuw i8, ptr @dst, i64 %7682
  %7684 = load i8, ptr %7683, align 1, !tbaa !10
  %7685 = getelementptr inbounds nuw i8, ptr @ref, i64 %7682
  %7686 = load i8, ptr %7685, align 1, !tbaa !10
  %7687 = icmp eq i8 %7684, %7686
  br i1 %7687, label %7688, label %7705

7688:
  %7689 = add nuw nsw i64 %7699, 2
  %7690 = getelementptr inbounds nuw i8, ptr @dst, i64 %7689
  %7691 = load i8, ptr %7690, align 1, !tbaa !10
  %7692 = getelementptr inbounds nuw i8, ptr @ref, i64 %7689
  %7693 = load i8, ptr %7692, align 1, !tbaa !10
  %7694 = icmp eq i8 %7691, %7693
  br i1 %7694, label %7695, label %7705

7695:
  %7696 = add nuw nsw i64 %7699, 3
  %7697 = icmp eq i64 %7696, 1200
  br i1 %7697, label %7706, label %7698, !llvm.loop !144

7698:
  %7699 = phi i64 [ 0, %7680 ], [ %7696, %7695 ]
  %7700 = getelementptr inbounds nuw i8, ptr @dst, i64 %7699
  %7701 = load i8, ptr %7700, align 1, !tbaa !10
  %7702 = getelementptr inbounds nuw i8, ptr @ref, i64 %7699
  %7703 = load i8, ptr %7702, align 1, !tbaa !10
  %7704 = icmp eq i8 %7701, %7703
  br i1 %7704, label %7681, label %7705

7705:
  tail call void @report(i64 noundef 257)
  store i1 true, ptr @bad, align 4
  br label %7707

7706:
  br label %7707

7707:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7708

7708:
  %7709 = phi i64 [ 0, %7707 ], [ %7729, %7708 ]
  %7710 = trunc i64 %7709 to i8
  %7711 = mul i8 %7710, 7
  %7712 = or disjoint i8 %7711, 3
  %7713 = getelementptr inbounds nuw i8, ptr @src, i64 %7709
  store i8 %7712, ptr %7713, align 4, !tbaa !10
  %7714 = or disjoint i64 %7709, 1
  %7715 = trunc i64 %7714 to i8
  %7716 = mul i8 %7715, 7
  %7717 = add i8 %7716, 3
  %7718 = getelementptr inbounds nuw i8, ptr @src, i64 %7714
  store i8 %7717, ptr %7718, align 1, !tbaa !10
  %7719 = or disjoint i64 %7709, 2
  %7720 = trunc i64 %7719 to i8
  %7721 = mul i8 %7720, 7
  %7722 = add i8 %7721, 3
  %7723 = getelementptr inbounds nuw i8, ptr @src, i64 %7719
  store i8 %7722, ptr %7723, align 2, !tbaa !10
  %7724 = or disjoint i64 %7709, 3
  %7725 = trunc i64 %7724 to i8
  %7726 = mul i8 %7725, 7
  %7727 = add i8 %7726, 3
  %7728 = getelementptr inbounds nuw i8, ptr @src, i64 %7724
  store i8 %7727, ptr %7728, align 1, !tbaa !10
  %7729 = add nuw nsw i64 %7709, 4
  %7730 = icmp eq i64 %7729, 1200
  br i1 %7730, label %7731, label %7708, !llvm.loop !11

7731:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr257()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 257)
  br label %7749

7732:
  %7733 = add nuw nsw i64 %7750, 1
  %7734 = getelementptr inbounds nuw i8, ptr @dst, i64 %7733
  %7735 = load i8, ptr %7734, align 1, !tbaa !10
  %7736 = getelementptr inbounds nuw i8, ptr @ref, i64 %7733
  %7737 = load i8, ptr %7736, align 1, !tbaa !10
  %7738 = icmp eq i8 %7735, %7737
  br i1 %7738, label %7739, label %7756

7739:
  %7740 = add nuw nsw i64 %7750, 2
  %7741 = getelementptr inbounds nuw i8, ptr @dst, i64 %7740
  %7742 = load i8, ptr %7741, align 1, !tbaa !10
  %7743 = getelementptr inbounds nuw i8, ptr @ref, i64 %7740
  %7744 = load i8, ptr %7743, align 1, !tbaa !10
  %7745 = icmp eq i8 %7742, %7744
  br i1 %7745, label %7746, label %7756

7746:
  %7747 = add nuw nsw i64 %7750, 3
  %7748 = icmp eq i64 %7747, 1200
  br i1 %7748, label %7757, label %7749, !llvm.loop !144

7749:
  %7750 = phi i64 [ 0, %7731 ], [ %7747, %7746 ]
  %7751 = getelementptr inbounds nuw i8, ptr @dst, i64 %7750
  %7752 = load i8, ptr %7751, align 1, !tbaa !10
  %7753 = getelementptr inbounds nuw i8, ptr @ref, i64 %7750
  %7754 = load i8, ptr %7753, align 1, !tbaa !10
  %7755 = icmp eq i8 %7752, %7754
  br i1 %7755, label %7732, label %7756

7756:
  tail call void @report(i64 noundef 257)
  store i1 true, ptr @bad, align 4
  br label %7758

7757:
  br label %7758

7758:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7759

7759:
  %7760 = phi i64 [ 0, %7758 ], [ %7780, %7759 ]
  %7761 = trunc i64 %7760 to i8
  %7762 = mul i8 %7761, 7
  %7763 = or disjoint i8 %7762, 3
  %7764 = getelementptr inbounds nuw i8, ptr @src, i64 %7760
  store i8 %7763, ptr %7764, align 4, !tbaa !10
  %7765 = or disjoint i64 %7760, 1
  %7766 = trunc i64 %7765 to i8
  %7767 = mul i8 %7766, 7
  %7768 = add i8 %7767, 3
  %7769 = getelementptr inbounds nuw i8, ptr @src, i64 %7765
  store i8 %7768, ptr %7769, align 1, !tbaa !10
  %7770 = or disjoint i64 %7760, 2
  %7771 = trunc i64 %7770 to i8
  %7772 = mul i8 %7771, 7
  %7773 = add i8 %7772, 3
  %7774 = getelementptr inbounds nuw i8, ptr @src, i64 %7770
  store i8 %7773, ptr %7774, align 2, !tbaa !10
  %7775 = or disjoint i64 %7760, 3
  %7776 = trunc i64 %7775 to i8
  %7777 = mul i8 %7776, 7
  %7778 = add i8 %7777, 3
  %7779 = getelementptr inbounds nuw i8, ptr @src, i64 %7775
  store i8 %7778, ptr %7779, align 1, !tbaa !10
  %7780 = add nuw nsw i64 %7760, 4
  %7781 = icmp eq i64 %7780, 1200
  br i1 %7781, label %7782, label %7759, !llvm.loop !11

7782:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7257()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 257)
  br label %7800

7783:
  %7784 = add nuw nsw i64 %7801, 1
  %7785 = getelementptr inbounds nuw i8, ptr @dst, i64 %7784
  %7786 = load i8, ptr %7785, align 1, !tbaa !10
  %7787 = getelementptr inbounds nuw i8, ptr @ref, i64 %7784
  %7788 = load i8, ptr %7787, align 1, !tbaa !10
  %7789 = icmp eq i8 %7786, %7788
  br i1 %7789, label %7790, label %7807

7790:
  %7791 = add nuw nsw i64 %7801, 2
  %7792 = getelementptr inbounds nuw i8, ptr @dst, i64 %7791
  %7793 = load i8, ptr %7792, align 1, !tbaa !10
  %7794 = getelementptr inbounds nuw i8, ptr @ref, i64 %7791
  %7795 = load i8, ptr %7794, align 1, !tbaa !10
  %7796 = icmp eq i8 %7793, %7795
  br i1 %7796, label %7797, label %7807

7797:
  %7798 = add nuw nsw i64 %7801, 3
  %7799 = icmp eq i64 %7798, 1200
  br i1 %7799, label %7808, label %7800, !llvm.loop !144

7800:
  %7801 = phi i64 [ 0, %7782 ], [ %7798, %7797 ]
  %7802 = getelementptr inbounds nuw i8, ptr @dst, i64 %7801
  %7803 = load i8, ptr %7802, align 1, !tbaa !10
  %7804 = getelementptr inbounds nuw i8, ptr @ref, i64 %7801
  %7805 = load i8, ptr %7804, align 1, !tbaa !10
  %7806 = icmp eq i8 %7803, %7805
  br i1 %7806, label %7783, label %7807

7807:
  tail call void @report(i64 noundef 257)
  store i1 true, ptr @bad, align 4
  br label %7809

7808:
  br label %7809

7809:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7810

7810:
  %7811 = phi i64 [ 0, %7809 ], [ %7831, %7810 ]
  %7812 = trunc i64 %7811 to i8
  %7813 = mul i8 %7812, 7
  %7814 = or disjoint i8 %7813, 3
  %7815 = getelementptr inbounds nuw i8, ptr @src, i64 %7811
  store i8 %7814, ptr %7815, align 4, !tbaa !10
  %7816 = or disjoint i64 %7811, 1
  %7817 = trunc i64 %7816 to i8
  %7818 = mul i8 %7817, 7
  %7819 = add i8 %7818, 3
  %7820 = getelementptr inbounds nuw i8, ptr @src, i64 %7816
  store i8 %7819, ptr %7820, align 1, !tbaa !10
  %7821 = or disjoint i64 %7811, 2
  %7822 = trunc i64 %7821 to i8
  %7823 = mul i8 %7822, 7
  %7824 = add i8 %7823, 3
  %7825 = getelementptr inbounds nuw i8, ptr @src, i64 %7821
  store i8 %7824, ptr %7825, align 2, !tbaa !10
  %7826 = or disjoint i64 %7811, 3
  %7827 = trunc i64 %7826 to i8
  %7828 = mul i8 %7827, 7
  %7829 = add i8 %7828, 3
  %7830 = getelementptr inbounds nuw i8, ptr @src, i64 %7826
  store i8 %7829, ptr %7830, align 1, !tbaa !10
  %7831 = add nuw nsw i64 %7811, 4
  %7832 = icmp eq i64 %7831, 1200
  br i1 %7832, label %7833, label %7810, !llvm.loop !11

7833:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr257()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 257)
  br label %7851

7834:
  %7835 = add nuw nsw i64 %7852, 1
  %7836 = getelementptr inbounds nuw i8, ptr @dst, i64 %7835
  %7837 = load i8, ptr %7836, align 1, !tbaa !10
  %7838 = getelementptr inbounds nuw i8, ptr @ref, i64 %7835
  %7839 = load i8, ptr %7838, align 1, !tbaa !10
  %7840 = icmp eq i8 %7837, %7839
  br i1 %7840, label %7841, label %7858

7841:
  %7842 = add nuw nsw i64 %7852, 2
  %7843 = getelementptr inbounds nuw i8, ptr @dst, i64 %7842
  %7844 = load i8, ptr %7843, align 1, !tbaa !10
  %7845 = getelementptr inbounds nuw i8, ptr @ref, i64 %7842
  %7846 = load i8, ptr %7845, align 1, !tbaa !10
  %7847 = icmp eq i8 %7844, %7846
  br i1 %7847, label %7848, label %7858

7848:
  %7849 = add nuw nsw i64 %7852, 3
  %7850 = icmp eq i64 %7849, 1200
  br i1 %7850, label %7859, label %7851, !llvm.loop !144

7851:
  %7852 = phi i64 [ 0, %7833 ], [ %7849, %7848 ]
  %7853 = getelementptr inbounds nuw i8, ptr @dst, i64 %7852
  %7854 = load i8, ptr %7853, align 1, !tbaa !10
  %7855 = getelementptr inbounds nuw i8, ptr @ref, i64 %7852
  %7856 = load i8, ptr %7855, align 1, !tbaa !10
  %7857 = icmp eq i8 %7854, %7856
  br i1 %7857, label %7834, label %7858

7858:
  tail call void @report(i64 noundef 257)
  store i1 true, ptr @bad, align 4
  br label %7860

7859:
  br label %7860

7860:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7861

7861:
  %7862 = phi i64 [ 0, %7860 ], [ %7882, %7861 ]
  %7863 = trunc i64 %7862 to i8
  %7864 = mul i8 %7863, 7
  %7865 = or disjoint i8 %7864, 3
  %7866 = getelementptr inbounds nuw i8, ptr @src, i64 %7862
  store i8 %7865, ptr %7866, align 4, !tbaa !10
  %7867 = or disjoint i64 %7862, 1
  %7868 = trunc i64 %7867 to i8
  %7869 = mul i8 %7868, 7
  %7870 = add i8 %7869, 3
  %7871 = getelementptr inbounds nuw i8, ptr @src, i64 %7867
  store i8 %7870, ptr %7871, align 1, !tbaa !10
  %7872 = or disjoint i64 %7862, 2
  %7873 = trunc i64 %7872 to i8
  %7874 = mul i8 %7873, 7
  %7875 = add i8 %7874, 3
  %7876 = getelementptr inbounds nuw i8, ptr @src, i64 %7872
  store i8 %7875, ptr %7876, align 2, !tbaa !10
  %7877 = or disjoint i64 %7862, 3
  %7878 = trunc i64 %7877 to i8
  %7879 = mul i8 %7878, 7
  %7880 = add i8 %7879, 3
  %7881 = getelementptr inbounds nuw i8, ptr @src, i64 %7877
  store i8 %7880, ptr %7881, align 1, !tbaa !10
  %7882 = add nuw nsw i64 %7862, 4
  %7883 = icmp eq i64 %7882, 1200
  br i1 %7883, label %7884, label %7861, !llvm.loop !11

7884:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv257(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 257)
  br label %7902

7885:
  %7886 = add nuw nsw i64 %7903, 1
  %7887 = getelementptr inbounds nuw i8, ptr @dst, i64 %7886
  %7888 = load i8, ptr %7887, align 1, !tbaa !10
  %7889 = getelementptr inbounds nuw i8, ptr @ref, i64 %7886
  %7890 = load i8, ptr %7889, align 1, !tbaa !10
  %7891 = icmp eq i8 %7888, %7890
  br i1 %7891, label %7892, label %7909

7892:
  %7893 = add nuw nsw i64 %7903, 2
  %7894 = getelementptr inbounds nuw i8, ptr @dst, i64 %7893
  %7895 = load i8, ptr %7894, align 1, !tbaa !10
  %7896 = getelementptr inbounds nuw i8, ptr @ref, i64 %7893
  %7897 = load i8, ptr %7896, align 1, !tbaa !10
  %7898 = icmp eq i8 %7895, %7897
  br i1 %7898, label %7899, label %7909

7899:
  %7900 = add nuw nsw i64 %7903, 3
  %7901 = icmp eq i64 %7900, 1200
  br i1 %7901, label %7910, label %7902, !llvm.loop !144

7902:
  %7903 = phi i64 [ 0, %7884 ], [ %7900, %7899 ]
  %7904 = getelementptr inbounds nuw i8, ptr @dst, i64 %7903
  %7905 = load i8, ptr %7904, align 1, !tbaa !10
  %7906 = getelementptr inbounds nuw i8, ptr @ref, i64 %7903
  %7907 = load i8, ptr %7906, align 1, !tbaa !10
  %7908 = icmp eq i8 %7905, %7907
  br i1 %7908, label %7885, label %7909

7909:
  tail call void @report(i64 noundef 257)
  store i1 true, ptr @bad, align 4
  br label %7911

7910:
  br label %7911

7911:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7912

7912:
  %7913 = phi i64 [ 0, %7911 ], [ %7933, %7912 ]
  %7914 = trunc i64 %7913 to i8
  %7915 = mul i8 %7914, 7
  %7916 = or disjoint i8 %7915, 3
  %7917 = getelementptr inbounds nuw i8, ptr @src, i64 %7913
  store i8 %7916, ptr %7917, align 4, !tbaa !10
  %7918 = or disjoint i64 %7913, 1
  %7919 = trunc i64 %7918 to i8
  %7920 = mul i8 %7919, 7
  %7921 = add i8 %7920, 3
  %7922 = getelementptr inbounds nuw i8, ptr @src, i64 %7918
  store i8 %7921, ptr %7922, align 1, !tbaa !10
  %7923 = or disjoint i64 %7913, 2
  %7924 = trunc i64 %7923 to i8
  %7925 = mul i8 %7924, 7
  %7926 = add i8 %7925, 3
  %7927 = getelementptr inbounds nuw i8, ptr @src, i64 %7923
  store i8 %7926, ptr %7927, align 2, !tbaa !10
  %7928 = or disjoint i64 %7913, 3
  %7929 = trunc i64 %7928 to i8
  %7930 = mul i8 %7929, 7
  %7931 = add i8 %7930, 3
  %7932 = getelementptr inbounds nuw i8, ptr @src, i64 %7928
  store i8 %7931, ptr %7932, align 1, !tbaa !10
  %7933 = add nuw nsw i64 %7913, 4
  %7934 = icmp eq i64 %7933, 1200
  br i1 %7934, label %7935, label %7912, !llvm.loop !11

7935:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv257(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 257)
  br label %7953

7936:
  %7937 = add nuw nsw i64 %7954, 1
  %7938 = getelementptr inbounds nuw i8, ptr @dst, i64 %7937
  %7939 = load i8, ptr %7938, align 1, !tbaa !10
  %7940 = getelementptr inbounds nuw i8, ptr @ref, i64 %7937
  %7941 = load i8, ptr %7940, align 1, !tbaa !10
  %7942 = icmp eq i8 %7939, %7941
  br i1 %7942, label %7943, label %7960

7943:
  %7944 = add nuw nsw i64 %7954, 2
  %7945 = getelementptr inbounds nuw i8, ptr @dst, i64 %7944
  %7946 = load i8, ptr %7945, align 1, !tbaa !10
  %7947 = getelementptr inbounds nuw i8, ptr @ref, i64 %7944
  %7948 = load i8, ptr %7947, align 1, !tbaa !10
  %7949 = icmp eq i8 %7946, %7948
  br i1 %7949, label %7950, label %7960

7950:
  %7951 = add nuw nsw i64 %7954, 3
  %7952 = icmp eq i64 %7951, 1200
  br i1 %7952, label %7961, label %7953, !llvm.loop !144

7953:
  %7954 = phi i64 [ 0, %7935 ], [ %7951, %7950 ]
  %7955 = getelementptr inbounds nuw i8, ptr @dst, i64 %7954
  %7956 = load i8, ptr %7955, align 1, !tbaa !10
  %7957 = getelementptr inbounds nuw i8, ptr @ref, i64 %7954
  %7958 = load i8, ptr %7957, align 1, !tbaa !10
  %7959 = icmp eq i8 %7956, %7958
  br i1 %7959, label %7936, label %7960

7960:
  tail call void @report(i64 noundef 257)
  store i1 true, ptr @bad, align 4
  br label %7962

7961:
  br label %7962

7962:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %7963

7963:
  %7964 = phi i64 [ 0, %7962 ], [ %7984, %7963 ]
  %7965 = trunc i64 %7964 to i8
  %7966 = mul i8 %7965, 7
  %7967 = or disjoint i8 %7966, 3
  %7968 = getelementptr inbounds nuw i8, ptr @src, i64 %7964
  store i8 %7967, ptr %7968, align 4, !tbaa !10
  %7969 = or disjoint i64 %7964, 1
  %7970 = trunc i64 %7969 to i8
  %7971 = mul i8 %7970, 7
  %7972 = add i8 %7971, 3
  %7973 = getelementptr inbounds nuw i8, ptr @src, i64 %7969
  store i8 %7972, ptr %7973, align 1, !tbaa !10
  %7974 = or disjoint i64 %7964, 2
  %7975 = trunc i64 %7974 to i8
  %7976 = mul i8 %7975, 7
  %7977 = add i8 %7976, 3
  %7978 = getelementptr inbounds nuw i8, ptr @src, i64 %7974
  store i8 %7977, ptr %7978, align 2, !tbaa !10
  %7979 = or disjoint i64 %7964, 3
  %7980 = trunc i64 %7979 to i8
  %7981 = mul i8 %7980, 7
  %7982 = add i8 %7981, 3
  %7983 = getelementptr inbounds nuw i8, ptr @src, i64 %7979
  store i8 %7982, ptr %7983, align 1, !tbaa !10
  %7984 = add nuw nsw i64 %7964, 4
  %7985 = icmp eq i64 %7984, 1200
  br i1 %7985, label %7986, label %7963, !llvm.loop !11

7986:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp300()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 300)
  br label %8004

7987:
  %7988 = add nuw nsw i64 %8005, 1
  %7989 = getelementptr inbounds nuw i8, ptr @dst, i64 %7988
  %7990 = load i8, ptr %7989, align 1, !tbaa !10
  %7991 = getelementptr inbounds nuw i8, ptr @ref, i64 %7988
  %7992 = load i8, ptr %7991, align 1, !tbaa !10
  %7993 = icmp eq i8 %7990, %7992
  br i1 %7993, label %7994, label %8011

7994:
  %7995 = add nuw nsw i64 %8005, 2
  %7996 = getelementptr inbounds nuw i8, ptr @dst, i64 %7995
  %7997 = load i8, ptr %7996, align 1, !tbaa !10
  %7998 = getelementptr inbounds nuw i8, ptr @ref, i64 %7995
  %7999 = load i8, ptr %7998, align 1, !tbaa !10
  %8000 = icmp eq i8 %7997, %7999
  br i1 %8000, label %8001, label %8011

8001:
  %8002 = add nuw nsw i64 %8005, 3
  %8003 = icmp eq i64 %8002, 1200
  br i1 %8003, label %8012, label %8004, !llvm.loop !144

8004:
  %8005 = phi i64 [ 0, %7986 ], [ %8002, %8001 ]
  %8006 = getelementptr inbounds nuw i8, ptr @dst, i64 %8005
  %8007 = load i8, ptr %8006, align 1, !tbaa !10
  %8008 = getelementptr inbounds nuw i8, ptr @ref, i64 %8005
  %8009 = load i8, ptr %8008, align 1, !tbaa !10
  %8010 = icmp eq i8 %8007, %8009
  br i1 %8010, label %7987, label %8011

8011:
  tail call void @report(i64 noundef 300)
  store i1 true, ptr @bad, align 4
  br label %8013

8012:
  br label %8013

8013:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8014

8014:
  %8015 = phi i64 [ 0, %8013 ], [ %8035, %8014 ]
  %8016 = trunc i64 %8015 to i8
  %8017 = mul i8 %8016, 7
  %8018 = or disjoint i8 %8017, 3
  %8019 = getelementptr inbounds nuw i8, ptr @src, i64 %8015
  store i8 %8018, ptr %8019, align 4, !tbaa !10
  %8020 = or disjoint i64 %8015, 1
  %8021 = trunc i64 %8020 to i8
  %8022 = mul i8 %8021, 7
  %8023 = add i8 %8022, 3
  %8024 = getelementptr inbounds nuw i8, ptr @src, i64 %8020
  store i8 %8023, ptr %8024, align 1, !tbaa !10
  %8025 = or disjoint i64 %8015, 2
  %8026 = trunc i64 %8025 to i8
  %8027 = mul i8 %8026, 7
  %8028 = add i8 %8027, 3
  %8029 = getelementptr inbounds nuw i8, ptr @src, i64 %8025
  store i8 %8028, ptr %8029, align 2, !tbaa !10
  %8030 = or disjoint i64 %8015, 3
  %8031 = trunc i64 %8030 to i8
  %8032 = mul i8 %8031, 7
  %8033 = add i8 %8032, 3
  %8034 = getelementptr inbounds nuw i8, ptr @src, i64 %8030
  store i8 %8033, ptr %8034, align 1, !tbaa !10
  %8035 = add nuw nsw i64 %8015, 4
  %8036 = icmp eq i64 %8035, 1200
  br i1 %8036, label %8037, label %8014, !llvm.loop !11

8037:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr300()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 300)
  br label %8055

8038:
  %8039 = add nuw nsw i64 %8056, 1
  %8040 = getelementptr inbounds nuw i8, ptr @dst, i64 %8039
  %8041 = load i8, ptr %8040, align 1, !tbaa !10
  %8042 = getelementptr inbounds nuw i8, ptr @ref, i64 %8039
  %8043 = load i8, ptr %8042, align 1, !tbaa !10
  %8044 = icmp eq i8 %8041, %8043
  br i1 %8044, label %8045, label %8062

8045:
  %8046 = add nuw nsw i64 %8056, 2
  %8047 = getelementptr inbounds nuw i8, ptr @dst, i64 %8046
  %8048 = load i8, ptr %8047, align 1, !tbaa !10
  %8049 = getelementptr inbounds nuw i8, ptr @ref, i64 %8046
  %8050 = load i8, ptr %8049, align 1, !tbaa !10
  %8051 = icmp eq i8 %8048, %8050
  br i1 %8051, label %8052, label %8062

8052:
  %8053 = add nuw nsw i64 %8056, 3
  %8054 = icmp eq i64 %8053, 1200
  br i1 %8054, label %8063, label %8055, !llvm.loop !144

8055:
  %8056 = phi i64 [ 0, %8037 ], [ %8053, %8052 ]
  %8057 = getelementptr inbounds nuw i8, ptr @dst, i64 %8056
  %8058 = load i8, ptr %8057, align 1, !tbaa !10
  %8059 = getelementptr inbounds nuw i8, ptr @ref, i64 %8056
  %8060 = load i8, ptr %8059, align 1, !tbaa !10
  %8061 = icmp eq i8 %8058, %8060
  br i1 %8061, label %8038, label %8062

8062:
  tail call void @report(i64 noundef 300)
  store i1 true, ptr @bad, align 4
  br label %8064

8063:
  br label %8064

8064:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8065

8065:
  %8066 = phi i64 [ 0, %8064 ], [ %8086, %8065 ]
  %8067 = trunc i64 %8066 to i8
  %8068 = mul i8 %8067, 7
  %8069 = or disjoint i8 %8068, 3
  %8070 = getelementptr inbounds nuw i8, ptr @src, i64 %8066
  store i8 %8069, ptr %8070, align 4, !tbaa !10
  %8071 = or disjoint i64 %8066, 1
  %8072 = trunc i64 %8071 to i8
  %8073 = mul i8 %8072, 7
  %8074 = add i8 %8073, 3
  %8075 = getelementptr inbounds nuw i8, ptr @src, i64 %8071
  store i8 %8074, ptr %8075, align 1, !tbaa !10
  %8076 = or disjoint i64 %8066, 2
  %8077 = trunc i64 %8076 to i8
  %8078 = mul i8 %8077, 7
  %8079 = add i8 %8078, 3
  %8080 = getelementptr inbounds nuw i8, ptr @src, i64 %8076
  store i8 %8079, ptr %8080, align 2, !tbaa !10
  %8081 = or disjoint i64 %8066, 3
  %8082 = trunc i64 %8081 to i8
  %8083 = mul i8 %8082, 7
  %8084 = add i8 %8083, 3
  %8085 = getelementptr inbounds nuw i8, ptr @src, i64 %8081
  store i8 %8084, ptr %8085, align 1, !tbaa !10
  %8086 = add nuw nsw i64 %8066, 4
  %8087 = icmp eq i64 %8086, 1200
  br i1 %8087, label %8088, label %8065, !llvm.loop !11

8088:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7300()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 300)
  br label %8106

8089:
  %8090 = add nuw nsw i64 %8107, 1
  %8091 = getelementptr inbounds nuw i8, ptr @dst, i64 %8090
  %8092 = load i8, ptr %8091, align 1, !tbaa !10
  %8093 = getelementptr inbounds nuw i8, ptr @ref, i64 %8090
  %8094 = load i8, ptr %8093, align 1, !tbaa !10
  %8095 = icmp eq i8 %8092, %8094
  br i1 %8095, label %8096, label %8113

8096:
  %8097 = add nuw nsw i64 %8107, 2
  %8098 = getelementptr inbounds nuw i8, ptr @dst, i64 %8097
  %8099 = load i8, ptr %8098, align 1, !tbaa !10
  %8100 = getelementptr inbounds nuw i8, ptr @ref, i64 %8097
  %8101 = load i8, ptr %8100, align 1, !tbaa !10
  %8102 = icmp eq i8 %8099, %8101
  br i1 %8102, label %8103, label %8113

8103:
  %8104 = add nuw nsw i64 %8107, 3
  %8105 = icmp eq i64 %8104, 1200
  br i1 %8105, label %8114, label %8106, !llvm.loop !144

8106:
  %8107 = phi i64 [ 0, %8088 ], [ %8104, %8103 ]
  %8108 = getelementptr inbounds nuw i8, ptr @dst, i64 %8107
  %8109 = load i8, ptr %8108, align 1, !tbaa !10
  %8110 = getelementptr inbounds nuw i8, ptr @ref, i64 %8107
  %8111 = load i8, ptr %8110, align 1, !tbaa !10
  %8112 = icmp eq i8 %8109, %8111
  br i1 %8112, label %8089, label %8113

8113:
  tail call void @report(i64 noundef 300)
  store i1 true, ptr @bad, align 4
  br label %8115

8114:
  br label %8115

8115:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8116

8116:
  %8117 = phi i64 [ 0, %8115 ], [ %8137, %8116 ]
  %8118 = trunc i64 %8117 to i8
  %8119 = mul i8 %8118, 7
  %8120 = or disjoint i8 %8119, 3
  %8121 = getelementptr inbounds nuw i8, ptr @src, i64 %8117
  store i8 %8120, ptr %8121, align 4, !tbaa !10
  %8122 = or disjoint i64 %8117, 1
  %8123 = trunc i64 %8122 to i8
  %8124 = mul i8 %8123, 7
  %8125 = add i8 %8124, 3
  %8126 = getelementptr inbounds nuw i8, ptr @src, i64 %8122
  store i8 %8125, ptr %8126, align 1, !tbaa !10
  %8127 = or disjoint i64 %8117, 2
  %8128 = trunc i64 %8127 to i8
  %8129 = mul i8 %8128, 7
  %8130 = add i8 %8129, 3
  %8131 = getelementptr inbounds nuw i8, ptr @src, i64 %8127
  store i8 %8130, ptr %8131, align 2, !tbaa !10
  %8132 = or disjoint i64 %8117, 3
  %8133 = trunc i64 %8132 to i8
  %8134 = mul i8 %8133, 7
  %8135 = add i8 %8134, 3
  %8136 = getelementptr inbounds nuw i8, ptr @src, i64 %8132
  store i8 %8135, ptr %8136, align 1, !tbaa !10
  %8137 = add nuw nsw i64 %8117, 4
  %8138 = icmp eq i64 %8137, 1200
  br i1 %8138, label %8139, label %8116, !llvm.loop !11

8139:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr300()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 300)
  br label %8157

8140:
  %8141 = add nuw nsw i64 %8158, 1
  %8142 = getelementptr inbounds nuw i8, ptr @dst, i64 %8141
  %8143 = load i8, ptr %8142, align 1, !tbaa !10
  %8144 = getelementptr inbounds nuw i8, ptr @ref, i64 %8141
  %8145 = load i8, ptr %8144, align 1, !tbaa !10
  %8146 = icmp eq i8 %8143, %8145
  br i1 %8146, label %8147, label %8164

8147:
  %8148 = add nuw nsw i64 %8158, 2
  %8149 = getelementptr inbounds nuw i8, ptr @dst, i64 %8148
  %8150 = load i8, ptr %8149, align 1, !tbaa !10
  %8151 = getelementptr inbounds nuw i8, ptr @ref, i64 %8148
  %8152 = load i8, ptr %8151, align 1, !tbaa !10
  %8153 = icmp eq i8 %8150, %8152
  br i1 %8153, label %8154, label %8164

8154:
  %8155 = add nuw nsw i64 %8158, 3
  %8156 = icmp eq i64 %8155, 1200
  br i1 %8156, label %8165, label %8157, !llvm.loop !144

8157:
  %8158 = phi i64 [ 0, %8139 ], [ %8155, %8154 ]
  %8159 = getelementptr inbounds nuw i8, ptr @dst, i64 %8158
  %8160 = load i8, ptr %8159, align 1, !tbaa !10
  %8161 = getelementptr inbounds nuw i8, ptr @ref, i64 %8158
  %8162 = load i8, ptr %8161, align 1, !tbaa !10
  %8163 = icmp eq i8 %8160, %8162
  br i1 %8163, label %8140, label %8164

8164:
  tail call void @report(i64 noundef 300)
  store i1 true, ptr @bad, align 4
  br label %8166

8165:
  br label %8166

8166:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8167

8167:
  %8168 = phi i64 [ 0, %8166 ], [ %8188, %8167 ]
  %8169 = trunc i64 %8168 to i8
  %8170 = mul i8 %8169, 7
  %8171 = or disjoint i8 %8170, 3
  %8172 = getelementptr inbounds nuw i8, ptr @src, i64 %8168
  store i8 %8171, ptr %8172, align 4, !tbaa !10
  %8173 = or disjoint i64 %8168, 1
  %8174 = trunc i64 %8173 to i8
  %8175 = mul i8 %8174, 7
  %8176 = add i8 %8175, 3
  %8177 = getelementptr inbounds nuw i8, ptr @src, i64 %8173
  store i8 %8176, ptr %8177, align 1, !tbaa !10
  %8178 = or disjoint i64 %8168, 2
  %8179 = trunc i64 %8178 to i8
  %8180 = mul i8 %8179, 7
  %8181 = add i8 %8180, 3
  %8182 = getelementptr inbounds nuw i8, ptr @src, i64 %8178
  store i8 %8181, ptr %8182, align 2, !tbaa !10
  %8183 = or disjoint i64 %8168, 3
  %8184 = trunc i64 %8183 to i8
  %8185 = mul i8 %8184, 7
  %8186 = add i8 %8185, 3
  %8187 = getelementptr inbounds nuw i8, ptr @src, i64 %8183
  store i8 %8186, ptr %8187, align 1, !tbaa !10
  %8188 = add nuw nsw i64 %8168, 4
  %8189 = icmp eq i64 %8188, 1200
  br i1 %8189, label %8190, label %8167, !llvm.loop !11

8190:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv300(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 300)
  br label %8208

8191:
  %8192 = add nuw nsw i64 %8209, 1
  %8193 = getelementptr inbounds nuw i8, ptr @dst, i64 %8192
  %8194 = load i8, ptr %8193, align 1, !tbaa !10
  %8195 = getelementptr inbounds nuw i8, ptr @ref, i64 %8192
  %8196 = load i8, ptr %8195, align 1, !tbaa !10
  %8197 = icmp eq i8 %8194, %8196
  br i1 %8197, label %8198, label %8215

8198:
  %8199 = add nuw nsw i64 %8209, 2
  %8200 = getelementptr inbounds nuw i8, ptr @dst, i64 %8199
  %8201 = load i8, ptr %8200, align 1, !tbaa !10
  %8202 = getelementptr inbounds nuw i8, ptr @ref, i64 %8199
  %8203 = load i8, ptr %8202, align 1, !tbaa !10
  %8204 = icmp eq i8 %8201, %8203
  br i1 %8204, label %8205, label %8215

8205:
  %8206 = add nuw nsw i64 %8209, 3
  %8207 = icmp eq i64 %8206, 1200
  br i1 %8207, label %8216, label %8208, !llvm.loop !144

8208:
  %8209 = phi i64 [ 0, %8190 ], [ %8206, %8205 ]
  %8210 = getelementptr inbounds nuw i8, ptr @dst, i64 %8209
  %8211 = load i8, ptr %8210, align 1, !tbaa !10
  %8212 = getelementptr inbounds nuw i8, ptr @ref, i64 %8209
  %8213 = load i8, ptr %8212, align 1, !tbaa !10
  %8214 = icmp eq i8 %8211, %8213
  br i1 %8214, label %8191, label %8215

8215:
  tail call void @report(i64 noundef 300)
  store i1 true, ptr @bad, align 4
  br label %8217

8216:
  br label %8217

8217:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8218

8218:
  %8219 = phi i64 [ 0, %8217 ], [ %8239, %8218 ]
  %8220 = trunc i64 %8219 to i8
  %8221 = mul i8 %8220, 7
  %8222 = or disjoint i8 %8221, 3
  %8223 = getelementptr inbounds nuw i8, ptr @src, i64 %8219
  store i8 %8222, ptr %8223, align 4, !tbaa !10
  %8224 = or disjoint i64 %8219, 1
  %8225 = trunc i64 %8224 to i8
  %8226 = mul i8 %8225, 7
  %8227 = add i8 %8226, 3
  %8228 = getelementptr inbounds nuw i8, ptr @src, i64 %8224
  store i8 %8227, ptr %8228, align 1, !tbaa !10
  %8229 = or disjoint i64 %8219, 2
  %8230 = trunc i64 %8229 to i8
  %8231 = mul i8 %8230, 7
  %8232 = add i8 %8231, 3
  %8233 = getelementptr inbounds nuw i8, ptr @src, i64 %8229
  store i8 %8232, ptr %8233, align 2, !tbaa !10
  %8234 = or disjoint i64 %8219, 3
  %8235 = trunc i64 %8234 to i8
  %8236 = mul i8 %8235, 7
  %8237 = add i8 %8236, 3
  %8238 = getelementptr inbounds nuw i8, ptr @src, i64 %8234
  store i8 %8237, ptr %8238, align 1, !tbaa !10
  %8239 = add nuw nsw i64 %8219, 4
  %8240 = icmp eq i64 %8239, 1200
  br i1 %8240, label %8241, label %8218, !llvm.loop !11

8241:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv300(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 300)
  br label %8259

8242:
  %8243 = add nuw nsw i64 %8260, 1
  %8244 = getelementptr inbounds nuw i8, ptr @dst, i64 %8243
  %8245 = load i8, ptr %8244, align 1, !tbaa !10
  %8246 = getelementptr inbounds nuw i8, ptr @ref, i64 %8243
  %8247 = load i8, ptr %8246, align 1, !tbaa !10
  %8248 = icmp eq i8 %8245, %8247
  br i1 %8248, label %8249, label %8266

8249:
  %8250 = add nuw nsw i64 %8260, 2
  %8251 = getelementptr inbounds nuw i8, ptr @dst, i64 %8250
  %8252 = load i8, ptr %8251, align 1, !tbaa !10
  %8253 = getelementptr inbounds nuw i8, ptr @ref, i64 %8250
  %8254 = load i8, ptr %8253, align 1, !tbaa !10
  %8255 = icmp eq i8 %8252, %8254
  br i1 %8255, label %8256, label %8266

8256:
  %8257 = add nuw nsw i64 %8260, 3
  %8258 = icmp eq i64 %8257, 1200
  br i1 %8258, label %8267, label %8259, !llvm.loop !144

8259:
  %8260 = phi i64 [ 0, %8241 ], [ %8257, %8256 ]
  %8261 = getelementptr inbounds nuw i8, ptr @dst, i64 %8260
  %8262 = load i8, ptr %8261, align 1, !tbaa !10
  %8263 = getelementptr inbounds nuw i8, ptr @ref, i64 %8260
  %8264 = load i8, ptr %8263, align 1, !tbaa !10
  %8265 = icmp eq i8 %8262, %8264
  br i1 %8265, label %8242, label %8266

8266:
  tail call void @report(i64 noundef 300)
  store i1 true, ptr @bad, align 4
  br label %8268

8267:
  br label %8268

8268:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8269

8269:
  %8270 = phi i64 [ 0, %8268 ], [ %8290, %8269 ]
  %8271 = trunc i64 %8270 to i8
  %8272 = mul i8 %8271, 7
  %8273 = or disjoint i8 %8272, 3
  %8274 = getelementptr inbounds nuw i8, ptr @src, i64 %8270
  store i8 %8273, ptr %8274, align 4, !tbaa !10
  %8275 = or disjoint i64 %8270, 1
  %8276 = trunc i64 %8275 to i8
  %8277 = mul i8 %8276, 7
  %8278 = add i8 %8277, 3
  %8279 = getelementptr inbounds nuw i8, ptr @src, i64 %8275
  store i8 %8278, ptr %8279, align 1, !tbaa !10
  %8280 = or disjoint i64 %8270, 2
  %8281 = trunc i64 %8280 to i8
  %8282 = mul i8 %8281, 7
  %8283 = add i8 %8282, 3
  %8284 = getelementptr inbounds nuw i8, ptr @src, i64 %8280
  store i8 %8283, ptr %8284, align 2, !tbaa !10
  %8285 = or disjoint i64 %8270, 3
  %8286 = trunc i64 %8285 to i8
  %8287 = mul i8 %8286, 7
  %8288 = add i8 %8287, 3
  %8289 = getelementptr inbounds nuw i8, ptr @src, i64 %8285
  store i8 %8288, ptr %8289, align 1, !tbaa !10
  %8290 = add nuw nsw i64 %8270, 4
  %8291 = icmp eq i64 %8290, 1200
  br i1 %8291, label %8292, label %8269, !llvm.loop !11

8292:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp511()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 511)
  br label %8310

8293:
  %8294 = add nuw nsw i64 %8311, 1
  %8295 = getelementptr inbounds nuw i8, ptr @dst, i64 %8294
  %8296 = load i8, ptr %8295, align 1, !tbaa !10
  %8297 = getelementptr inbounds nuw i8, ptr @ref, i64 %8294
  %8298 = load i8, ptr %8297, align 1, !tbaa !10
  %8299 = icmp eq i8 %8296, %8298
  br i1 %8299, label %8300, label %8317

8300:
  %8301 = add nuw nsw i64 %8311, 2
  %8302 = getelementptr inbounds nuw i8, ptr @dst, i64 %8301
  %8303 = load i8, ptr %8302, align 1, !tbaa !10
  %8304 = getelementptr inbounds nuw i8, ptr @ref, i64 %8301
  %8305 = load i8, ptr %8304, align 1, !tbaa !10
  %8306 = icmp eq i8 %8303, %8305
  br i1 %8306, label %8307, label %8317

8307:
  %8308 = add nuw nsw i64 %8311, 3
  %8309 = icmp eq i64 %8308, 1200
  br i1 %8309, label %8318, label %8310, !llvm.loop !144

8310:
  %8311 = phi i64 [ 0, %8292 ], [ %8308, %8307 ]
  %8312 = getelementptr inbounds nuw i8, ptr @dst, i64 %8311
  %8313 = load i8, ptr %8312, align 1, !tbaa !10
  %8314 = getelementptr inbounds nuw i8, ptr @ref, i64 %8311
  %8315 = load i8, ptr %8314, align 1, !tbaa !10
  %8316 = icmp eq i8 %8313, %8315
  br i1 %8316, label %8293, label %8317

8317:
  tail call void @report(i64 noundef 511)
  store i1 true, ptr @bad, align 4
  br label %8319

8318:
  br label %8319

8319:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8320

8320:
  %8321 = phi i64 [ 0, %8319 ], [ %8341, %8320 ]
  %8322 = trunc i64 %8321 to i8
  %8323 = mul i8 %8322, 7
  %8324 = or disjoint i8 %8323, 3
  %8325 = getelementptr inbounds nuw i8, ptr @src, i64 %8321
  store i8 %8324, ptr %8325, align 4, !tbaa !10
  %8326 = or disjoint i64 %8321, 1
  %8327 = trunc i64 %8326 to i8
  %8328 = mul i8 %8327, 7
  %8329 = add i8 %8328, 3
  %8330 = getelementptr inbounds nuw i8, ptr @src, i64 %8326
  store i8 %8329, ptr %8330, align 1, !tbaa !10
  %8331 = or disjoint i64 %8321, 2
  %8332 = trunc i64 %8331 to i8
  %8333 = mul i8 %8332, 7
  %8334 = add i8 %8333, 3
  %8335 = getelementptr inbounds nuw i8, ptr @src, i64 %8331
  store i8 %8334, ptr %8335, align 2, !tbaa !10
  %8336 = or disjoint i64 %8321, 3
  %8337 = trunc i64 %8336 to i8
  %8338 = mul i8 %8337, 7
  %8339 = add i8 %8338, 3
  %8340 = getelementptr inbounds nuw i8, ptr @src, i64 %8336
  store i8 %8339, ptr %8340, align 1, !tbaa !10
  %8341 = add nuw nsw i64 %8321, 4
  %8342 = icmp eq i64 %8341, 1200
  br i1 %8342, label %8343, label %8320, !llvm.loop !11

8343:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr511()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 511)
  br label %8361

8344:
  %8345 = add nuw nsw i64 %8362, 1
  %8346 = getelementptr inbounds nuw i8, ptr @dst, i64 %8345
  %8347 = load i8, ptr %8346, align 1, !tbaa !10
  %8348 = getelementptr inbounds nuw i8, ptr @ref, i64 %8345
  %8349 = load i8, ptr %8348, align 1, !tbaa !10
  %8350 = icmp eq i8 %8347, %8349
  br i1 %8350, label %8351, label %8368

8351:
  %8352 = add nuw nsw i64 %8362, 2
  %8353 = getelementptr inbounds nuw i8, ptr @dst, i64 %8352
  %8354 = load i8, ptr %8353, align 1, !tbaa !10
  %8355 = getelementptr inbounds nuw i8, ptr @ref, i64 %8352
  %8356 = load i8, ptr %8355, align 1, !tbaa !10
  %8357 = icmp eq i8 %8354, %8356
  br i1 %8357, label %8358, label %8368

8358:
  %8359 = add nuw nsw i64 %8362, 3
  %8360 = icmp eq i64 %8359, 1200
  br i1 %8360, label %8369, label %8361, !llvm.loop !144

8361:
  %8362 = phi i64 [ 0, %8343 ], [ %8359, %8358 ]
  %8363 = getelementptr inbounds nuw i8, ptr @dst, i64 %8362
  %8364 = load i8, ptr %8363, align 1, !tbaa !10
  %8365 = getelementptr inbounds nuw i8, ptr @ref, i64 %8362
  %8366 = load i8, ptr %8365, align 1, !tbaa !10
  %8367 = icmp eq i8 %8364, %8366
  br i1 %8367, label %8344, label %8368

8368:
  tail call void @report(i64 noundef 511)
  store i1 true, ptr @bad, align 4
  br label %8370

8369:
  br label %8370

8370:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8371

8371:
  %8372 = phi i64 [ 0, %8370 ], [ %8392, %8371 ]
  %8373 = trunc i64 %8372 to i8
  %8374 = mul i8 %8373, 7
  %8375 = or disjoint i8 %8374, 3
  %8376 = getelementptr inbounds nuw i8, ptr @src, i64 %8372
  store i8 %8375, ptr %8376, align 4, !tbaa !10
  %8377 = or disjoint i64 %8372, 1
  %8378 = trunc i64 %8377 to i8
  %8379 = mul i8 %8378, 7
  %8380 = add i8 %8379, 3
  %8381 = getelementptr inbounds nuw i8, ptr @src, i64 %8377
  store i8 %8380, ptr %8381, align 1, !tbaa !10
  %8382 = or disjoint i64 %8372, 2
  %8383 = trunc i64 %8382 to i8
  %8384 = mul i8 %8383, 7
  %8385 = add i8 %8384, 3
  %8386 = getelementptr inbounds nuw i8, ptr @src, i64 %8382
  store i8 %8385, ptr %8386, align 2, !tbaa !10
  %8387 = or disjoint i64 %8372, 3
  %8388 = trunc i64 %8387 to i8
  %8389 = mul i8 %8388, 7
  %8390 = add i8 %8389, 3
  %8391 = getelementptr inbounds nuw i8, ptr @src, i64 %8387
  store i8 %8390, ptr %8391, align 1, !tbaa !10
  %8392 = add nuw nsw i64 %8372, 4
  %8393 = icmp eq i64 %8392, 1200
  br i1 %8393, label %8394, label %8371, !llvm.loop !11

8394:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7511()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 511)
  br label %8412

8395:
  %8396 = add nuw nsw i64 %8413, 1
  %8397 = getelementptr inbounds nuw i8, ptr @dst, i64 %8396
  %8398 = load i8, ptr %8397, align 1, !tbaa !10
  %8399 = getelementptr inbounds nuw i8, ptr @ref, i64 %8396
  %8400 = load i8, ptr %8399, align 1, !tbaa !10
  %8401 = icmp eq i8 %8398, %8400
  br i1 %8401, label %8402, label %8419

8402:
  %8403 = add nuw nsw i64 %8413, 2
  %8404 = getelementptr inbounds nuw i8, ptr @dst, i64 %8403
  %8405 = load i8, ptr %8404, align 1, !tbaa !10
  %8406 = getelementptr inbounds nuw i8, ptr @ref, i64 %8403
  %8407 = load i8, ptr %8406, align 1, !tbaa !10
  %8408 = icmp eq i8 %8405, %8407
  br i1 %8408, label %8409, label %8419

8409:
  %8410 = add nuw nsw i64 %8413, 3
  %8411 = icmp eq i64 %8410, 1200
  br i1 %8411, label %8420, label %8412, !llvm.loop !144

8412:
  %8413 = phi i64 [ 0, %8394 ], [ %8410, %8409 ]
  %8414 = getelementptr inbounds nuw i8, ptr @dst, i64 %8413
  %8415 = load i8, ptr %8414, align 1, !tbaa !10
  %8416 = getelementptr inbounds nuw i8, ptr @ref, i64 %8413
  %8417 = load i8, ptr %8416, align 1, !tbaa !10
  %8418 = icmp eq i8 %8415, %8417
  br i1 %8418, label %8395, label %8419

8419:
  tail call void @report(i64 noundef 511)
  store i1 true, ptr @bad, align 4
  br label %8421

8420:
  br label %8421

8421:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8422

8422:
  %8423 = phi i64 [ 0, %8421 ], [ %8443, %8422 ]
  %8424 = trunc i64 %8423 to i8
  %8425 = mul i8 %8424, 7
  %8426 = or disjoint i8 %8425, 3
  %8427 = getelementptr inbounds nuw i8, ptr @src, i64 %8423
  store i8 %8426, ptr %8427, align 4, !tbaa !10
  %8428 = or disjoint i64 %8423, 1
  %8429 = trunc i64 %8428 to i8
  %8430 = mul i8 %8429, 7
  %8431 = add i8 %8430, 3
  %8432 = getelementptr inbounds nuw i8, ptr @src, i64 %8428
  store i8 %8431, ptr %8432, align 1, !tbaa !10
  %8433 = or disjoint i64 %8423, 2
  %8434 = trunc i64 %8433 to i8
  %8435 = mul i8 %8434, 7
  %8436 = add i8 %8435, 3
  %8437 = getelementptr inbounds nuw i8, ptr @src, i64 %8433
  store i8 %8436, ptr %8437, align 2, !tbaa !10
  %8438 = or disjoint i64 %8423, 3
  %8439 = trunc i64 %8438 to i8
  %8440 = mul i8 %8439, 7
  %8441 = add i8 %8440, 3
  %8442 = getelementptr inbounds nuw i8, ptr @src, i64 %8438
  store i8 %8441, ptr %8442, align 1, !tbaa !10
  %8443 = add nuw nsw i64 %8423, 4
  %8444 = icmp eq i64 %8443, 1200
  br i1 %8444, label %8445, label %8422, !llvm.loop !11

8445:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr511()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 511)
  br label %8463

8446:
  %8447 = add nuw nsw i64 %8464, 1
  %8448 = getelementptr inbounds nuw i8, ptr @dst, i64 %8447
  %8449 = load i8, ptr %8448, align 1, !tbaa !10
  %8450 = getelementptr inbounds nuw i8, ptr @ref, i64 %8447
  %8451 = load i8, ptr %8450, align 1, !tbaa !10
  %8452 = icmp eq i8 %8449, %8451
  br i1 %8452, label %8453, label %8470

8453:
  %8454 = add nuw nsw i64 %8464, 2
  %8455 = getelementptr inbounds nuw i8, ptr @dst, i64 %8454
  %8456 = load i8, ptr %8455, align 1, !tbaa !10
  %8457 = getelementptr inbounds nuw i8, ptr @ref, i64 %8454
  %8458 = load i8, ptr %8457, align 1, !tbaa !10
  %8459 = icmp eq i8 %8456, %8458
  br i1 %8459, label %8460, label %8470

8460:
  %8461 = add nuw nsw i64 %8464, 3
  %8462 = icmp eq i64 %8461, 1200
  br i1 %8462, label %8471, label %8463, !llvm.loop !144

8463:
  %8464 = phi i64 [ 0, %8445 ], [ %8461, %8460 ]
  %8465 = getelementptr inbounds nuw i8, ptr @dst, i64 %8464
  %8466 = load i8, ptr %8465, align 1, !tbaa !10
  %8467 = getelementptr inbounds nuw i8, ptr @ref, i64 %8464
  %8468 = load i8, ptr %8467, align 1, !tbaa !10
  %8469 = icmp eq i8 %8466, %8468
  br i1 %8469, label %8446, label %8470

8470:
  tail call void @report(i64 noundef 511)
  store i1 true, ptr @bad, align 4
  br label %8472

8471:
  br label %8472

8472:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8473

8473:
  %8474 = phi i64 [ 0, %8472 ], [ %8494, %8473 ]
  %8475 = trunc i64 %8474 to i8
  %8476 = mul i8 %8475, 7
  %8477 = or disjoint i8 %8476, 3
  %8478 = getelementptr inbounds nuw i8, ptr @src, i64 %8474
  store i8 %8477, ptr %8478, align 4, !tbaa !10
  %8479 = or disjoint i64 %8474, 1
  %8480 = trunc i64 %8479 to i8
  %8481 = mul i8 %8480, 7
  %8482 = add i8 %8481, 3
  %8483 = getelementptr inbounds nuw i8, ptr @src, i64 %8479
  store i8 %8482, ptr %8483, align 1, !tbaa !10
  %8484 = or disjoint i64 %8474, 2
  %8485 = trunc i64 %8484 to i8
  %8486 = mul i8 %8485, 7
  %8487 = add i8 %8486, 3
  %8488 = getelementptr inbounds nuw i8, ptr @src, i64 %8484
  store i8 %8487, ptr %8488, align 2, !tbaa !10
  %8489 = or disjoint i64 %8474, 3
  %8490 = trunc i64 %8489 to i8
  %8491 = mul i8 %8490, 7
  %8492 = add i8 %8491, 3
  %8493 = getelementptr inbounds nuw i8, ptr @src, i64 %8489
  store i8 %8492, ptr %8493, align 1, !tbaa !10
  %8494 = add nuw nsw i64 %8474, 4
  %8495 = icmp eq i64 %8494, 1200
  br i1 %8495, label %8496, label %8473, !llvm.loop !11

8496:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv511(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 511)
  br label %8514

8497:
  %8498 = add nuw nsw i64 %8515, 1
  %8499 = getelementptr inbounds nuw i8, ptr @dst, i64 %8498
  %8500 = load i8, ptr %8499, align 1, !tbaa !10
  %8501 = getelementptr inbounds nuw i8, ptr @ref, i64 %8498
  %8502 = load i8, ptr %8501, align 1, !tbaa !10
  %8503 = icmp eq i8 %8500, %8502
  br i1 %8503, label %8504, label %8521

8504:
  %8505 = add nuw nsw i64 %8515, 2
  %8506 = getelementptr inbounds nuw i8, ptr @dst, i64 %8505
  %8507 = load i8, ptr %8506, align 1, !tbaa !10
  %8508 = getelementptr inbounds nuw i8, ptr @ref, i64 %8505
  %8509 = load i8, ptr %8508, align 1, !tbaa !10
  %8510 = icmp eq i8 %8507, %8509
  br i1 %8510, label %8511, label %8521

8511:
  %8512 = add nuw nsw i64 %8515, 3
  %8513 = icmp eq i64 %8512, 1200
  br i1 %8513, label %8522, label %8514, !llvm.loop !144

8514:
  %8515 = phi i64 [ 0, %8496 ], [ %8512, %8511 ]
  %8516 = getelementptr inbounds nuw i8, ptr @dst, i64 %8515
  %8517 = load i8, ptr %8516, align 1, !tbaa !10
  %8518 = getelementptr inbounds nuw i8, ptr @ref, i64 %8515
  %8519 = load i8, ptr %8518, align 1, !tbaa !10
  %8520 = icmp eq i8 %8517, %8519
  br i1 %8520, label %8497, label %8521

8521:
  tail call void @report(i64 noundef 511)
  store i1 true, ptr @bad, align 4
  br label %8523

8522:
  br label %8523

8523:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8524

8524:
  %8525 = phi i64 [ 0, %8523 ], [ %8545, %8524 ]
  %8526 = trunc i64 %8525 to i8
  %8527 = mul i8 %8526, 7
  %8528 = or disjoint i8 %8527, 3
  %8529 = getelementptr inbounds nuw i8, ptr @src, i64 %8525
  store i8 %8528, ptr %8529, align 4, !tbaa !10
  %8530 = or disjoint i64 %8525, 1
  %8531 = trunc i64 %8530 to i8
  %8532 = mul i8 %8531, 7
  %8533 = add i8 %8532, 3
  %8534 = getelementptr inbounds nuw i8, ptr @src, i64 %8530
  store i8 %8533, ptr %8534, align 1, !tbaa !10
  %8535 = or disjoint i64 %8525, 2
  %8536 = trunc i64 %8535 to i8
  %8537 = mul i8 %8536, 7
  %8538 = add i8 %8537, 3
  %8539 = getelementptr inbounds nuw i8, ptr @src, i64 %8535
  store i8 %8538, ptr %8539, align 2, !tbaa !10
  %8540 = or disjoint i64 %8525, 3
  %8541 = trunc i64 %8540 to i8
  %8542 = mul i8 %8541, 7
  %8543 = add i8 %8542, 3
  %8544 = getelementptr inbounds nuw i8, ptr @src, i64 %8540
  store i8 %8543, ptr %8544, align 1, !tbaa !10
  %8545 = add nuw nsw i64 %8525, 4
  %8546 = icmp eq i64 %8545, 1200
  br i1 %8546, label %8547, label %8524, !llvm.loop !11

8547:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv511(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 511)
  br label %8565

8548:
  %8549 = add nuw nsw i64 %8566, 1
  %8550 = getelementptr inbounds nuw i8, ptr @dst, i64 %8549
  %8551 = load i8, ptr %8550, align 1, !tbaa !10
  %8552 = getelementptr inbounds nuw i8, ptr @ref, i64 %8549
  %8553 = load i8, ptr %8552, align 1, !tbaa !10
  %8554 = icmp eq i8 %8551, %8553
  br i1 %8554, label %8555, label %8572

8555:
  %8556 = add nuw nsw i64 %8566, 2
  %8557 = getelementptr inbounds nuw i8, ptr @dst, i64 %8556
  %8558 = load i8, ptr %8557, align 1, !tbaa !10
  %8559 = getelementptr inbounds nuw i8, ptr @ref, i64 %8556
  %8560 = load i8, ptr %8559, align 1, !tbaa !10
  %8561 = icmp eq i8 %8558, %8560
  br i1 %8561, label %8562, label %8572

8562:
  %8563 = add nuw nsw i64 %8566, 3
  %8564 = icmp eq i64 %8563, 1200
  br i1 %8564, label %8573, label %8565, !llvm.loop !144

8565:
  %8566 = phi i64 [ 0, %8547 ], [ %8563, %8562 ]
  %8567 = getelementptr inbounds nuw i8, ptr @dst, i64 %8566
  %8568 = load i8, ptr %8567, align 1, !tbaa !10
  %8569 = getelementptr inbounds nuw i8, ptr @ref, i64 %8566
  %8570 = load i8, ptr %8569, align 1, !tbaa !10
  %8571 = icmp eq i8 %8568, %8570
  br i1 %8571, label %8548, label %8572

8572:
  tail call void @report(i64 noundef 511)
  store i1 true, ptr @bad, align 4
  br label %8574

8573:
  br label %8574

8574:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8575

8575:
  %8576 = phi i64 [ 0, %8574 ], [ %8596, %8575 ]
  %8577 = trunc i64 %8576 to i8
  %8578 = mul i8 %8577, 7
  %8579 = or disjoint i8 %8578, 3
  %8580 = getelementptr inbounds nuw i8, ptr @src, i64 %8576
  store i8 %8579, ptr %8580, align 4, !tbaa !10
  %8581 = or disjoint i64 %8576, 1
  %8582 = trunc i64 %8581 to i8
  %8583 = mul i8 %8582, 7
  %8584 = add i8 %8583, 3
  %8585 = getelementptr inbounds nuw i8, ptr @src, i64 %8581
  store i8 %8584, ptr %8585, align 1, !tbaa !10
  %8586 = or disjoint i64 %8576, 2
  %8587 = trunc i64 %8586 to i8
  %8588 = mul i8 %8587, 7
  %8589 = add i8 %8588, 3
  %8590 = getelementptr inbounds nuw i8, ptr @src, i64 %8586
  store i8 %8589, ptr %8590, align 2, !tbaa !10
  %8591 = or disjoint i64 %8576, 3
  %8592 = trunc i64 %8591 to i8
  %8593 = mul i8 %8592, 7
  %8594 = add i8 %8593, 3
  %8595 = getelementptr inbounds nuw i8, ptr @src, i64 %8591
  store i8 %8594, ptr %8595, align 1, !tbaa !10
  %8596 = add nuw nsw i64 %8576, 4
  %8597 = icmp eq i64 %8596, 1200
  br i1 %8597, label %8598, label %8575, !llvm.loop !11

8598:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp512()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 512)
  br label %8616

8599:
  %8600 = add nuw nsw i64 %8617, 1
  %8601 = getelementptr inbounds nuw i8, ptr @dst, i64 %8600
  %8602 = load i8, ptr %8601, align 1, !tbaa !10
  %8603 = getelementptr inbounds nuw i8, ptr @ref, i64 %8600
  %8604 = load i8, ptr %8603, align 1, !tbaa !10
  %8605 = icmp eq i8 %8602, %8604
  br i1 %8605, label %8606, label %8623

8606:
  %8607 = add nuw nsw i64 %8617, 2
  %8608 = getelementptr inbounds nuw i8, ptr @dst, i64 %8607
  %8609 = load i8, ptr %8608, align 1, !tbaa !10
  %8610 = getelementptr inbounds nuw i8, ptr @ref, i64 %8607
  %8611 = load i8, ptr %8610, align 1, !tbaa !10
  %8612 = icmp eq i8 %8609, %8611
  br i1 %8612, label %8613, label %8623

8613:
  %8614 = add nuw nsw i64 %8617, 3
  %8615 = icmp eq i64 %8614, 1200
  br i1 %8615, label %8624, label %8616, !llvm.loop !144

8616:
  %8617 = phi i64 [ 0, %8598 ], [ %8614, %8613 ]
  %8618 = getelementptr inbounds nuw i8, ptr @dst, i64 %8617
  %8619 = load i8, ptr %8618, align 1, !tbaa !10
  %8620 = getelementptr inbounds nuw i8, ptr @ref, i64 %8617
  %8621 = load i8, ptr %8620, align 1, !tbaa !10
  %8622 = icmp eq i8 %8619, %8621
  br i1 %8622, label %8599, label %8623

8623:
  tail call void @report(i64 noundef 512)
  store i1 true, ptr @bad, align 4
  br label %8625

8624:
  br label %8625

8625:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8626

8626:
  %8627 = phi i64 [ 0, %8625 ], [ %8647, %8626 ]
  %8628 = trunc i64 %8627 to i8
  %8629 = mul i8 %8628, 7
  %8630 = or disjoint i8 %8629, 3
  %8631 = getelementptr inbounds nuw i8, ptr @src, i64 %8627
  store i8 %8630, ptr %8631, align 4, !tbaa !10
  %8632 = or disjoint i64 %8627, 1
  %8633 = trunc i64 %8632 to i8
  %8634 = mul i8 %8633, 7
  %8635 = add i8 %8634, 3
  %8636 = getelementptr inbounds nuw i8, ptr @src, i64 %8632
  store i8 %8635, ptr %8636, align 1, !tbaa !10
  %8637 = or disjoint i64 %8627, 2
  %8638 = trunc i64 %8637 to i8
  %8639 = mul i8 %8638, 7
  %8640 = add i8 %8639, 3
  %8641 = getelementptr inbounds nuw i8, ptr @src, i64 %8637
  store i8 %8640, ptr %8641, align 2, !tbaa !10
  %8642 = or disjoint i64 %8627, 3
  %8643 = trunc i64 %8642 to i8
  %8644 = mul i8 %8643, 7
  %8645 = add i8 %8644, 3
  %8646 = getelementptr inbounds nuw i8, ptr @src, i64 %8642
  store i8 %8645, ptr %8646, align 1, !tbaa !10
  %8647 = add nuw nsw i64 %8627, 4
  %8648 = icmp eq i64 %8647, 1200
  br i1 %8648, label %8649, label %8626, !llvm.loop !11

8649:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr512()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 512)
  br label %8667

8650:
  %8651 = add nuw nsw i64 %8668, 1
  %8652 = getelementptr inbounds nuw i8, ptr @dst, i64 %8651
  %8653 = load i8, ptr %8652, align 1, !tbaa !10
  %8654 = getelementptr inbounds nuw i8, ptr @ref, i64 %8651
  %8655 = load i8, ptr %8654, align 1, !tbaa !10
  %8656 = icmp eq i8 %8653, %8655
  br i1 %8656, label %8657, label %8674

8657:
  %8658 = add nuw nsw i64 %8668, 2
  %8659 = getelementptr inbounds nuw i8, ptr @dst, i64 %8658
  %8660 = load i8, ptr %8659, align 1, !tbaa !10
  %8661 = getelementptr inbounds nuw i8, ptr @ref, i64 %8658
  %8662 = load i8, ptr %8661, align 1, !tbaa !10
  %8663 = icmp eq i8 %8660, %8662
  br i1 %8663, label %8664, label %8674

8664:
  %8665 = add nuw nsw i64 %8668, 3
  %8666 = icmp eq i64 %8665, 1200
  br i1 %8666, label %8675, label %8667, !llvm.loop !144

8667:
  %8668 = phi i64 [ 0, %8649 ], [ %8665, %8664 ]
  %8669 = getelementptr inbounds nuw i8, ptr @dst, i64 %8668
  %8670 = load i8, ptr %8669, align 1, !tbaa !10
  %8671 = getelementptr inbounds nuw i8, ptr @ref, i64 %8668
  %8672 = load i8, ptr %8671, align 1, !tbaa !10
  %8673 = icmp eq i8 %8670, %8672
  br i1 %8673, label %8650, label %8674

8674:
  tail call void @report(i64 noundef 512)
  store i1 true, ptr @bad, align 4
  br label %8676

8675:
  br label %8676

8676:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8677

8677:
  %8678 = phi i64 [ 0, %8676 ], [ %8698, %8677 ]
  %8679 = trunc i64 %8678 to i8
  %8680 = mul i8 %8679, 7
  %8681 = or disjoint i8 %8680, 3
  %8682 = getelementptr inbounds nuw i8, ptr @src, i64 %8678
  store i8 %8681, ptr %8682, align 4, !tbaa !10
  %8683 = or disjoint i64 %8678, 1
  %8684 = trunc i64 %8683 to i8
  %8685 = mul i8 %8684, 7
  %8686 = add i8 %8685, 3
  %8687 = getelementptr inbounds nuw i8, ptr @src, i64 %8683
  store i8 %8686, ptr %8687, align 1, !tbaa !10
  %8688 = or disjoint i64 %8678, 2
  %8689 = trunc i64 %8688 to i8
  %8690 = mul i8 %8689, 7
  %8691 = add i8 %8690, 3
  %8692 = getelementptr inbounds nuw i8, ptr @src, i64 %8688
  store i8 %8691, ptr %8692, align 2, !tbaa !10
  %8693 = or disjoint i64 %8678, 3
  %8694 = trunc i64 %8693 to i8
  %8695 = mul i8 %8694, 7
  %8696 = add i8 %8695, 3
  %8697 = getelementptr inbounds nuw i8, ptr @src, i64 %8693
  store i8 %8696, ptr %8697, align 1, !tbaa !10
  %8698 = add nuw nsw i64 %8678, 4
  %8699 = icmp eq i64 %8698, 1200
  br i1 %8699, label %8700, label %8677, !llvm.loop !11

8700:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7512()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 512)
  br label %8718

8701:
  %8702 = add nuw nsw i64 %8719, 1
  %8703 = getelementptr inbounds nuw i8, ptr @dst, i64 %8702
  %8704 = load i8, ptr %8703, align 1, !tbaa !10
  %8705 = getelementptr inbounds nuw i8, ptr @ref, i64 %8702
  %8706 = load i8, ptr %8705, align 1, !tbaa !10
  %8707 = icmp eq i8 %8704, %8706
  br i1 %8707, label %8708, label %8725

8708:
  %8709 = add nuw nsw i64 %8719, 2
  %8710 = getelementptr inbounds nuw i8, ptr @dst, i64 %8709
  %8711 = load i8, ptr %8710, align 1, !tbaa !10
  %8712 = getelementptr inbounds nuw i8, ptr @ref, i64 %8709
  %8713 = load i8, ptr %8712, align 1, !tbaa !10
  %8714 = icmp eq i8 %8711, %8713
  br i1 %8714, label %8715, label %8725

8715:
  %8716 = add nuw nsw i64 %8719, 3
  %8717 = icmp eq i64 %8716, 1200
  br i1 %8717, label %8726, label %8718, !llvm.loop !144

8718:
  %8719 = phi i64 [ 0, %8700 ], [ %8716, %8715 ]
  %8720 = getelementptr inbounds nuw i8, ptr @dst, i64 %8719
  %8721 = load i8, ptr %8720, align 1, !tbaa !10
  %8722 = getelementptr inbounds nuw i8, ptr @ref, i64 %8719
  %8723 = load i8, ptr %8722, align 1, !tbaa !10
  %8724 = icmp eq i8 %8721, %8723
  br i1 %8724, label %8701, label %8725

8725:
  tail call void @report(i64 noundef 512)
  store i1 true, ptr @bad, align 4
  br label %8727

8726:
  br label %8727

8727:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8728

8728:
  %8729 = phi i64 [ 0, %8727 ], [ %8749, %8728 ]
  %8730 = trunc i64 %8729 to i8
  %8731 = mul i8 %8730, 7
  %8732 = or disjoint i8 %8731, 3
  %8733 = getelementptr inbounds nuw i8, ptr @src, i64 %8729
  store i8 %8732, ptr %8733, align 4, !tbaa !10
  %8734 = or disjoint i64 %8729, 1
  %8735 = trunc i64 %8734 to i8
  %8736 = mul i8 %8735, 7
  %8737 = add i8 %8736, 3
  %8738 = getelementptr inbounds nuw i8, ptr @src, i64 %8734
  store i8 %8737, ptr %8738, align 1, !tbaa !10
  %8739 = or disjoint i64 %8729, 2
  %8740 = trunc i64 %8739 to i8
  %8741 = mul i8 %8740, 7
  %8742 = add i8 %8741, 3
  %8743 = getelementptr inbounds nuw i8, ptr @src, i64 %8739
  store i8 %8742, ptr %8743, align 2, !tbaa !10
  %8744 = or disjoint i64 %8729, 3
  %8745 = trunc i64 %8744 to i8
  %8746 = mul i8 %8745, 7
  %8747 = add i8 %8746, 3
  %8748 = getelementptr inbounds nuw i8, ptr @src, i64 %8744
  store i8 %8747, ptr %8748, align 1, !tbaa !10
  %8749 = add nuw nsw i64 %8729, 4
  %8750 = icmp eq i64 %8749, 1200
  br i1 %8750, label %8751, label %8728, !llvm.loop !11

8751:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr512()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 512)
  br label %8769

8752:
  %8753 = add nuw nsw i64 %8770, 1
  %8754 = getelementptr inbounds nuw i8, ptr @dst, i64 %8753
  %8755 = load i8, ptr %8754, align 1, !tbaa !10
  %8756 = getelementptr inbounds nuw i8, ptr @ref, i64 %8753
  %8757 = load i8, ptr %8756, align 1, !tbaa !10
  %8758 = icmp eq i8 %8755, %8757
  br i1 %8758, label %8759, label %8776

8759:
  %8760 = add nuw nsw i64 %8770, 2
  %8761 = getelementptr inbounds nuw i8, ptr @dst, i64 %8760
  %8762 = load i8, ptr %8761, align 1, !tbaa !10
  %8763 = getelementptr inbounds nuw i8, ptr @ref, i64 %8760
  %8764 = load i8, ptr %8763, align 1, !tbaa !10
  %8765 = icmp eq i8 %8762, %8764
  br i1 %8765, label %8766, label %8776

8766:
  %8767 = add nuw nsw i64 %8770, 3
  %8768 = icmp eq i64 %8767, 1200
  br i1 %8768, label %8777, label %8769, !llvm.loop !144

8769:
  %8770 = phi i64 [ 0, %8751 ], [ %8767, %8766 ]
  %8771 = getelementptr inbounds nuw i8, ptr @dst, i64 %8770
  %8772 = load i8, ptr %8771, align 1, !tbaa !10
  %8773 = getelementptr inbounds nuw i8, ptr @ref, i64 %8770
  %8774 = load i8, ptr %8773, align 1, !tbaa !10
  %8775 = icmp eq i8 %8772, %8774
  br i1 %8775, label %8752, label %8776

8776:
  tail call void @report(i64 noundef 512)
  store i1 true, ptr @bad, align 4
  br label %8778

8777:
  br label %8778

8778:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8779

8779:
  %8780 = phi i64 [ 0, %8778 ], [ %8800, %8779 ]
  %8781 = trunc i64 %8780 to i8
  %8782 = mul i8 %8781, 7
  %8783 = or disjoint i8 %8782, 3
  %8784 = getelementptr inbounds nuw i8, ptr @src, i64 %8780
  store i8 %8783, ptr %8784, align 4, !tbaa !10
  %8785 = or disjoint i64 %8780, 1
  %8786 = trunc i64 %8785 to i8
  %8787 = mul i8 %8786, 7
  %8788 = add i8 %8787, 3
  %8789 = getelementptr inbounds nuw i8, ptr @src, i64 %8785
  store i8 %8788, ptr %8789, align 1, !tbaa !10
  %8790 = or disjoint i64 %8780, 2
  %8791 = trunc i64 %8790 to i8
  %8792 = mul i8 %8791, 7
  %8793 = add i8 %8792, 3
  %8794 = getelementptr inbounds nuw i8, ptr @src, i64 %8790
  store i8 %8793, ptr %8794, align 2, !tbaa !10
  %8795 = or disjoint i64 %8780, 3
  %8796 = trunc i64 %8795 to i8
  %8797 = mul i8 %8796, 7
  %8798 = add i8 %8797, 3
  %8799 = getelementptr inbounds nuw i8, ptr @src, i64 %8795
  store i8 %8798, ptr %8799, align 1, !tbaa !10
  %8800 = add nuw nsw i64 %8780, 4
  %8801 = icmp eq i64 %8800, 1200
  br i1 %8801, label %8802, label %8779, !llvm.loop !11

8802:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv512(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 512)
  br label %8820

8803:
  %8804 = add nuw nsw i64 %8821, 1
  %8805 = getelementptr inbounds nuw i8, ptr @dst, i64 %8804
  %8806 = load i8, ptr %8805, align 1, !tbaa !10
  %8807 = getelementptr inbounds nuw i8, ptr @ref, i64 %8804
  %8808 = load i8, ptr %8807, align 1, !tbaa !10
  %8809 = icmp eq i8 %8806, %8808
  br i1 %8809, label %8810, label %8827

8810:
  %8811 = add nuw nsw i64 %8821, 2
  %8812 = getelementptr inbounds nuw i8, ptr @dst, i64 %8811
  %8813 = load i8, ptr %8812, align 1, !tbaa !10
  %8814 = getelementptr inbounds nuw i8, ptr @ref, i64 %8811
  %8815 = load i8, ptr %8814, align 1, !tbaa !10
  %8816 = icmp eq i8 %8813, %8815
  br i1 %8816, label %8817, label %8827

8817:
  %8818 = add nuw nsw i64 %8821, 3
  %8819 = icmp eq i64 %8818, 1200
  br i1 %8819, label %8828, label %8820, !llvm.loop !144

8820:
  %8821 = phi i64 [ 0, %8802 ], [ %8818, %8817 ]
  %8822 = getelementptr inbounds nuw i8, ptr @dst, i64 %8821
  %8823 = load i8, ptr %8822, align 1, !tbaa !10
  %8824 = getelementptr inbounds nuw i8, ptr @ref, i64 %8821
  %8825 = load i8, ptr %8824, align 1, !tbaa !10
  %8826 = icmp eq i8 %8823, %8825
  br i1 %8826, label %8803, label %8827

8827:
  tail call void @report(i64 noundef 512)
  store i1 true, ptr @bad, align 4
  br label %8829

8828:
  br label %8829

8829:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8830

8830:
  %8831 = phi i64 [ 0, %8829 ], [ %8851, %8830 ]
  %8832 = trunc i64 %8831 to i8
  %8833 = mul i8 %8832, 7
  %8834 = or disjoint i8 %8833, 3
  %8835 = getelementptr inbounds nuw i8, ptr @src, i64 %8831
  store i8 %8834, ptr %8835, align 4, !tbaa !10
  %8836 = or disjoint i64 %8831, 1
  %8837 = trunc i64 %8836 to i8
  %8838 = mul i8 %8837, 7
  %8839 = add i8 %8838, 3
  %8840 = getelementptr inbounds nuw i8, ptr @src, i64 %8836
  store i8 %8839, ptr %8840, align 1, !tbaa !10
  %8841 = or disjoint i64 %8831, 2
  %8842 = trunc i64 %8841 to i8
  %8843 = mul i8 %8842, 7
  %8844 = add i8 %8843, 3
  %8845 = getelementptr inbounds nuw i8, ptr @src, i64 %8841
  store i8 %8844, ptr %8845, align 2, !tbaa !10
  %8846 = or disjoint i64 %8831, 3
  %8847 = trunc i64 %8846 to i8
  %8848 = mul i8 %8847, 7
  %8849 = add i8 %8848, 3
  %8850 = getelementptr inbounds nuw i8, ptr @src, i64 %8846
  store i8 %8849, ptr %8850, align 1, !tbaa !10
  %8851 = add nuw nsw i64 %8831, 4
  %8852 = icmp eq i64 %8851, 1200
  br i1 %8852, label %8853, label %8830, !llvm.loop !11

8853:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv512(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 512)
  br label %8871

8854:
  %8855 = add nuw nsw i64 %8872, 1
  %8856 = getelementptr inbounds nuw i8, ptr @dst, i64 %8855
  %8857 = load i8, ptr %8856, align 1, !tbaa !10
  %8858 = getelementptr inbounds nuw i8, ptr @ref, i64 %8855
  %8859 = load i8, ptr %8858, align 1, !tbaa !10
  %8860 = icmp eq i8 %8857, %8859
  br i1 %8860, label %8861, label %8878

8861:
  %8862 = add nuw nsw i64 %8872, 2
  %8863 = getelementptr inbounds nuw i8, ptr @dst, i64 %8862
  %8864 = load i8, ptr %8863, align 1, !tbaa !10
  %8865 = getelementptr inbounds nuw i8, ptr @ref, i64 %8862
  %8866 = load i8, ptr %8865, align 1, !tbaa !10
  %8867 = icmp eq i8 %8864, %8866
  br i1 %8867, label %8868, label %8878

8868:
  %8869 = add nuw nsw i64 %8872, 3
  %8870 = icmp eq i64 %8869, 1200
  br i1 %8870, label %8879, label %8871, !llvm.loop !144

8871:
  %8872 = phi i64 [ 0, %8853 ], [ %8869, %8868 ]
  %8873 = getelementptr inbounds nuw i8, ptr @dst, i64 %8872
  %8874 = load i8, ptr %8873, align 1, !tbaa !10
  %8875 = getelementptr inbounds nuw i8, ptr @ref, i64 %8872
  %8876 = load i8, ptr %8875, align 1, !tbaa !10
  %8877 = icmp eq i8 %8874, %8876
  br i1 %8877, label %8854, label %8878

8878:
  tail call void @report(i64 noundef 512)
  store i1 true, ptr @bad, align 4
  br label %8880

8879:
  br label %8880

8880:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8881

8881:
  %8882 = phi i64 [ 0, %8880 ], [ %8902, %8881 ]
  %8883 = trunc i64 %8882 to i8
  %8884 = mul i8 %8883, 7
  %8885 = or disjoint i8 %8884, 3
  %8886 = getelementptr inbounds nuw i8, ptr @src, i64 %8882
  store i8 %8885, ptr %8886, align 4, !tbaa !10
  %8887 = or disjoint i64 %8882, 1
  %8888 = trunc i64 %8887 to i8
  %8889 = mul i8 %8888, 7
  %8890 = add i8 %8889, 3
  %8891 = getelementptr inbounds nuw i8, ptr @src, i64 %8887
  store i8 %8890, ptr %8891, align 1, !tbaa !10
  %8892 = or disjoint i64 %8882, 2
  %8893 = trunc i64 %8892 to i8
  %8894 = mul i8 %8893, 7
  %8895 = add i8 %8894, 3
  %8896 = getelementptr inbounds nuw i8, ptr @src, i64 %8892
  store i8 %8895, ptr %8896, align 2, !tbaa !10
  %8897 = or disjoint i64 %8882, 3
  %8898 = trunc i64 %8897 to i8
  %8899 = mul i8 %8898, 7
  %8900 = add i8 %8899, 3
  %8901 = getelementptr inbounds nuw i8, ptr @src, i64 %8897
  store i8 %8900, ptr %8901, align 1, !tbaa !10
  %8902 = add nuw nsw i64 %8882, 4
  %8903 = icmp eq i64 %8902, 1200
  br i1 %8903, label %8904, label %8881, !llvm.loop !11

8904:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp513()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 513)
  br label %8922

8905:
  %8906 = add nuw nsw i64 %8923, 1
  %8907 = getelementptr inbounds nuw i8, ptr @dst, i64 %8906
  %8908 = load i8, ptr %8907, align 1, !tbaa !10
  %8909 = getelementptr inbounds nuw i8, ptr @ref, i64 %8906
  %8910 = load i8, ptr %8909, align 1, !tbaa !10
  %8911 = icmp eq i8 %8908, %8910
  br i1 %8911, label %8912, label %8929

8912:
  %8913 = add nuw nsw i64 %8923, 2
  %8914 = getelementptr inbounds nuw i8, ptr @dst, i64 %8913
  %8915 = load i8, ptr %8914, align 1, !tbaa !10
  %8916 = getelementptr inbounds nuw i8, ptr @ref, i64 %8913
  %8917 = load i8, ptr %8916, align 1, !tbaa !10
  %8918 = icmp eq i8 %8915, %8917
  br i1 %8918, label %8919, label %8929

8919:
  %8920 = add nuw nsw i64 %8923, 3
  %8921 = icmp eq i64 %8920, 1200
  br i1 %8921, label %8930, label %8922, !llvm.loop !144

8922:
  %8923 = phi i64 [ 0, %8904 ], [ %8920, %8919 ]
  %8924 = getelementptr inbounds nuw i8, ptr @dst, i64 %8923
  %8925 = load i8, ptr %8924, align 1, !tbaa !10
  %8926 = getelementptr inbounds nuw i8, ptr @ref, i64 %8923
  %8927 = load i8, ptr %8926, align 1, !tbaa !10
  %8928 = icmp eq i8 %8925, %8927
  br i1 %8928, label %8905, label %8929

8929:
  tail call void @report(i64 noundef 513)
  store i1 true, ptr @bad, align 4
  br label %8931

8930:
  br label %8931

8931:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8932

8932:
  %8933 = phi i64 [ 0, %8931 ], [ %8953, %8932 ]
  %8934 = trunc i64 %8933 to i8
  %8935 = mul i8 %8934, 7
  %8936 = or disjoint i8 %8935, 3
  %8937 = getelementptr inbounds nuw i8, ptr @src, i64 %8933
  store i8 %8936, ptr %8937, align 4, !tbaa !10
  %8938 = or disjoint i64 %8933, 1
  %8939 = trunc i64 %8938 to i8
  %8940 = mul i8 %8939, 7
  %8941 = add i8 %8940, 3
  %8942 = getelementptr inbounds nuw i8, ptr @src, i64 %8938
  store i8 %8941, ptr %8942, align 1, !tbaa !10
  %8943 = or disjoint i64 %8933, 2
  %8944 = trunc i64 %8943 to i8
  %8945 = mul i8 %8944, 7
  %8946 = add i8 %8945, 3
  %8947 = getelementptr inbounds nuw i8, ptr @src, i64 %8943
  store i8 %8946, ptr %8947, align 2, !tbaa !10
  %8948 = or disjoint i64 %8933, 3
  %8949 = trunc i64 %8948 to i8
  %8950 = mul i8 %8949, 7
  %8951 = add i8 %8950, 3
  %8952 = getelementptr inbounds nuw i8, ptr @src, i64 %8948
  store i8 %8951, ptr %8952, align 1, !tbaa !10
  %8953 = add nuw nsw i64 %8933, 4
  %8954 = icmp eq i64 %8953, 1200
  br i1 %8954, label %8955, label %8932, !llvm.loop !11

8955:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr513()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 513)
  br label %8973

8956:
  %8957 = add nuw nsw i64 %8974, 1
  %8958 = getelementptr inbounds nuw i8, ptr @dst, i64 %8957
  %8959 = load i8, ptr %8958, align 1, !tbaa !10
  %8960 = getelementptr inbounds nuw i8, ptr @ref, i64 %8957
  %8961 = load i8, ptr %8960, align 1, !tbaa !10
  %8962 = icmp eq i8 %8959, %8961
  br i1 %8962, label %8963, label %8980

8963:
  %8964 = add nuw nsw i64 %8974, 2
  %8965 = getelementptr inbounds nuw i8, ptr @dst, i64 %8964
  %8966 = load i8, ptr %8965, align 1, !tbaa !10
  %8967 = getelementptr inbounds nuw i8, ptr @ref, i64 %8964
  %8968 = load i8, ptr %8967, align 1, !tbaa !10
  %8969 = icmp eq i8 %8966, %8968
  br i1 %8969, label %8970, label %8980

8970:
  %8971 = add nuw nsw i64 %8974, 3
  %8972 = icmp eq i64 %8971, 1200
  br i1 %8972, label %8981, label %8973, !llvm.loop !144

8973:
  %8974 = phi i64 [ 0, %8955 ], [ %8971, %8970 ]
  %8975 = getelementptr inbounds nuw i8, ptr @dst, i64 %8974
  %8976 = load i8, ptr %8975, align 1, !tbaa !10
  %8977 = getelementptr inbounds nuw i8, ptr @ref, i64 %8974
  %8978 = load i8, ptr %8977, align 1, !tbaa !10
  %8979 = icmp eq i8 %8976, %8978
  br i1 %8979, label %8956, label %8980

8980:
  tail call void @report(i64 noundef 513)
  store i1 true, ptr @bad, align 4
  br label %8982

8981:
  br label %8982

8982:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %8983

8983:
  %8984 = phi i64 [ 0, %8982 ], [ %9004, %8983 ]
  %8985 = trunc i64 %8984 to i8
  %8986 = mul i8 %8985, 7
  %8987 = or disjoint i8 %8986, 3
  %8988 = getelementptr inbounds nuw i8, ptr @src, i64 %8984
  store i8 %8987, ptr %8988, align 4, !tbaa !10
  %8989 = or disjoint i64 %8984, 1
  %8990 = trunc i64 %8989 to i8
  %8991 = mul i8 %8990, 7
  %8992 = add i8 %8991, 3
  %8993 = getelementptr inbounds nuw i8, ptr @src, i64 %8989
  store i8 %8992, ptr %8993, align 1, !tbaa !10
  %8994 = or disjoint i64 %8984, 2
  %8995 = trunc i64 %8994 to i8
  %8996 = mul i8 %8995, 7
  %8997 = add i8 %8996, 3
  %8998 = getelementptr inbounds nuw i8, ptr @src, i64 %8994
  store i8 %8997, ptr %8998, align 2, !tbaa !10
  %8999 = or disjoint i64 %8984, 3
  %9000 = trunc i64 %8999 to i8
  %9001 = mul i8 %9000, 7
  %9002 = add i8 %9001, 3
  %9003 = getelementptr inbounds nuw i8, ptr @src, i64 %8999
  store i8 %9002, ptr %9003, align 1, !tbaa !10
  %9004 = add nuw nsw i64 %8984, 4
  %9005 = icmp eq i64 %9004, 1200
  br i1 %9005, label %9006, label %8983, !llvm.loop !11

9006:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7513()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 513)
  br label %9024

9007:
  %9008 = add nuw nsw i64 %9025, 1
  %9009 = getelementptr inbounds nuw i8, ptr @dst, i64 %9008
  %9010 = load i8, ptr %9009, align 1, !tbaa !10
  %9011 = getelementptr inbounds nuw i8, ptr @ref, i64 %9008
  %9012 = load i8, ptr %9011, align 1, !tbaa !10
  %9013 = icmp eq i8 %9010, %9012
  br i1 %9013, label %9014, label %9031

9014:
  %9015 = add nuw nsw i64 %9025, 2
  %9016 = getelementptr inbounds nuw i8, ptr @dst, i64 %9015
  %9017 = load i8, ptr %9016, align 1, !tbaa !10
  %9018 = getelementptr inbounds nuw i8, ptr @ref, i64 %9015
  %9019 = load i8, ptr %9018, align 1, !tbaa !10
  %9020 = icmp eq i8 %9017, %9019
  br i1 %9020, label %9021, label %9031

9021:
  %9022 = add nuw nsw i64 %9025, 3
  %9023 = icmp eq i64 %9022, 1200
  br i1 %9023, label %9032, label %9024, !llvm.loop !144

9024:
  %9025 = phi i64 [ 0, %9006 ], [ %9022, %9021 ]
  %9026 = getelementptr inbounds nuw i8, ptr @dst, i64 %9025
  %9027 = load i8, ptr %9026, align 1, !tbaa !10
  %9028 = getelementptr inbounds nuw i8, ptr @ref, i64 %9025
  %9029 = load i8, ptr %9028, align 1, !tbaa !10
  %9030 = icmp eq i8 %9027, %9029
  br i1 %9030, label %9007, label %9031

9031:
  tail call void @report(i64 noundef 513)
  store i1 true, ptr @bad, align 4
  br label %9033

9032:
  br label %9033

9033:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %9034

9034:
  %9035 = phi i64 [ 0, %9033 ], [ %9055, %9034 ]
  %9036 = trunc i64 %9035 to i8
  %9037 = mul i8 %9036, 7
  %9038 = or disjoint i8 %9037, 3
  %9039 = getelementptr inbounds nuw i8, ptr @src, i64 %9035
  store i8 %9038, ptr %9039, align 4, !tbaa !10
  %9040 = or disjoint i64 %9035, 1
  %9041 = trunc i64 %9040 to i8
  %9042 = mul i8 %9041, 7
  %9043 = add i8 %9042, 3
  %9044 = getelementptr inbounds nuw i8, ptr @src, i64 %9040
  store i8 %9043, ptr %9044, align 1, !tbaa !10
  %9045 = or disjoint i64 %9035, 2
  %9046 = trunc i64 %9045 to i8
  %9047 = mul i8 %9046, 7
  %9048 = add i8 %9047, 3
  %9049 = getelementptr inbounds nuw i8, ptr @src, i64 %9045
  store i8 %9048, ptr %9049, align 2, !tbaa !10
  %9050 = or disjoint i64 %9035, 3
  %9051 = trunc i64 %9050 to i8
  %9052 = mul i8 %9051, 7
  %9053 = add i8 %9052, 3
  %9054 = getelementptr inbounds nuw i8, ptr @src, i64 %9050
  store i8 %9053, ptr %9054, align 1, !tbaa !10
  %9055 = add nuw nsw i64 %9035, 4
  %9056 = icmp eq i64 %9055, 1200
  br i1 %9056, label %9057, label %9034, !llvm.loop !11

9057:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr513()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 513)
  br label %9075

9058:
  %9059 = add nuw nsw i64 %9076, 1
  %9060 = getelementptr inbounds nuw i8, ptr @dst, i64 %9059
  %9061 = load i8, ptr %9060, align 1, !tbaa !10
  %9062 = getelementptr inbounds nuw i8, ptr @ref, i64 %9059
  %9063 = load i8, ptr %9062, align 1, !tbaa !10
  %9064 = icmp eq i8 %9061, %9063
  br i1 %9064, label %9065, label %9082

9065:
  %9066 = add nuw nsw i64 %9076, 2
  %9067 = getelementptr inbounds nuw i8, ptr @dst, i64 %9066
  %9068 = load i8, ptr %9067, align 1, !tbaa !10
  %9069 = getelementptr inbounds nuw i8, ptr @ref, i64 %9066
  %9070 = load i8, ptr %9069, align 1, !tbaa !10
  %9071 = icmp eq i8 %9068, %9070
  br i1 %9071, label %9072, label %9082

9072:
  %9073 = add nuw nsw i64 %9076, 3
  %9074 = icmp eq i64 %9073, 1200
  br i1 %9074, label %9083, label %9075, !llvm.loop !144

9075:
  %9076 = phi i64 [ 0, %9057 ], [ %9073, %9072 ]
  %9077 = getelementptr inbounds nuw i8, ptr @dst, i64 %9076
  %9078 = load i8, ptr %9077, align 1, !tbaa !10
  %9079 = getelementptr inbounds nuw i8, ptr @ref, i64 %9076
  %9080 = load i8, ptr %9079, align 1, !tbaa !10
  %9081 = icmp eq i8 %9078, %9080
  br i1 %9081, label %9058, label %9082

9082:
  tail call void @report(i64 noundef 513)
  store i1 true, ptr @bad, align 4
  br label %9084

9083:
  br label %9084

9084:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %9085

9085:
  %9086 = phi i64 [ 0, %9084 ], [ %9106, %9085 ]
  %9087 = trunc i64 %9086 to i8
  %9088 = mul i8 %9087, 7
  %9089 = or disjoint i8 %9088, 3
  %9090 = getelementptr inbounds nuw i8, ptr @src, i64 %9086
  store i8 %9089, ptr %9090, align 4, !tbaa !10
  %9091 = or disjoint i64 %9086, 1
  %9092 = trunc i64 %9091 to i8
  %9093 = mul i8 %9092, 7
  %9094 = add i8 %9093, 3
  %9095 = getelementptr inbounds nuw i8, ptr @src, i64 %9091
  store i8 %9094, ptr %9095, align 1, !tbaa !10
  %9096 = or disjoint i64 %9086, 2
  %9097 = trunc i64 %9096 to i8
  %9098 = mul i8 %9097, 7
  %9099 = add i8 %9098, 3
  %9100 = getelementptr inbounds nuw i8, ptr @src, i64 %9096
  store i8 %9099, ptr %9100, align 2, !tbaa !10
  %9101 = or disjoint i64 %9086, 3
  %9102 = trunc i64 %9101 to i8
  %9103 = mul i8 %9102, 7
  %9104 = add i8 %9103, 3
  %9105 = getelementptr inbounds nuw i8, ptr @src, i64 %9101
  store i8 %9104, ptr %9105, align 1, !tbaa !10
  %9106 = add nuw nsw i64 %9086, 4
  %9107 = icmp eq i64 %9106, 1200
  br i1 %9107, label %9108, label %9085, !llvm.loop !11

9108:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv513(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 513)
  br label %9126

9109:
  %9110 = add nuw nsw i64 %9127, 1
  %9111 = getelementptr inbounds nuw i8, ptr @dst, i64 %9110
  %9112 = load i8, ptr %9111, align 1, !tbaa !10
  %9113 = getelementptr inbounds nuw i8, ptr @ref, i64 %9110
  %9114 = load i8, ptr %9113, align 1, !tbaa !10
  %9115 = icmp eq i8 %9112, %9114
  br i1 %9115, label %9116, label %9133

9116:
  %9117 = add nuw nsw i64 %9127, 2
  %9118 = getelementptr inbounds nuw i8, ptr @dst, i64 %9117
  %9119 = load i8, ptr %9118, align 1, !tbaa !10
  %9120 = getelementptr inbounds nuw i8, ptr @ref, i64 %9117
  %9121 = load i8, ptr %9120, align 1, !tbaa !10
  %9122 = icmp eq i8 %9119, %9121
  br i1 %9122, label %9123, label %9133

9123:
  %9124 = add nuw nsw i64 %9127, 3
  %9125 = icmp eq i64 %9124, 1200
  br i1 %9125, label %9134, label %9126, !llvm.loop !144

9126:
  %9127 = phi i64 [ 0, %9108 ], [ %9124, %9123 ]
  %9128 = getelementptr inbounds nuw i8, ptr @dst, i64 %9127
  %9129 = load i8, ptr %9128, align 1, !tbaa !10
  %9130 = getelementptr inbounds nuw i8, ptr @ref, i64 %9127
  %9131 = load i8, ptr %9130, align 1, !tbaa !10
  %9132 = icmp eq i8 %9129, %9131
  br i1 %9132, label %9109, label %9133

9133:
  tail call void @report(i64 noundef 513)
  store i1 true, ptr @bad, align 4
  br label %9135

9134:
  br label %9135

9135:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %9136

9136:
  %9137 = phi i64 [ 0, %9135 ], [ %9157, %9136 ]
  %9138 = trunc i64 %9137 to i8
  %9139 = mul i8 %9138, 7
  %9140 = or disjoint i8 %9139, 3
  %9141 = getelementptr inbounds nuw i8, ptr @src, i64 %9137
  store i8 %9140, ptr %9141, align 4, !tbaa !10
  %9142 = or disjoint i64 %9137, 1
  %9143 = trunc i64 %9142 to i8
  %9144 = mul i8 %9143, 7
  %9145 = add i8 %9144, 3
  %9146 = getelementptr inbounds nuw i8, ptr @src, i64 %9142
  store i8 %9145, ptr %9146, align 1, !tbaa !10
  %9147 = or disjoint i64 %9137, 2
  %9148 = trunc i64 %9147 to i8
  %9149 = mul i8 %9148, 7
  %9150 = add i8 %9149, 3
  %9151 = getelementptr inbounds nuw i8, ptr @src, i64 %9147
  store i8 %9150, ptr %9151, align 2, !tbaa !10
  %9152 = or disjoint i64 %9137, 3
  %9153 = trunc i64 %9152 to i8
  %9154 = mul i8 %9153, 7
  %9155 = add i8 %9154, 3
  %9156 = getelementptr inbounds nuw i8, ptr @src, i64 %9152
  store i8 %9155, ptr %9156, align 1, !tbaa !10
  %9157 = add nuw nsw i64 %9137, 4
  %9158 = icmp eq i64 %9157, 1200
  br i1 %9158, label %9159, label %9136, !llvm.loop !11

9159:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv513(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 513)
  br label %9177

9160:
  %9161 = add nuw nsw i64 %9178, 1
  %9162 = getelementptr inbounds nuw i8, ptr @dst, i64 %9161
  %9163 = load i8, ptr %9162, align 1, !tbaa !10
  %9164 = getelementptr inbounds nuw i8, ptr @ref, i64 %9161
  %9165 = load i8, ptr %9164, align 1, !tbaa !10
  %9166 = icmp eq i8 %9163, %9165
  br i1 %9166, label %9167, label %9184

9167:
  %9168 = add nuw nsw i64 %9178, 2
  %9169 = getelementptr inbounds nuw i8, ptr @dst, i64 %9168
  %9170 = load i8, ptr %9169, align 1, !tbaa !10
  %9171 = getelementptr inbounds nuw i8, ptr @ref, i64 %9168
  %9172 = load i8, ptr %9171, align 1, !tbaa !10
  %9173 = icmp eq i8 %9170, %9172
  br i1 %9173, label %9174, label %9184

9174:
  %9175 = add nuw nsw i64 %9178, 3
  %9176 = icmp eq i64 %9175, 1200
  br i1 %9176, label %9185, label %9177, !llvm.loop !144

9177:
  %9178 = phi i64 [ 0, %9159 ], [ %9175, %9174 ]
  %9179 = getelementptr inbounds nuw i8, ptr @dst, i64 %9178
  %9180 = load i8, ptr %9179, align 1, !tbaa !10
  %9181 = getelementptr inbounds nuw i8, ptr @ref, i64 %9178
  %9182 = load i8, ptr %9181, align 1, !tbaa !10
  %9183 = icmp eq i8 %9180, %9182
  br i1 %9183, label %9160, label %9184

9184:
  tail call void @report(i64 noundef 513)
  store i1 true, ptr @bad, align 4
  br label %9186

9185:
  br label %9186

9186:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %9187

9187:
  %9188 = phi i64 [ 0, %9186 ], [ %9208, %9187 ]
  %9189 = trunc i64 %9188 to i8
  %9190 = mul i8 %9189, 7
  %9191 = or disjoint i8 %9190, 3
  %9192 = getelementptr inbounds nuw i8, ptr @src, i64 %9188
  store i8 %9191, ptr %9192, align 4, !tbaa !10
  %9193 = or disjoint i64 %9188, 1
  %9194 = trunc i64 %9193 to i8
  %9195 = mul i8 %9194, 7
  %9196 = add i8 %9195, 3
  %9197 = getelementptr inbounds nuw i8, ptr @src, i64 %9193
  store i8 %9196, ptr %9197, align 1, !tbaa !10
  %9198 = or disjoint i64 %9188, 2
  %9199 = trunc i64 %9198 to i8
  %9200 = mul i8 %9199, 7
  %9201 = add i8 %9200, 3
  %9202 = getelementptr inbounds nuw i8, ptr @src, i64 %9198
  store i8 %9201, ptr %9202, align 2, !tbaa !10
  %9203 = or disjoint i64 %9188, 3
  %9204 = trunc i64 %9203 to i8
  %9205 = mul i8 %9204, 7
  %9206 = add i8 %9205, 3
  %9207 = getelementptr inbounds nuw i8, ptr @src, i64 %9203
  store i8 %9206, ptr %9207, align 1, !tbaa !10
  %9208 = add nuw nsw i64 %9188, 4
  %9209 = icmp eq i64 %9208, 1200
  br i1 %9209, label %9210, label %9187, !llvm.loop !11

9210:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @cp600()
  tail call void @ref_copy(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 5), ptr noundef getelementptr inbounds nuw (i8, ptr @src, i64 1), i32 noundef 600)
  br label %9228

9211:
  %9212 = add nuw nsw i64 %9229, 1
  %9213 = getelementptr inbounds nuw i8, ptr @dst, i64 %9212
  %9214 = load i8, ptr %9213, align 1, !tbaa !10
  %9215 = getelementptr inbounds nuw i8, ptr @ref, i64 %9212
  %9216 = load i8, ptr %9215, align 1, !tbaa !10
  %9217 = icmp eq i8 %9214, %9216
  br i1 %9217, label %9218, label %9235

9218:
  %9219 = add nuw nsw i64 %9229, 2
  %9220 = getelementptr inbounds nuw i8, ptr @dst, i64 %9219
  %9221 = load i8, ptr %9220, align 1, !tbaa !10
  %9222 = getelementptr inbounds nuw i8, ptr @ref, i64 %9219
  %9223 = load i8, ptr %9222, align 1, !tbaa !10
  %9224 = icmp eq i8 %9221, %9223
  br i1 %9224, label %9225, label %9235

9225:
  %9226 = add nuw nsw i64 %9229, 3
  %9227 = icmp eq i64 %9226, 1200
  br i1 %9227, label %9236, label %9228, !llvm.loop !144

9228:
  %9229 = phi i64 [ 0, %9210 ], [ %9226, %9225 ]
  %9230 = getelementptr inbounds nuw i8, ptr @dst, i64 %9229
  %9231 = load i8, ptr %9230, align 1, !tbaa !10
  %9232 = getelementptr inbounds nuw i8, ptr @ref, i64 %9229
  %9233 = load i8, ptr %9232, align 1, !tbaa !10
  %9234 = icmp eq i8 %9231, %9233
  br i1 %9234, label %9211, label %9235

9235:
  tail call void @report(i64 noundef 600)
  store i1 true, ptr @bad, align 4
  br label %9237

9236:
  br label %9237

9237:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %9238

9238:
  %9239 = phi i64 [ 0, %9237 ], [ %9259, %9238 ]
  %9240 = trunc i64 %9239 to i8
  %9241 = mul i8 %9240, 7
  %9242 = or disjoint i8 %9241, 3
  %9243 = getelementptr inbounds nuw i8, ptr @src, i64 %9239
  store i8 %9242, ptr %9243, align 4, !tbaa !10
  %9244 = or disjoint i64 %9239, 1
  %9245 = trunc i64 %9244 to i8
  %9246 = mul i8 %9245, 7
  %9247 = add i8 %9246, 3
  %9248 = getelementptr inbounds nuw i8, ptr @src, i64 %9244
  store i8 %9247, ptr %9248, align 1, !tbaa !10
  %9249 = or disjoint i64 %9239, 2
  %9250 = trunc i64 %9249 to i8
  %9251 = mul i8 %9250, 7
  %9252 = add i8 %9251, 3
  %9253 = getelementptr inbounds nuw i8, ptr @src, i64 %9249
  store i8 %9252, ptr %9253, align 2, !tbaa !10
  %9254 = or disjoint i64 %9239, 3
  %9255 = trunc i64 %9254 to i8
  %9256 = mul i8 %9255, 7
  %9257 = add i8 %9256, 3
  %9258 = getelementptr inbounds nuw i8, ptr @src, i64 %9254
  store i8 %9257, ptr %9258, align 1, !tbaa !10
  %9259 = add nuw nsw i64 %9239, 4
  %9260 = icmp eq i64 %9259, 1200
  br i1 %9260, label %9261, label %9238, !llvm.loop !11

9261:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @zr600()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 0, i32 noundef 600)
  br label %9279

9262:
  %9263 = add nuw nsw i64 %9280, 1
  %9264 = getelementptr inbounds nuw i8, ptr @dst, i64 %9263
  %9265 = load i8, ptr %9264, align 1, !tbaa !10
  %9266 = getelementptr inbounds nuw i8, ptr @ref, i64 %9263
  %9267 = load i8, ptr %9266, align 1, !tbaa !10
  %9268 = icmp eq i8 %9265, %9267
  br i1 %9268, label %9269, label %9286

9269:
  %9270 = add nuw nsw i64 %9280, 2
  %9271 = getelementptr inbounds nuw i8, ptr @dst, i64 %9270
  %9272 = load i8, ptr %9271, align 1, !tbaa !10
  %9273 = getelementptr inbounds nuw i8, ptr @ref, i64 %9270
  %9274 = load i8, ptr %9273, align 1, !tbaa !10
  %9275 = icmp eq i8 %9272, %9274
  br i1 %9275, label %9276, label %9286

9276:
  %9277 = add nuw nsw i64 %9280, 3
  %9278 = icmp eq i64 %9277, 1200
  br i1 %9278, label %9287, label %9279, !llvm.loop !144

9279:
  %9280 = phi i64 [ 0, %9261 ], [ %9277, %9276 ]
  %9281 = getelementptr inbounds nuw i8, ptr @dst, i64 %9280
  %9282 = load i8, ptr %9281, align 1, !tbaa !10
  %9283 = getelementptr inbounds nuw i8, ptr @ref, i64 %9280
  %9284 = load i8, ptr %9283, align 1, !tbaa !10
  %9285 = icmp eq i8 %9282, %9284
  br i1 %9285, label %9262, label %9286

9286:
  tail call void @report(i64 noundef 600)
  store i1 true, ptr @bad, align 4
  br label %9288

9287:
  br label %9288

9288:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %9289

9289:
  %9290 = phi i64 [ 0, %9288 ], [ %9310, %9289 ]
  %9291 = trunc i64 %9290 to i8
  %9292 = mul i8 %9291, 7
  %9293 = or disjoint i8 %9292, 3
  %9294 = getelementptr inbounds nuw i8, ptr @src, i64 %9290
  store i8 %9293, ptr %9294, align 4, !tbaa !10
  %9295 = or disjoint i64 %9290, 1
  %9296 = trunc i64 %9295 to i8
  %9297 = mul i8 %9296, 7
  %9298 = add i8 %9297, 3
  %9299 = getelementptr inbounds nuw i8, ptr @src, i64 %9295
  store i8 %9298, ptr %9299, align 1, !tbaa !10
  %9300 = or disjoint i64 %9290, 2
  %9301 = trunc i64 %9300 to i8
  %9302 = mul i8 %9301, 7
  %9303 = add i8 %9302, 3
  %9304 = getelementptr inbounds nuw i8, ptr @src, i64 %9300
  store i8 %9303, ptr %9304, align 2, !tbaa !10
  %9305 = or disjoint i64 %9290, 3
  %9306 = trunc i64 %9305 to i8
  %9307 = mul i8 %9306, 7
  %9308 = add i8 %9307, 3
  %9309 = getelementptr inbounds nuw i8, ptr @src, i64 %9305
  store i8 %9308, ptr %9309, align 1, !tbaa !10
  %9310 = add nuw nsw i64 %9290, 4
  %9311 = icmp eq i64 %9310, 1200
  br i1 %9311, label %9312, label %9289, !llvm.loop !11

9312:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @c7600()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 123, i32 noundef 600)
  br label %9330

9313:
  %9314 = add nuw nsw i64 %9331, 1
  %9315 = getelementptr inbounds nuw i8, ptr @dst, i64 %9314
  %9316 = load i8, ptr %9315, align 1, !tbaa !10
  %9317 = getelementptr inbounds nuw i8, ptr @ref, i64 %9314
  %9318 = load i8, ptr %9317, align 1, !tbaa !10
  %9319 = icmp eq i8 %9316, %9318
  br i1 %9319, label %9320, label %9337

9320:
  %9321 = add nuw nsw i64 %9331, 2
  %9322 = getelementptr inbounds nuw i8, ptr @dst, i64 %9321
  %9323 = load i8, ptr %9322, align 1, !tbaa !10
  %9324 = getelementptr inbounds nuw i8, ptr @ref, i64 %9321
  %9325 = load i8, ptr %9324, align 1, !tbaa !10
  %9326 = icmp eq i8 %9323, %9325
  br i1 %9326, label %9327, label %9337

9327:
  %9328 = add nuw nsw i64 %9331, 3
  %9329 = icmp eq i64 %9328, 1200
  br i1 %9329, label %9338, label %9330, !llvm.loop !144

9330:
  %9331 = phi i64 [ 0, %9312 ], [ %9328, %9327 ]
  %9332 = getelementptr inbounds nuw i8, ptr @dst, i64 %9331
  %9333 = load i8, ptr %9332, align 1, !tbaa !10
  %9334 = getelementptr inbounds nuw i8, ptr @ref, i64 %9331
  %9335 = load i8, ptr %9334, align 1, !tbaa !10
  %9336 = icmp eq i8 %9333, %9335
  br i1 %9336, label %9313, label %9337

9337:
  tail call void @report(i64 noundef 600)
  store i1 true, ptr @bad, align 4
  br label %9339

9338:
  br label %9339

9339:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %9340

9340:
  %9341 = phi i64 [ 0, %9339 ], [ %9361, %9340 ]
  %9342 = trunc i64 %9341 to i8
  %9343 = mul i8 %9342, 7
  %9344 = or disjoint i8 %9343, 3
  %9345 = getelementptr inbounds nuw i8, ptr @src, i64 %9341
  store i8 %9344, ptr %9345, align 4, !tbaa !10
  %9346 = or disjoint i64 %9341, 1
  %9347 = trunc i64 %9346 to i8
  %9348 = mul i8 %9347, 7
  %9349 = add i8 %9348, 3
  %9350 = getelementptr inbounds nuw i8, ptr @src, i64 %9346
  store i8 %9349, ptr %9350, align 1, !tbaa !10
  %9351 = or disjoint i64 %9341, 2
  %9352 = trunc i64 %9351 to i8
  %9353 = mul i8 %9352, 7
  %9354 = add i8 %9353, 3
  %9355 = getelementptr inbounds nuw i8, ptr @src, i64 %9351
  store i8 %9354, ptr %9355, align 2, !tbaa !10
  %9356 = or disjoint i64 %9341, 3
  %9357 = trunc i64 %9356 to i8
  %9358 = mul i8 %9357, 7
  %9359 = add i8 %9358, 3
  %9360 = getelementptr inbounds nuw i8, ptr @src, i64 %9356
  store i8 %9359, ptr %9360, align 1, !tbaa !10
  %9361 = add nuw nsw i64 %9341, 4
  %9362 = icmp eq i64 %9361, 1200
  br i1 %9362, label %9363, label %9340, !llvm.loop !11

9363:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @vr600()
  tail call void @ref_fill(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 3), i32 noundef 193, i32 noundef 600)
  br label %9381

9364:
  %9365 = add nuw nsw i64 %9382, 1
  %9366 = getelementptr inbounds nuw i8, ptr @dst, i64 %9365
  %9367 = load i8, ptr %9366, align 1, !tbaa !10
  %9368 = getelementptr inbounds nuw i8, ptr @ref, i64 %9365
  %9369 = load i8, ptr %9368, align 1, !tbaa !10
  %9370 = icmp eq i8 %9367, %9369
  br i1 %9370, label %9371, label %9388

9371:
  %9372 = add nuw nsw i64 %9382, 2
  %9373 = getelementptr inbounds nuw i8, ptr @dst, i64 %9372
  %9374 = load i8, ptr %9373, align 1, !tbaa !10
  %9375 = getelementptr inbounds nuw i8, ptr @ref, i64 %9372
  %9376 = load i8, ptr %9375, align 1, !tbaa !10
  %9377 = icmp eq i8 %9374, %9376
  br i1 %9377, label %9378, label %9388

9378:
  %9379 = add nuw nsw i64 %9382, 3
  %9380 = icmp eq i64 %9379, 1200
  br i1 %9380, label %9389, label %9381, !llvm.loop !144

9381:
  %9382 = phi i64 [ 0, %9363 ], [ %9379, %9378 ]
  %9383 = getelementptr inbounds nuw i8, ptr @dst, i64 %9382
  %9384 = load i8, ptr %9383, align 1, !tbaa !10
  %9385 = getelementptr inbounds nuw i8, ptr @ref, i64 %9382
  %9386 = load i8, ptr %9385, align 1, !tbaa !10
  %9387 = icmp eq i8 %9384, %9386
  br i1 %9387, label %9364, label %9388

9388:
  tail call void @report(i64 noundef 600)
  store i1 true, ptr @bad, align 4
  br label %9390

9389:
  br label %9390

9390:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %9391

9391:
  %9392 = phi i64 [ 0, %9390 ], [ %9412, %9391 ]
  %9393 = trunc i64 %9392 to i8
  %9394 = mul i8 %9393, 7
  %9395 = or disjoint i8 %9394, 3
  %9396 = getelementptr inbounds nuw i8, ptr @src, i64 %9392
  store i8 %9395, ptr %9396, align 4, !tbaa !10
  %9397 = or disjoint i64 %9392, 1
  %9398 = trunc i64 %9397 to i8
  %9399 = mul i8 %9398, 7
  %9400 = add i8 %9399, 3
  %9401 = getelementptr inbounds nuw i8, ptr @src, i64 %9397
  store i8 %9400, ptr %9401, align 1, !tbaa !10
  %9402 = or disjoint i64 %9392, 2
  %9403 = trunc i64 %9402 to i8
  %9404 = mul i8 %9403, 7
  %9405 = add i8 %9404, 3
  %9406 = getelementptr inbounds nuw i8, ptr @src, i64 %9402
  store i8 %9405, ptr %9406, align 2, !tbaa !10
  %9407 = or disjoint i64 %9392, 3
  %9408 = trunc i64 %9407 to i8
  %9409 = mul i8 %9408, 7
  %9410 = add i8 %9409, 3
  %9411 = getelementptr inbounds nuw i8, ptr @src, i64 %9407
  store i8 %9410, ptr %9411, align 1, !tbaa !10
  %9412 = add nuw nsw i64 %9392, 4
  %9413 = icmp eq i64 %9412, 1200
  br i1 %9413, label %9414, label %9391, !llvm.loop !11

9414:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv600(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), i32 noundef 600)
  br label %9432

9415:
  %9416 = add nuw nsw i64 %9433, 1
  %9417 = getelementptr inbounds nuw i8, ptr @dst, i64 %9416
  %9418 = load i8, ptr %9417, align 1, !tbaa !10
  %9419 = getelementptr inbounds nuw i8, ptr @ref, i64 %9416
  %9420 = load i8, ptr %9419, align 1, !tbaa !10
  %9421 = icmp eq i8 %9418, %9420
  br i1 %9421, label %9422, label %9439

9422:
  %9423 = add nuw nsw i64 %9433, 2
  %9424 = getelementptr inbounds nuw i8, ptr @dst, i64 %9423
  %9425 = load i8, ptr %9424, align 1, !tbaa !10
  %9426 = getelementptr inbounds nuw i8, ptr @ref, i64 %9423
  %9427 = load i8, ptr %9426, align 1, !tbaa !10
  %9428 = icmp eq i8 %9425, %9427
  br i1 %9428, label %9429, label %9439

9429:
  %9430 = add nuw nsw i64 %9433, 3
  %9431 = icmp eq i64 %9430, 1200
  br i1 %9431, label %9440, label %9432, !llvm.loop !144

9432:
  %9433 = phi i64 [ 0, %9414 ], [ %9430, %9429 ]
  %9434 = getelementptr inbounds nuw i8, ptr @dst, i64 %9433
  %9435 = load i8, ptr %9434, align 1, !tbaa !10
  %9436 = getelementptr inbounds nuw i8, ptr @ref, i64 %9433
  %9437 = load i8, ptr %9436, align 1, !tbaa !10
  %9438 = icmp eq i8 %9435, %9437
  br i1 %9438, label %9415, label %9439

9439:
  tail call void @report(i64 noundef 600)
  store i1 true, ptr @bad, align 4
  br label %9441

9440:
  br label %9441

9441:
  tail call void @llvm.memset.p0.i64(ptr align 16 @dst, i8 -18, i64 1200, i1 false)
  br label %9442

9442:
  %9443 = phi i64 [ 0, %9441 ], [ %9463, %9442 ]
  %9444 = trunc i64 %9443 to i8
  %9445 = mul i8 %9444, 7
  %9446 = or disjoint i8 %9445, 3
  %9447 = getelementptr inbounds nuw i8, ptr @src, i64 %9443
  store i8 %9446, ptr %9447, align 4, !tbaa !10
  %9448 = or disjoint i64 %9443, 1
  %9449 = trunc i64 %9448 to i8
  %9450 = mul i8 %9449, 7
  %9451 = add i8 %9450, 3
  %9452 = getelementptr inbounds nuw i8, ptr @src, i64 %9448
  store i8 %9451, ptr %9452, align 1, !tbaa !10
  %9453 = or disjoint i64 %9443, 2
  %9454 = trunc i64 %9453 to i8
  %9455 = mul i8 %9454, 7
  %9456 = add i8 %9455, 3
  %9457 = getelementptr inbounds nuw i8, ptr @src, i64 %9453
  store i8 %9456, ptr %9457, align 2, !tbaa !10
  %9458 = or disjoint i64 %9443, 3
  %9459 = trunc i64 %9458 to i8
  %9460 = mul i8 %9459, 7
  %9461 = add i8 %9460, 3
  %9462 = getelementptr inbounds nuw i8, ptr @src, i64 %9458
  store i8 %9461, ptr %9462, align 1, !tbaa !10
  %9463 = add nuw nsw i64 %9443, 4
  %9464 = icmp eq i64 %9463, 1200
  br i1 %9464, label %9465, label %9442, !llvm.loop !11

9465:
  tail call void @llvm.memset.p0.i64(ptr align 16 @ref, i8 -18, i64 1200, i1 false)
  tail call void @ref_copy(ptr noundef @dst, ptr noundef @src, i32 noundef 1200)
  tail call void @ref_copy(ptr noundef @ref, ptr noundef @src, i32 noundef 1200)
  tail call void @mv600(ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @dst, i64 4))
  tail call void @ref_move(ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 1), ptr noundef getelementptr inbounds nuw (i8, ptr @ref, i64 4), i32 noundef 600)
  br label %9483

9466:
  %9467 = add nuw nsw i64 %9484, 1
  %9468 = getelementptr inbounds nuw i8, ptr @dst, i64 %9467
  %9469 = load i8, ptr %9468, align 1, !tbaa !10
  %9470 = getelementptr inbounds nuw i8, ptr @ref, i64 %9467
  %9471 = load i8, ptr %9470, align 1, !tbaa !10
  %9472 = icmp eq i8 %9469, %9471
  br i1 %9472, label %9473, label %9490

9473:
  %9474 = add nuw nsw i64 %9484, 2
  %9475 = getelementptr inbounds nuw i8, ptr @dst, i64 %9474
  %9476 = load i8, ptr %9475, align 1, !tbaa !10
  %9477 = getelementptr inbounds nuw i8, ptr @ref, i64 %9474
  %9478 = load i8, ptr %9477, align 1, !tbaa !10
  %9479 = icmp eq i8 %9476, %9478
  br i1 %9479, label %9480, label %9490

9480:
  %9481 = add nuw nsw i64 %9484, 3
  %9482 = icmp eq i64 %9481, 1200
  br i1 %9482, label %9491, label %9483, !llvm.loop !144

9483:
  %9484 = phi i64 [ 0, %9465 ], [ %9481, %9480 ]
  %9485 = getelementptr inbounds nuw i8, ptr @dst, i64 %9484
  %9486 = load i8, ptr %9485, align 1, !tbaa !10
  %9487 = getelementptr inbounds nuw i8, ptr @ref, i64 %9484
  %9488 = load i8, ptr %9487, align 1, !tbaa !10
  %9489 = icmp eq i8 %9486, %9488
  br i1 %9489, label %9466, label %9490

9490:
  tail call void @report(i64 noundef 600)
  store i1 true, ptr @bad, align 4
  br label %9492

9491:
  br label %9492

9492:
  tail call void @lc1()
  tail call void @lr1()
  br label %9503

9493:
  %9494 = or disjoint i64 %9504, 1
  %9495 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9494
  %9496 = load i8, ptr %9495, align 1, !tbaa !10
  %9497 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9494
  %9498 = load i8, ptr %9497, align 1, !tbaa !10
  %9499 = icmp eq i8 %9496, %9498
  br i1 %9499, label %9500, label %9510

9500:
  %9501 = add nuw nsw i64 %9504, 2
  %9502 = icmp eq i64 %9501, 700
  br i1 %9502, label %9511, label %9503, !llvm.loop !144

9503:
  %9504 = phi i64 [ 0, %9492 ], [ %9501, %9500 ]
  %9505 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9504
  %9506 = load i8, ptr %9505, align 2, !tbaa !10
  %9507 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9504
  %9508 = load i8, ptr %9507, align 2, !tbaa !10
  %9509 = icmp eq i8 %9506, %9508
  br i1 %9509, label %9493, label %9510

9510:
  tail call void @report(i64 noundef -1)
  store i1 true, ptr @bad, align 4
  br label %9512

9511:
  br label %9512

9512:
  tail call void @lc2()
  tail call void @lr2()
  br label %9523

9513:
  %9514 = or disjoint i64 %9524, 1
  %9515 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9514
  %9516 = load i8, ptr %9515, align 1, !tbaa !10
  %9517 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9514
  %9518 = load i8, ptr %9517, align 1, !tbaa !10
  %9519 = icmp eq i8 %9516, %9518
  br i1 %9519, label %9520, label %9530

9520:
  %9521 = add nuw nsw i64 %9524, 2
  %9522 = icmp eq i64 %9521, 700
  br i1 %9522, label %9531, label %9523, !llvm.loop !144

9523:
  %9524 = phi i64 [ 0, %9512 ], [ %9521, %9520 ]
  %9525 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9524
  %9526 = load i8, ptr %9525, align 2, !tbaa !10
  %9527 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9524
  %9528 = load i8, ptr %9527, align 2, !tbaa !10
  %9529 = icmp eq i8 %9526, %9528
  br i1 %9529, label %9513, label %9530

9530:
  tail call void @report(i64 noundef -2)
  store i1 true, ptr @bad, align 4
  br label %9532

9531:
  br label %9532

9532:
  tail call void @lc3()
  tail call void @lr3()
  br label %9543

9533:
  %9534 = or disjoint i64 %9544, 1
  %9535 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9534
  %9536 = load i8, ptr %9535, align 1, !tbaa !10
  %9537 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9534
  %9538 = load i8, ptr %9537, align 1, !tbaa !10
  %9539 = icmp eq i8 %9536, %9538
  br i1 %9539, label %9540, label %9550

9540:
  %9541 = add nuw nsw i64 %9544, 2
  %9542 = icmp eq i64 %9541, 700
  br i1 %9542, label %9551, label %9543, !llvm.loop !144

9543:
  %9544 = phi i64 [ 0, %9532 ], [ %9541, %9540 ]
  %9545 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9544
  %9546 = load i8, ptr %9545, align 2, !tbaa !10
  %9547 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9544
  %9548 = load i8, ptr %9547, align 2, !tbaa !10
  %9549 = icmp eq i8 %9546, %9548
  br i1 %9549, label %9533, label %9550

9550:
  tail call void @report(i64 noundef -3)
  store i1 true, ptr @bad, align 4
  br label %9552

9551:
  br label %9552

9552:
  tail call void @lc5()
  tail call void @lr5()
  br label %9563

9553:
  %9554 = or disjoint i64 %9564, 1
  %9555 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9554
  %9556 = load i8, ptr %9555, align 1, !tbaa !10
  %9557 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9554
  %9558 = load i8, ptr %9557, align 1, !tbaa !10
  %9559 = icmp eq i8 %9556, %9558
  br i1 %9559, label %9560, label %9570

9560:
  %9561 = add nuw nsw i64 %9564, 2
  %9562 = icmp eq i64 %9561, 700
  br i1 %9562, label %9571, label %9563, !llvm.loop !144

9563:
  %9564 = phi i64 [ 0, %9552 ], [ %9561, %9560 ]
  %9565 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9564
  %9566 = load i8, ptr %9565, align 2, !tbaa !10
  %9567 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9564
  %9568 = load i8, ptr %9567, align 2, !tbaa !10
  %9569 = icmp eq i8 %9566, %9568
  br i1 %9569, label %9553, label %9570

9570:
  tail call void @report(i64 noundef -5)
  store i1 true, ptr @bad, align 4
  br label %9572

9571:
  br label %9572

9572:
  tail call void @lc7()
  tail call void @lr7()
  br label %9583

9573:
  %9574 = or disjoint i64 %9584, 1
  %9575 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9574
  %9576 = load i8, ptr %9575, align 1, !tbaa !10
  %9577 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9574
  %9578 = load i8, ptr %9577, align 1, !tbaa !10
  %9579 = icmp eq i8 %9576, %9578
  br i1 %9579, label %9580, label %9590

9580:
  %9581 = add nuw nsw i64 %9584, 2
  %9582 = icmp eq i64 %9581, 700
  br i1 %9582, label %9591, label %9583, !llvm.loop !144

9583:
  %9584 = phi i64 [ 0, %9572 ], [ %9581, %9580 ]
  %9585 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9584
  %9586 = load i8, ptr %9585, align 2, !tbaa !10
  %9587 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9584
  %9588 = load i8, ptr %9587, align 2, !tbaa !10
  %9589 = icmp eq i8 %9586, %9588
  br i1 %9589, label %9573, label %9590

9590:
  tail call void @report(i64 noundef -7)
  store i1 true, ptr @bad, align 4
  br label %9592

9591:
  br label %9592

9592:
  tail call void @lc8()
  tail call void @lr8()
  br label %9603

9593:
  %9594 = or disjoint i64 %9604, 1
  %9595 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9594
  %9596 = load i8, ptr %9595, align 1, !tbaa !10
  %9597 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9594
  %9598 = load i8, ptr %9597, align 1, !tbaa !10
  %9599 = icmp eq i8 %9596, %9598
  br i1 %9599, label %9600, label %9610

9600:
  %9601 = add nuw nsw i64 %9604, 2
  %9602 = icmp eq i64 %9601, 700
  br i1 %9602, label %9611, label %9603, !llvm.loop !144

9603:
  %9604 = phi i64 [ 0, %9592 ], [ %9601, %9600 ]
  %9605 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9604
  %9606 = load i8, ptr %9605, align 2, !tbaa !10
  %9607 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9604
  %9608 = load i8, ptr %9607, align 2, !tbaa !10
  %9609 = icmp eq i8 %9606, %9608
  br i1 %9609, label %9593, label %9610

9610:
  tail call void @report(i64 noundef -8)
  store i1 true, ptr @bad, align 4
  br label %9612

9611:
  br label %9612

9612:
  tail call void @lc9()
  tail call void @lr9()
  br label %9623

9613:
  %9614 = or disjoint i64 %9624, 1
  %9615 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9614
  %9616 = load i8, ptr %9615, align 1, !tbaa !10
  %9617 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9614
  %9618 = load i8, ptr %9617, align 1, !tbaa !10
  %9619 = icmp eq i8 %9616, %9618
  br i1 %9619, label %9620, label %9630

9620:
  %9621 = add nuw nsw i64 %9624, 2
  %9622 = icmp eq i64 %9621, 700
  br i1 %9622, label %9631, label %9623, !llvm.loop !144

9623:
  %9624 = phi i64 [ 0, %9612 ], [ %9621, %9620 ]
  %9625 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9624
  %9626 = load i8, ptr %9625, align 2, !tbaa !10
  %9627 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9624
  %9628 = load i8, ptr %9627, align 2, !tbaa !10
  %9629 = icmp eq i8 %9626, %9628
  br i1 %9629, label %9613, label %9630

9630:
  tail call void @report(i64 noundef -9)
  store i1 true, ptr @bad, align 4
  br label %9632

9631:
  br label %9632

9632:
  tail call void @lc15()
  tail call void @lr15()
  br label %9643

9633:
  %9634 = or disjoint i64 %9644, 1
  %9635 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9634
  %9636 = load i8, ptr %9635, align 1, !tbaa !10
  %9637 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9634
  %9638 = load i8, ptr %9637, align 1, !tbaa !10
  %9639 = icmp eq i8 %9636, %9638
  br i1 %9639, label %9640, label %9650

9640:
  %9641 = add nuw nsw i64 %9644, 2
  %9642 = icmp eq i64 %9641, 700
  br i1 %9642, label %9651, label %9643, !llvm.loop !144

9643:
  %9644 = phi i64 [ 0, %9632 ], [ %9641, %9640 ]
  %9645 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9644
  %9646 = load i8, ptr %9645, align 2, !tbaa !10
  %9647 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9644
  %9648 = load i8, ptr %9647, align 2, !tbaa !10
  %9649 = icmp eq i8 %9646, %9648
  br i1 %9649, label %9633, label %9650

9650:
  tail call void @report(i64 noundef -15)
  store i1 true, ptr @bad, align 4
  br label %9652

9651:
  br label %9652

9652:
  tail call void @lc16()
  tail call void @lr16()
  br label %9663

9653:
  %9654 = or disjoint i64 %9664, 1
  %9655 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9654
  %9656 = load i8, ptr %9655, align 1, !tbaa !10
  %9657 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9654
  %9658 = load i8, ptr %9657, align 1, !tbaa !10
  %9659 = icmp eq i8 %9656, %9658
  br i1 %9659, label %9660, label %9670

9660:
  %9661 = add nuw nsw i64 %9664, 2
  %9662 = icmp eq i64 %9661, 700
  br i1 %9662, label %9671, label %9663, !llvm.loop !144

9663:
  %9664 = phi i64 [ 0, %9652 ], [ %9661, %9660 ]
  %9665 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9664
  %9666 = load i8, ptr %9665, align 2, !tbaa !10
  %9667 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9664
  %9668 = load i8, ptr %9667, align 2, !tbaa !10
  %9669 = icmp eq i8 %9666, %9668
  br i1 %9669, label %9653, label %9670

9670:
  tail call void @report(i64 noundef -16)
  store i1 true, ptr @bad, align 4
  br label %9672

9671:
  br label %9672

9672:
  tail call void @lc17()
  tail call void @lr17()
  br label %9683

9673:
  %9674 = or disjoint i64 %9684, 1
  %9675 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9674
  %9676 = load i8, ptr %9675, align 1, !tbaa !10
  %9677 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9674
  %9678 = load i8, ptr %9677, align 1, !tbaa !10
  %9679 = icmp eq i8 %9676, %9678
  br i1 %9679, label %9680, label %9690

9680:
  %9681 = add nuw nsw i64 %9684, 2
  %9682 = icmp eq i64 %9681, 700
  br i1 %9682, label %9691, label %9683, !llvm.loop !144

9683:
  %9684 = phi i64 [ 0, %9672 ], [ %9681, %9680 ]
  %9685 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9684
  %9686 = load i8, ptr %9685, align 2, !tbaa !10
  %9687 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9684
  %9688 = load i8, ptr %9687, align 2, !tbaa !10
  %9689 = icmp eq i8 %9686, %9688
  br i1 %9689, label %9673, label %9690

9690:
  tail call void @report(i64 noundef -17)
  store i1 true, ptr @bad, align 4
  br label %9692

9691:
  br label %9692

9692:
  tail call void @lc24()
  tail call void @lr24()
  br label %9703

9693:
  %9694 = or disjoint i64 %9704, 1
  %9695 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9694
  %9696 = load i8, ptr %9695, align 1, !tbaa !10
  %9697 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9694
  %9698 = load i8, ptr %9697, align 1, !tbaa !10
  %9699 = icmp eq i8 %9696, %9698
  br i1 %9699, label %9700, label %9710

9700:
  %9701 = add nuw nsw i64 %9704, 2
  %9702 = icmp eq i64 %9701, 700
  br i1 %9702, label %9711, label %9703, !llvm.loop !144

9703:
  %9704 = phi i64 [ 0, %9692 ], [ %9701, %9700 ]
  %9705 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9704
  %9706 = load i8, ptr %9705, align 2, !tbaa !10
  %9707 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9704
  %9708 = load i8, ptr %9707, align 2, !tbaa !10
  %9709 = icmp eq i8 %9706, %9708
  br i1 %9709, label %9693, label %9710

9710:
  tail call void @report(i64 noundef -24)
  store i1 true, ptr @bad, align 4
  br label %9712

9711:
  br label %9712

9712:
  tail call void @lc31()
  tail call void @lr31()
  br label %9723

9713:
  %9714 = or disjoint i64 %9724, 1
  %9715 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9714
  %9716 = load i8, ptr %9715, align 1, !tbaa !10
  %9717 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9714
  %9718 = load i8, ptr %9717, align 1, !tbaa !10
  %9719 = icmp eq i8 %9716, %9718
  br i1 %9719, label %9720, label %9730

9720:
  %9721 = add nuw nsw i64 %9724, 2
  %9722 = icmp eq i64 %9721, 700
  br i1 %9722, label %9731, label %9723, !llvm.loop !144

9723:
  %9724 = phi i64 [ 0, %9712 ], [ %9721, %9720 ]
  %9725 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9724
  %9726 = load i8, ptr %9725, align 2, !tbaa !10
  %9727 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9724
  %9728 = load i8, ptr %9727, align 2, !tbaa !10
  %9729 = icmp eq i8 %9726, %9728
  br i1 %9729, label %9713, label %9730

9730:
  tail call void @report(i64 noundef -31)
  store i1 true, ptr @bad, align 4
  br label %9732

9731:
  br label %9732

9732:
  tail call void @lc32()
  tail call void @lr32()
  br label %9743

9733:
  %9734 = or disjoint i64 %9744, 1
  %9735 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9734
  %9736 = load i8, ptr %9735, align 1, !tbaa !10
  %9737 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9734
  %9738 = load i8, ptr %9737, align 1, !tbaa !10
  %9739 = icmp eq i8 %9736, %9738
  br i1 %9739, label %9740, label %9750

9740:
  %9741 = add nuw nsw i64 %9744, 2
  %9742 = icmp eq i64 %9741, 700
  br i1 %9742, label %9751, label %9743, !llvm.loop !144

9743:
  %9744 = phi i64 [ 0, %9732 ], [ %9741, %9740 ]
  %9745 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9744
  %9746 = load i8, ptr %9745, align 2, !tbaa !10
  %9747 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9744
  %9748 = load i8, ptr %9747, align 2, !tbaa !10
  %9749 = icmp eq i8 %9746, %9748
  br i1 %9749, label %9733, label %9750

9750:
  tail call void @report(i64 noundef -32)
  store i1 true, ptr @bad, align 4
  br label %9752

9751:
  br label %9752

9752:
  tail call void @lc33()
  tail call void @lr33()
  br label %9763

9753:
  %9754 = or disjoint i64 %9764, 1
  %9755 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9754
  %9756 = load i8, ptr %9755, align 1, !tbaa !10
  %9757 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9754
  %9758 = load i8, ptr %9757, align 1, !tbaa !10
  %9759 = icmp eq i8 %9756, %9758
  br i1 %9759, label %9760, label %9770

9760:
  %9761 = add nuw nsw i64 %9764, 2
  %9762 = icmp eq i64 %9761, 700
  br i1 %9762, label %9771, label %9763, !llvm.loop !144

9763:
  %9764 = phi i64 [ 0, %9752 ], [ %9761, %9760 ]
  %9765 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9764
  %9766 = load i8, ptr %9765, align 2, !tbaa !10
  %9767 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9764
  %9768 = load i8, ptr %9767, align 2, !tbaa !10
  %9769 = icmp eq i8 %9766, %9768
  br i1 %9769, label %9753, label %9770

9770:
  tail call void @report(i64 noundef -33)
  store i1 true, ptr @bad, align 4
  br label %9772

9771:
  br label %9772

9772:
  tail call void @lc40()
  tail call void @lr40()
  br label %9783

9773:
  %9774 = or disjoint i64 %9784, 1
  %9775 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9774
  %9776 = load i8, ptr %9775, align 1, !tbaa !10
  %9777 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9774
  %9778 = load i8, ptr %9777, align 1, !tbaa !10
  %9779 = icmp eq i8 %9776, %9778
  br i1 %9779, label %9780, label %9790

9780:
  %9781 = add nuw nsw i64 %9784, 2
  %9782 = icmp eq i64 %9781, 700
  br i1 %9782, label %9791, label %9783, !llvm.loop !144

9783:
  %9784 = phi i64 [ 0, %9772 ], [ %9781, %9780 ]
  %9785 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9784
  %9786 = load i8, ptr %9785, align 2, !tbaa !10
  %9787 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9784
  %9788 = load i8, ptr %9787, align 2, !tbaa !10
  %9789 = icmp eq i8 %9786, %9788
  br i1 %9789, label %9773, label %9790

9790:
  tail call void @report(i64 noundef -40)
  store i1 true, ptr @bad, align 4
  br label %9792

9791:
  br label %9792

9792:
  tail call void @lc63()
  tail call void @lr63()
  br label %9803

9793:
  %9794 = or disjoint i64 %9804, 1
  %9795 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9794
  %9796 = load i8, ptr %9795, align 1, !tbaa !10
  %9797 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9794
  %9798 = load i8, ptr %9797, align 1, !tbaa !10
  %9799 = icmp eq i8 %9796, %9798
  br i1 %9799, label %9800, label %9810

9800:
  %9801 = add nuw nsw i64 %9804, 2
  %9802 = icmp eq i64 %9801, 700
  br i1 %9802, label %9811, label %9803, !llvm.loop !144

9803:
  %9804 = phi i64 [ 0, %9792 ], [ %9801, %9800 ]
  %9805 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9804
  %9806 = load i8, ptr %9805, align 2, !tbaa !10
  %9807 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9804
  %9808 = load i8, ptr %9807, align 2, !tbaa !10
  %9809 = icmp eq i8 %9806, %9808
  br i1 %9809, label %9793, label %9810

9810:
  tail call void @report(i64 noundef -63)
  store i1 true, ptr @bad, align 4
  br label %9812

9811:
  br label %9812

9812:
  tail call void @lc64()
  tail call void @lr64()
  br label %9823

9813:
  %9814 = or disjoint i64 %9824, 1
  %9815 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9814
  %9816 = load i8, ptr %9815, align 1, !tbaa !10
  %9817 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9814
  %9818 = load i8, ptr %9817, align 1, !tbaa !10
  %9819 = icmp eq i8 %9816, %9818
  br i1 %9819, label %9820, label %9830

9820:
  %9821 = add nuw nsw i64 %9824, 2
  %9822 = icmp eq i64 %9821, 700
  br i1 %9822, label %9831, label %9823, !llvm.loop !144

9823:
  %9824 = phi i64 [ 0, %9812 ], [ %9821, %9820 ]
  %9825 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9824
  %9826 = load i8, ptr %9825, align 2, !tbaa !10
  %9827 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9824
  %9828 = load i8, ptr %9827, align 2, !tbaa !10
  %9829 = icmp eq i8 %9826, %9828
  br i1 %9829, label %9813, label %9830

9830:
  tail call void @report(i64 noundef -64)
  store i1 true, ptr @bad, align 4
  br label %9832

9831:
  br label %9832

9832:
  tail call void @lc65()
  tail call void @lr65()
  br label %9843

9833:
  %9834 = or disjoint i64 %9844, 1
  %9835 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9834
  %9836 = load i8, ptr %9835, align 1, !tbaa !10
  %9837 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9834
  %9838 = load i8, ptr %9837, align 1, !tbaa !10
  %9839 = icmp eq i8 %9836, %9838
  br i1 %9839, label %9840, label %9850

9840:
  %9841 = add nuw nsw i64 %9844, 2
  %9842 = icmp eq i64 %9841, 700
  br i1 %9842, label %9851, label %9843, !llvm.loop !144

9843:
  %9844 = phi i64 [ 0, %9832 ], [ %9841, %9840 ]
  %9845 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9844
  %9846 = load i8, ptr %9845, align 2, !tbaa !10
  %9847 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9844
  %9848 = load i8, ptr %9847, align 2, !tbaa !10
  %9849 = icmp eq i8 %9846, %9848
  br i1 %9849, label %9833, label %9850

9850:
  tail call void @report(i64 noundef -65)
  store i1 true, ptr @bad, align 4
  br label %9852

9851:
  br label %9852

9852:
  tail call void @lc100()
  tail call void @lr100()
  br label %9863

9853:
  %9854 = or disjoint i64 %9864, 1
  %9855 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9854
  %9856 = load i8, ptr %9855, align 1, !tbaa !10
  %9857 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9854
  %9858 = load i8, ptr %9857, align 1, !tbaa !10
  %9859 = icmp eq i8 %9856, %9858
  br i1 %9859, label %9860, label %9870

9860:
  %9861 = add nuw nsw i64 %9864, 2
  %9862 = icmp eq i64 %9861, 700
  br i1 %9862, label %9871, label %9863, !llvm.loop !144

9863:
  %9864 = phi i64 [ 0, %9852 ], [ %9861, %9860 ]
  %9865 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9864
  %9866 = load i8, ptr %9865, align 2, !tbaa !10
  %9867 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9864
  %9868 = load i8, ptr %9867, align 2, !tbaa !10
  %9869 = icmp eq i8 %9866, %9868
  br i1 %9869, label %9853, label %9870

9870:
  tail call void @report(i64 noundef -100)
  store i1 true, ptr @bad, align 4
  br label %9872

9871:
  br label %9872

9872:
  tail call void @lc127()
  tail call void @lr127()
  br label %9883

9873:
  %9874 = or disjoint i64 %9884, 1
  %9875 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9874
  %9876 = load i8, ptr %9875, align 1, !tbaa !10
  %9877 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9874
  %9878 = load i8, ptr %9877, align 1, !tbaa !10
  %9879 = icmp eq i8 %9876, %9878
  br i1 %9879, label %9880, label %9890

9880:
  %9881 = add nuw nsw i64 %9884, 2
  %9882 = icmp eq i64 %9881, 700
  br i1 %9882, label %9891, label %9883, !llvm.loop !144

9883:
  %9884 = phi i64 [ 0, %9872 ], [ %9881, %9880 ]
  %9885 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9884
  %9886 = load i8, ptr %9885, align 2, !tbaa !10
  %9887 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9884
  %9888 = load i8, ptr %9887, align 2, !tbaa !10
  %9889 = icmp eq i8 %9886, %9888
  br i1 %9889, label %9873, label %9890

9890:
  tail call void @report(i64 noundef -127)
  store i1 true, ptr @bad, align 4
  br label %9892

9891:
  br label %9892

9892:
  tail call void @lc128()
  tail call void @lr128()
  br label %9903

9893:
  %9894 = or disjoint i64 %9904, 1
  %9895 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9894
  %9896 = load i8, ptr %9895, align 1, !tbaa !10
  %9897 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9894
  %9898 = load i8, ptr %9897, align 1, !tbaa !10
  %9899 = icmp eq i8 %9896, %9898
  br i1 %9899, label %9900, label %9910

9900:
  %9901 = add nuw nsw i64 %9904, 2
  %9902 = icmp eq i64 %9901, 700
  br i1 %9902, label %9911, label %9903, !llvm.loop !144

9903:
  %9904 = phi i64 [ 0, %9892 ], [ %9901, %9900 ]
  %9905 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9904
  %9906 = load i8, ptr %9905, align 2, !tbaa !10
  %9907 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9904
  %9908 = load i8, ptr %9907, align 2, !tbaa !10
  %9909 = icmp eq i8 %9906, %9908
  br i1 %9909, label %9893, label %9910

9910:
  tail call void @report(i64 noundef -128)
  store i1 true, ptr @bad, align 4
  br label %9912

9911:
  br label %9912

9912:
  tail call void @lc129()
  tail call void @lr129()
  br label %9923

9913:
  %9914 = or disjoint i64 %9924, 1
  %9915 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9914
  %9916 = load i8, ptr %9915, align 1, !tbaa !10
  %9917 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9914
  %9918 = load i8, ptr %9917, align 1, !tbaa !10
  %9919 = icmp eq i8 %9916, %9918
  br i1 %9919, label %9920, label %9930

9920:
  %9921 = add nuw nsw i64 %9924, 2
  %9922 = icmp eq i64 %9921, 700
  br i1 %9922, label %9931, label %9923, !llvm.loop !144

9923:
  %9924 = phi i64 [ 0, %9912 ], [ %9921, %9920 ]
  %9925 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9924
  %9926 = load i8, ptr %9925, align 2, !tbaa !10
  %9927 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9924
  %9928 = load i8, ptr %9927, align 2, !tbaa !10
  %9929 = icmp eq i8 %9926, %9928
  br i1 %9929, label %9913, label %9930

9930:
  tail call void @report(i64 noundef -129)
  store i1 true, ptr @bad, align 4
  br label %9932

9931:
  br label %9932

9932:
  tail call void @lc200()
  tail call void @lr200()
  br label %9943

9933:
  %9934 = or disjoint i64 %9944, 1
  %9935 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9934
  %9936 = load i8, ptr %9935, align 1, !tbaa !10
  %9937 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9934
  %9938 = load i8, ptr %9937, align 1, !tbaa !10
  %9939 = icmp eq i8 %9936, %9938
  br i1 %9939, label %9940, label %9950

9940:
  %9941 = add nuw nsw i64 %9944, 2
  %9942 = icmp eq i64 %9941, 700
  br i1 %9942, label %9951, label %9943, !llvm.loop !144

9943:
  %9944 = phi i64 [ 0, %9932 ], [ %9941, %9940 ]
  %9945 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9944
  %9946 = load i8, ptr %9945, align 2, !tbaa !10
  %9947 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9944
  %9948 = load i8, ptr %9947, align 2, !tbaa !10
  %9949 = icmp eq i8 %9946, %9948
  br i1 %9949, label %9933, label %9950

9950:
  tail call void @report(i64 noundef -200)
  store i1 true, ptr @bad, align 4
  br label %9952

9951:
  br label %9952

9952:
  tail call void @lc255()
  tail call void @lr255()
  br label %9963

9953:
  %9954 = or disjoint i64 %9964, 1
  %9955 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9954
  %9956 = load i8, ptr %9955, align 1, !tbaa !10
  %9957 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9954
  %9958 = load i8, ptr %9957, align 1, !tbaa !10
  %9959 = icmp eq i8 %9956, %9958
  br i1 %9959, label %9960, label %9970

9960:
  %9961 = add nuw nsw i64 %9964, 2
  %9962 = icmp eq i64 %9961, 700
  br i1 %9962, label %9971, label %9963, !llvm.loop !144

9963:
  %9964 = phi i64 [ 0, %9952 ], [ %9961, %9960 ]
  %9965 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9964
  %9966 = load i8, ptr %9965, align 2, !tbaa !10
  %9967 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9964
  %9968 = load i8, ptr %9967, align 2, !tbaa !10
  %9969 = icmp eq i8 %9966, %9968
  br i1 %9969, label %9953, label %9970

9970:
  tail call void @report(i64 noundef -255)
  store i1 true, ptr @bad, align 4
  br label %9972

9971:
  br label %9972

9972:
  tail call void @lc256()
  tail call void @lr256()
  br label %9983

9973:
  %9974 = or disjoint i64 %9984, 1
  %9975 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9974
  %9976 = load i8, ptr %9975, align 1, !tbaa !10
  %9977 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9974
  %9978 = load i8, ptr %9977, align 1, !tbaa !10
  %9979 = icmp eq i8 %9976, %9978
  br i1 %9979, label %9980, label %9990

9980:
  %9981 = add nuw nsw i64 %9984, 2
  %9982 = icmp eq i64 %9981, 700
  br i1 %9982, label %9991, label %9983, !llvm.loop !144

9983:
  %9984 = phi i64 [ 0, %9972 ], [ %9981, %9980 ]
  %9985 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9984
  %9986 = load i8, ptr %9985, align 2, !tbaa !10
  %9987 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9984
  %9988 = load i8, ptr %9987, align 2, !tbaa !10
  %9989 = icmp eq i8 %9986, %9988
  br i1 %9989, label %9973, label %9990

9990:
  tail call void @report(i64 noundef -256)
  store i1 true, ptr @bad, align 4
  br label %9992

9991:
  br label %9992

9992:
  tail call void @lc257()
  tail call void @lr257()
  br label %10003

9993:
  %9994 = or disjoint i64 %10004, 1
  %9995 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9994
  %9996 = load i8, ptr %9995, align 1, !tbaa !10
  %9997 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %9994
  %9998 = load i8, ptr %9997, align 1, !tbaa !10
  %9999 = icmp eq i8 %9996, %9998
  br i1 %9999, label %10000, label %10010

10000:
  %10001 = add nuw nsw i64 %10004, 2
  %10002 = icmp eq i64 %10001, 700
  br i1 %10002, label %10011, label %10003, !llvm.loop !144

10003:
  %10004 = phi i64 [ 0, %9992 ], [ %10001, %10000 ]
  %10005 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10004
  %10006 = load i8, ptr %10005, align 2, !tbaa !10
  %10007 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10004
  %10008 = load i8, ptr %10007, align 2, !tbaa !10
  %10009 = icmp eq i8 %10006, %10008
  br i1 %10009, label %9993, label %10010

10010:
  tail call void @report(i64 noundef -257)
  store i1 true, ptr @bad, align 4
  br label %10012

10011:
  br label %10012

10012:
  tail call void @lc300()
  tail call void @lr300()
  br label %10023

10013:
  %10014 = or disjoint i64 %10024, 1
  %10015 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10014
  %10016 = load i8, ptr %10015, align 1, !tbaa !10
  %10017 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10014
  %10018 = load i8, ptr %10017, align 1, !tbaa !10
  %10019 = icmp eq i8 %10016, %10018
  br i1 %10019, label %10020, label %10030

10020:
  %10021 = add nuw nsw i64 %10024, 2
  %10022 = icmp eq i64 %10021, 700
  br i1 %10022, label %10031, label %10023, !llvm.loop !144

10023:
  %10024 = phi i64 [ 0, %10012 ], [ %10021, %10020 ]
  %10025 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10024
  %10026 = load i8, ptr %10025, align 2, !tbaa !10
  %10027 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10024
  %10028 = load i8, ptr %10027, align 2, !tbaa !10
  %10029 = icmp eq i8 %10026, %10028
  br i1 %10029, label %10013, label %10030

10030:
  tail call void @report(i64 noundef -300)
  store i1 true, ptr @bad, align 4
  br label %10032

10031:
  br label %10032

10032:
  tail call void @lc511()
  tail call void @lr511()
  br label %10043

10033:
  %10034 = or disjoint i64 %10044, 1
  %10035 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10034
  %10036 = load i8, ptr %10035, align 1, !tbaa !10
  %10037 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10034
  %10038 = load i8, ptr %10037, align 1, !tbaa !10
  %10039 = icmp eq i8 %10036, %10038
  br i1 %10039, label %10040, label %10050

10040:
  %10041 = add nuw nsw i64 %10044, 2
  %10042 = icmp eq i64 %10041, 700
  br i1 %10042, label %10051, label %10043, !llvm.loop !144

10043:
  %10044 = phi i64 [ 0, %10032 ], [ %10041, %10040 ]
  %10045 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10044
  %10046 = load i8, ptr %10045, align 2, !tbaa !10
  %10047 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10044
  %10048 = load i8, ptr %10047, align 2, !tbaa !10
  %10049 = icmp eq i8 %10046, %10048
  br i1 %10049, label %10033, label %10050

10050:
  tail call void @report(i64 noundef -511)
  store i1 true, ptr @bad, align 4
  br label %10052

10051:
  br label %10052

10052:
  tail call void @lc512()
  tail call void @lr512()
  br label %10063

10053:
  %10054 = or disjoint i64 %10064, 1
  %10055 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10054
  %10056 = load i8, ptr %10055, align 1, !tbaa !10
  %10057 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10054
  %10058 = load i8, ptr %10057, align 1, !tbaa !10
  %10059 = icmp eq i8 %10056, %10058
  br i1 %10059, label %10060, label %10070

10060:
  %10061 = add nuw nsw i64 %10064, 2
  %10062 = icmp eq i64 %10061, 700
  br i1 %10062, label %10071, label %10063, !llvm.loop !144

10063:
  %10064 = phi i64 [ 0, %10052 ], [ %10061, %10060 ]
  %10065 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10064
  %10066 = load i8, ptr %10065, align 2, !tbaa !10
  %10067 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10064
  %10068 = load i8, ptr %10067, align 2, !tbaa !10
  %10069 = icmp eq i8 %10066, %10068
  br i1 %10069, label %10053, label %10070

10070:
  tail call void @report(i64 noundef -512)
  store i1 true, ptr @bad, align 4
  br label %10072

10071:
  br label %10072

10072:
  tail call void @lc513()
  tail call void @lr513()
  br label %10083

10073:
  %10074 = or disjoint i64 %10084, 1
  %10075 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10074
  %10076 = load i8, ptr %10075, align 1, !tbaa !10
  %10077 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10074
  %10078 = load i8, ptr %10077, align 1, !tbaa !10
  %10079 = icmp eq i8 %10076, %10078
  br i1 %10079, label %10080, label %10090

10080:
  %10081 = add nuw nsw i64 %10084, 2
  %10082 = icmp eq i64 %10081, 700
  br i1 %10082, label %10091, label %10083, !llvm.loop !144

10083:
  %10084 = phi i64 [ 0, %10072 ], [ %10081, %10080 ]
  %10085 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10084
  %10086 = load i8, ptr %10085, align 2, !tbaa !10
  %10087 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10084
  %10088 = load i8, ptr %10087, align 2, !tbaa !10
  %10089 = icmp eq i8 %10086, %10088
  br i1 %10089, label %10073, label %10090

10090:
  tail call void @report(i64 noundef -513)
  store i1 true, ptr @bad, align 4
  br label %10092

10091:
  br label %10092

10092:
  tail call void @lc600()
  tail call void @lr600()
  br label %10103

10093:
  %10094 = or disjoint i64 %10104, 1
  %10095 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10094
  %10096 = load i8, ptr %10095, align 1, !tbaa !10
  %10097 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10094
  %10098 = load i8, ptr %10097, align 1, !tbaa !10
  %10099 = icmp eq i8 %10096, %10098
  br i1 %10099, label %10100, label %10110

10100:
  %10101 = add nuw nsw i64 %10104, 2
  %10102 = icmp eq i64 %10101, 700
  br i1 %10102, label %10111, label %10103, !llvm.loop !144

10103:
  %10104 = phi i64 [ 0, %10092 ], [ %10101, %10100 ]
  %10105 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %10104
  %10106 = load i8, ptr %10105, align 2, !tbaa !10
  %10107 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10104
  %10108 = load i8, ptr %10107, align 2, !tbaa !10
  %10109 = icmp eq i8 %10106, %10108
  br i1 %10109, label %10093, label %10110

10110:
  tail call void @report(i64 noundef -600)
  store i1 true, ptr @bad, align 4
  br label %10114

10111:
  %10112 = load i1, ptr @bad, align 4
  %10113 = zext i1 %10112 to i64
  br label %10114

10114:
  %10115 = phi i64 [ %10113, %10111 ], [ 1, %10110 ]
  tail call void @report(i64 noundef %10115)
  %10116 = load i1, ptr @bad, align 4
  %10117 = zext i1 %10116 to i32
  ret i32 %10117
}

define internal void @cp1() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = load i8, ptr getelementptr inbounds nuw (i8, ptr @src, i64 1), align 1
  store i8 %1, ptr getelementptr inbounds nuw (i8, ptr @dst, i64 5), align 1
  ret void
}

define internal void @ref_copy(ptr noundef nofree %0, ptr noundef nocapture nofree readonly %1, i32 noundef range(i32 0, -2147483648) %2) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse memory(argmem: readwrite, inaccessiblemem: readwrite) willreturn {
  %4 = icmp eq i32 %2, 0
  br i1 %4, label %14, label %5

5:
  %wide.trip.count = zext nneg i32 %2 to i64
  %xtraiter = and i64 %wide.trip.count, 3
  %6 = icmp samesign ult i32 %2, 4
  br i1 %6, label %.epil.preheader, label %.new

.new:
  %7 = and i64 %wide.trip.count, 2147483644
  br label %15

.unr-lcssa:
  %lcmp.mod = icmp eq i64 %xtraiter, 0
  br i1 %lcmp.mod, label %14, label %.epil.preheader

.epil.preheader:
  %.epil.init = phi i64 [ 0, %5 ], [ %32, %.unr-lcssa ]
  %lcmp.mod1 = icmp ne i64 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod1)
  br label %.epil

.epil:
  %8 = phi i64 [ %.epil.init, %.epil.preheader ], [ %12, %.epil ]
  %epil.iter = phi i64 [ 0, %.epil.preheader ], [ %epil.iter.next, %.epil ]
  %9 = getelementptr inbounds nuw i8, ptr %1, i64 %8
  %10 = load i8, ptr %9, align 1, !tbaa !10
  %11 = getelementptr inbounds nuw i8, ptr %0, i64 %8
  store volatile i8 %10, ptr %11, align 1, !tbaa !10
  %12 = add nuw nsw i64 %8, 1
  %epil.iter.next = add i64 %epil.iter, 1
  %epil.iter.cmp = icmp eq i64 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp, label %13, label %.epil

13:
  br label %14

14:
  ret void

15:
  %16 = phi i64 [ 0, %.new ], [ %32, %15 ]
  %niter = phi i64 [ 0, %.new ], [ %33, %15 ]
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %0, i64 %16
  store volatile i8 %18, ptr %19, align 1, !tbaa !10
  %20 = or disjoint i64 %16, 1
  %21 = getelementptr inbounds nuw i8, ptr %1, i64 %20
  %22 = load i8, ptr %21, align 1, !tbaa !10
  %23 = getelementptr inbounds nuw i8, ptr %0, i64 %20
  store volatile i8 %22, ptr %23, align 1, !tbaa !10
  %24 = or disjoint i64 %16, 2
  %25 = getelementptr inbounds nuw i8, ptr %1, i64 %24
  %26 = load i8, ptr %25, align 1, !tbaa !10
  %27 = getelementptr inbounds nuw i8, ptr %0, i64 %24
  store volatile i8 %26, ptr %27, align 1, !tbaa !10
  %28 = or disjoint i64 %16, 3
  %29 = getelementptr inbounds nuw i8, ptr %1, i64 %28
  %30 = load i8, ptr %29, align 1, !tbaa !10
  %31 = getelementptr inbounds nuw i8, ptr %0, i64 %28
  store volatile i8 %30, ptr %31, align 1, !tbaa !10
  %32 = add nuw nsw i64 %16, 4
  %33 = add i64 %niter, 4
  %34 = icmp eq i64 %33, %7
  br i1 %34, label %.unr-lcssa, label %15, !llvm.loop !17
}

define internal void @zr1() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 1, i1 false)
  ret void
}

define internal void @ref_fill(ptr noundef nofree %0, i32 noundef range(i32 0, 194) %1, i32 noundef range(i32 0, -2147483648) %2) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse memory(argmem: readwrite, inaccessiblemem: readwrite) willreturn {
  %4 = icmp eq i32 %2, 0
  br i1 %4, label %13, label %5

5:
  %6 = trunc nuw i32 %1 to i8
  %wide.trip.count = zext nneg i32 %2 to i64
  %xtraiter = and i64 %wide.trip.count, 7
  %7 = icmp samesign ult i32 %2, 8
  br i1 %7, label %.epil.preheader, label %.new

.new:
  %8 = and i64 %wide.trip.count, 2147483640
  br label %14

.unr-lcssa:
  %lcmp.mod = icmp eq i64 %xtraiter, 0
  br i1 %lcmp.mod, label %13, label %.epil.preheader

.epil.preheader:
  %.epil.init = phi i64 [ 0, %5 ], [ %31, %.unr-lcssa ]
  %lcmp.mod1 = icmp ne i64 %xtraiter, 0
  tail call void @llvm.assume(i1 %lcmp.mod1)
  br label %.epil

.epil:
  %9 = phi i64 [ %.epil.init, %.epil.preheader ], [ %11, %.epil ]
  %epil.iter = phi i64 [ 0, %.epil.preheader ], [ %epil.iter.next, %.epil ]
  %10 = getelementptr inbounds nuw i8, ptr %0, i64 %9
  store volatile i8 %6, ptr %10, align 1, !tbaa !10
  %11 = add nuw nsw i64 %9, 1
  %epil.iter.next = add i64 %epil.iter, 1
  %epil.iter.cmp = icmp eq i64 %epil.iter.next, %xtraiter
  br i1 %epil.iter.cmp, label %12, label %.epil

12:
  br label %13

13:
  ret void

14:
  %15 = phi i64 [ 0, %.new ], [ %31, %14 ]
  %niter = phi i64 [ 0, %.new ], [ %32, %14 ]
  %16 = getelementptr inbounds nuw i8, ptr %0, i64 %15
  store volatile i8 %6, ptr %16, align 1, !tbaa !10
  %17 = getelementptr inbounds nuw i8, ptr %0, i64 %15
  %18 = getelementptr inbounds nuw i8, ptr %17, i64 1
  store volatile i8 %6, ptr %18, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %0, i64 %15
  %20 = getelementptr inbounds nuw i8, ptr %19, i64 2
  store volatile i8 %6, ptr %20, align 1, !tbaa !10
  %21 = getelementptr inbounds nuw i8, ptr %0, i64 %15
  %22 = getelementptr inbounds nuw i8, ptr %21, i64 3
  store volatile i8 %6, ptr %22, align 1, !tbaa !10
  %23 = getelementptr inbounds nuw i8, ptr %0, i64 %15
  %24 = getelementptr inbounds nuw i8, ptr %23, i64 4
  store volatile i8 %6, ptr %24, align 1, !tbaa !10
  %25 = getelementptr inbounds nuw i8, ptr %0, i64 %15
  %26 = getelementptr inbounds nuw i8, ptr %25, i64 5
  store volatile i8 %6, ptr %26, align 1, !tbaa !10
  %27 = getelementptr inbounds nuw i8, ptr %0, i64 %15
  %28 = getelementptr inbounds nuw i8, ptr %27, i64 6
  store volatile i8 %6, ptr %28, align 1, !tbaa !10
  %29 = getelementptr inbounds nuw i8, ptr %0, i64 %15
  %30 = getelementptr inbounds nuw i8, ptr %29, i64 7
  store volatile i8 %6, ptr %30, align 1, !tbaa !10
  %31 = add nuw nsw i64 %15, 8
  %32 = add i64 %niter, 8
  %33 = icmp eq i64 %32, %8
  br i1 %33, label %.unr-lcssa, label %14, !llvm.loop !18
}

define internal void @c71() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 1, i1 false)
  ret void
}

define internal void @vr1() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 1, i1 false)
  ret void
}

define internal void @mv1(ptr noundef nocapture nofree writeonly initializes((0, 1)) "llrm.reads"="" "llrm.writes"="0:1" %0, i8 %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: write) {
  store i8 %1, ptr %0, align 1
  ret void
}

define internal void @ref_move(ptr noundef %0, ptr noundef nocapture nofree readonly %1, i32 noundef range(i32 0, -2147483648) %2) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse memory(argmem: readwrite, inaccessiblemem: readwrite) willreturn {
  %4 = alloca [1200 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %4) nounwind
  call void @ref_copy(ptr noundef nonnull %4, ptr noundef %1, i32 noundef %2)
  call void @ref_copy(ptr noundef %0, ptr noundef nonnull %4, i32 noundef %2)
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %4) nounwind
  ret void
}

define internal void @cp2() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = load i16, ptr getelementptr inbounds nuw (i8, ptr @src, i64 1), align 1
  store i16 %1, ptr getelementptr inbounds nuw (i8, ptr @dst, i64 5), align 1
  ret void
}

define internal void @zr2() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 2, i1 false)
  ret void
}

define internal void @c72() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 2, i1 false)
  ret void
}

define internal void @vr2() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 2, i1 false)
  ret void
}

define internal void @mv2(ptr noundef nocapture nofree writeonly initializes((0, 2)) "llrm.reads"="" "llrm.writes"="0:2" %0, i16 %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: write) {
  store i16 %1, ptr %0, align 1
  ret void
}

define internal void @cp3() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 3, i1 false)
  ret void
}

define internal void @zr3() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 3, i1 false)
  ret void
}

define internal void @c73() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 3, i1 false)
  ret void
}

define internal void @vr3() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 3, i1 false)
  ret void
}

define internal void @mv3(ptr noundef nocapture nofree writeonly initializes((0, 3)) "llrm.reads"="" "llrm.writes"="0:3" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:3" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 3, i1 false)
  ret void
}

define internal void @cp5() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 5, i1 false)
  ret void
}

define internal void @zr5() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 5, i1 false)
  ret void
}

define internal void @c75() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 5, i1 false)
  ret void
}

define internal void @vr5() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 5, i1 false)
  ret void
}

define internal void @mv5(ptr noundef nocapture nofree writeonly initializes((0, 5)) "llrm.reads"="" "llrm.writes"="0:5" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:5" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 5, i1 false)
  ret void
}

define internal void @cp7() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 7, i1 false)
  ret void
}

define internal void @zr7() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 7, i1 false)
  ret void
}

define internal void @c77() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 7, i1 false)
  ret void
}

define internal void @vr7() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 7, i1 false)
  ret void
}

define internal void @mv7(ptr noundef nocapture nofree writeonly initializes((0, 7)) "llrm.reads"="" "llrm.writes"="0:7" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:7" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 7, i1 false)
  ret void
}

define internal void @cp8() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = load i64, ptr getelementptr inbounds nuw (i8, ptr @src, i64 1), align 1
  store i64 %1, ptr getelementptr inbounds nuw (i8, ptr @dst, i64 5), align 1
  ret void
}

define internal void @zr8() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 8, i1 false)
  ret void
}

define internal void @c78() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 8, i1 false)
  ret void
}

define internal void @vr8() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 8, i1 false)
  ret void
}

define internal void @mv8(ptr noundef nocapture nofree writeonly initializes((0, 8)) "llrm.reads"="" "llrm.writes"="0:8" %0, i64 %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: write) {
  store i64 %1, ptr %0, align 1
  ret void
}

define internal void @cp9() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 9, i1 false)
  ret void
}

define internal void @zr9() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 9, i1 false)
  ret void
}

define internal void @c79() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 9, i1 false)
  ret void
}

define internal void @vr9() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 9, i1 false)
  ret void
}

define internal void @mv9(ptr noundef nocapture nofree writeonly initializes((0, 9)) "llrm.reads"="" "llrm.writes"="0:9" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:9" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 9, i1 false)
  ret void
}

define internal void @cp15() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 15, i1 false)
  ret void
}

define internal void @zr15() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 15, i1 false)
  ret void
}

define internal void @c715() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 15, i1 false)
  ret void
}

define internal void @vr15() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 15, i1 false)
  ret void
}

define internal void @mv15(ptr noundef nocapture nofree writeonly initializes((0, 15)) "llrm.reads"="" "llrm.writes"="0:15" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:15" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 15, i1 false)
  ret void
}

define internal void @cp16() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 16, i1 false)
  ret void
}

define internal void @zr16() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 16, i1 false)
  ret void
}

define internal void @c716() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 16, i1 false)
  ret void
}

define internal void @vr16() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 16, i1 false)
  ret void
}

define internal void @mv16(ptr noundef nocapture nofree writeonly initializes((0, 16)) "llrm.reads"="" "llrm.writes"="0:16" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:16" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 16, i1 false)
  ret void
}

define internal void @cp17() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 17, i1 false)
  ret void
}

define internal void @zr17() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 17, i1 false)
  ret void
}

define internal void @c717() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 17, i1 false)
  ret void
}

define internal void @vr17() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 17, i1 false)
  ret void
}

define internal void @mv17(ptr noundef nocapture nofree writeonly initializes((0, 17)) "llrm.reads"="" "llrm.writes"="0:17" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:17" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 17, i1 false)
  ret void
}

define internal void @cp24() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 24, i1 false)
  ret void
}

define internal void @zr24() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 24, i1 false)
  ret void
}

define internal void @c724() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 24, i1 false)
  ret void
}

define internal void @vr24() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 24, i1 false)
  ret void
}

define internal void @mv24(ptr noundef nocapture nofree writeonly initializes((0, 24)) "llrm.reads"="" "llrm.writes"="0:24" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:24" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 24, i1 false)
  ret void
}

define internal void @cp31() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 31, i1 false)
  ret void
}

define internal void @zr31() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 31, i1 false)
  ret void
}

define internal void @c731() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 31, i1 false)
  ret void
}

define internal void @vr31() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 31, i1 false)
  ret void
}

define internal void @mv31(ptr noundef nocapture nofree writeonly initializes((0, 31)) "llrm.reads"="" "llrm.writes"="0:31" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:31" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 31, i1 false)
  ret void
}

define internal void @cp32() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 32, i1 false)
  ret void
}

define internal void @zr32() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 32, i1 false)
  ret void
}

define internal void @c732() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 32, i1 false)
  ret void
}

define internal void @vr32() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 32, i1 false)
  ret void
}

define internal void @mv32(ptr noundef nocapture nofree writeonly initializes((0, 32)) "llrm.reads"="" "llrm.writes"="0:32" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:32" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 32, i1 false)
  ret void
}

define internal void @cp33() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 33, i1 false)
  ret void
}

define internal void @zr33() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 33, i1 false)
  ret void
}

define internal void @c733() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 33, i1 false)
  ret void
}

define internal void @vr33() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 33, i1 false)
  ret void
}

define internal void @mv33(ptr noundef nocapture nofree writeonly initializes((0, 33)) "llrm.reads"="" "llrm.writes"="0:33" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:33" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 33, i1 false)
  ret void
}

define internal void @cp40() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 40, i1 false)
  ret void
}

define internal void @zr40() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 40, i1 false)
  ret void
}

define internal void @c740() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 40, i1 false)
  ret void
}

define internal void @vr40() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 40, i1 false)
  ret void
}

define internal void @mv40(ptr noundef nocapture nofree writeonly initializes((0, 40)) "llrm.reads"="" "llrm.writes"="0:40" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:40" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 40, i1 false)
  ret void
}

define internal void @cp63() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 63, i1 false)
  ret void
}

define internal void @zr63() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 63, i1 false)
  ret void
}

define internal void @c763() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 63, i1 false)
  ret void
}

define internal void @vr63() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 63, i1 false)
  ret void
}

define internal void @mv63(ptr noundef nocapture nofree writeonly initializes((0, 63)) "llrm.reads"="" "llrm.writes"="0:63" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:63" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 63, i1 false)
  ret void
}

define internal void @cp64() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 64, i1 false)
  ret void
}

define internal void @zr64() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 64, i1 false)
  ret void
}

define internal void @c764() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 64, i1 false)
  ret void
}

define internal void @vr64() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 64, i1 false)
  ret void
}

define internal void @mv64(ptr noundef nocapture nofree writeonly initializes((0, 64)) "llrm.reads"="" "llrm.writes"="0:64" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:64" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 64, i1 false)
  ret void
}

define internal void @cp65() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 65, i1 false)
  ret void
}

define internal void @zr65() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 65, i1 false)
  ret void
}

define internal void @c765() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 65, i1 false)
  ret void
}

define internal void @vr65() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 65, i1 false)
  ret void
}

define internal void @mv65(ptr noundef nocapture nofree writeonly initializes((0, 65)) "llrm.reads"="" "llrm.writes"="0:65" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:65" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 65, i1 false)
  ret void
}

define internal void @cp100() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 100, i1 false)
  ret void
}

define internal void @zr100() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 100, i1 false)
  ret void
}

define internal void @c7100() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 100, i1 false)
  ret void
}

define internal void @vr100() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 100, i1 false)
  ret void
}

define internal void @mv100(ptr noundef nocapture nofree writeonly initializes((0, 100)) "llrm.reads"="" "llrm.writes"="0:100" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:100" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 100, i1 false)
  ret void
}

define internal void @cp127() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 127, i1 false)
  ret void
}

define internal void @zr127() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 127, i1 false)
  ret void
}

define internal void @c7127() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 127, i1 false)
  ret void
}

define internal void @vr127() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 127, i1 false)
  ret void
}

define internal void @mv127(ptr noundef nocapture nofree writeonly initializes((0, 127)) "llrm.reads"="" "llrm.writes"="0:127" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:127" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 127, i1 false)
  ret void
}

define internal void @cp128() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 128, i1 false)
  ret void
}

define internal void @zr128() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 128, i1 false)
  ret void
}

define internal void @c7128() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 128, i1 false)
  ret void
}

define internal void @vr128() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 128, i1 false)
  ret void
}

define internal void @mv128(ptr noundef nocapture nofree writeonly initializes((0, 128)) "llrm.reads"="" "llrm.writes"="0:128" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:128" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 128, i1 false)
  ret void
}

define internal void @cp129() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 129, i1 false)
  ret void
}

define internal void @zr129() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 129, i1 false)
  ret void
}

define internal void @c7129() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 129, i1 false)
  ret void
}

define internal void @vr129() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 129, i1 false)
  ret void
}

define internal void @mv129(ptr noundef nocapture nofree writeonly initializes((0, 129)) "llrm.reads"="" "llrm.writes"="0:129" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:129" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 129, i1 false)
  ret void
}

define internal void @cp200() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 200, i1 false)
  ret void
}

define internal void @zr200() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 200, i1 false)
  ret void
}

define internal void @c7200() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 200, i1 false)
  ret void
}

define internal void @vr200() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 200, i1 false)
  ret void
}

define internal void @mv200(ptr noundef nocapture nofree writeonly initializes((0, 200)) "llrm.reads"="" "llrm.writes"="0:200" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:200" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 200, i1 false)
  ret void
}

define internal void @cp255() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 255, i1 false)
  ret void
}

define internal void @zr255() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 255, i1 false)
  ret void
}

define internal void @c7255() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 255, i1 false)
  ret void
}

define internal void @vr255() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 255, i1 false)
  ret void
}

define internal void @mv255(ptr noundef nocapture nofree writeonly initializes((0, 255)) "llrm.reads"="" "llrm.writes"="0:255" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:255" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 255, i1 false)
  ret void
}

define internal void @cp256() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 256, i1 false)
  ret void
}

define internal void @zr256() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 256, i1 false)
  ret void
}

define internal void @c7256() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 256, i1 false)
  ret void
}

define internal void @vr256() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 256, i1 false)
  ret void
}

define internal void @mv256(ptr noundef nocapture nofree writeonly initializes((0, 256)) "llrm.reads"="" "llrm.writes"="0:256" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:256" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 256, i1 false)
  ret void
}

define internal void @cp257() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 257, i1 false)
  ret void
}

define internal void @zr257() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 257, i1 false)
  ret void
}

define internal void @c7257() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 257, i1 false)
  ret void
}

define internal void @vr257() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 257, i1 false)
  ret void
}

define internal void @mv257(ptr noundef nocapture nofree writeonly initializes((0, 257)) "llrm.reads"="" "llrm.writes"="0:257" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:257" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 257, i1 false)
  ret void
}

define internal void @cp300() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 300, i1 false)
  ret void
}

define internal void @zr300() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 300, i1 false)
  ret void
}

define internal void @c7300() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 300, i1 false)
  ret void
}

define internal void @vr300() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 300, i1 false)
  ret void
}

define internal void @mv300(ptr noundef nocapture nofree writeonly initializes((0, 300)) "llrm.reads"="" "llrm.writes"="0:300" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:300" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 300, i1 false)
  ret void
}

define internal void @cp511() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 511, i1 false)
  ret void
}

define internal void @zr511() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 511, i1 false)
  ret void
}

define internal void @c7511() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 511, i1 false)
  ret void
}

define internal void @vr511() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 511, i1 false)
  ret void
}

define internal void @mv511(ptr noundef nocapture nofree writeonly initializes((0, 511)) "llrm.reads"="" "llrm.writes"="0:511" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:511" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 511, i1 false)
  ret void
}

define internal void @cp512() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 512, i1 false)
  ret void
}

define internal void @zr512() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 512, i1 false)
  ret void
}

define internal void @c7512() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 512, i1 false)
  ret void
}

define internal void @vr512() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 512, i1 false)
  ret void
}

define internal void @mv512(ptr noundef nocapture nofree writeonly initializes((0, 512)) "llrm.reads"="" "llrm.writes"="0:512" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:512" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 512, i1 false)
  ret void
}

define internal void @cp513() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 513, i1 false)
  ret void
}

define internal void @zr513() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 513, i1 false)
  ret void
}

define internal void @c7513() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 513, i1 false)
  ret void
}

define internal void @vr513() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 513, i1 false)
  ret void
}

define internal void @mv513(ptr noundef nocapture nofree writeonly initializes((0, 513)) "llrm.reads"="" "llrm.writes"="0:513" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:513" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 513, i1 false)
  ret void
}

define internal void @cp600() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(readwrite, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memcpy.p0.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 5), ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 600, i1 false)
  ret void
}

define internal void @zr600() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 0, i64 600, i1 false)
  ret void
}

define internal void @c7600() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 123, i64 600, i1 false)
  ret void
}

define internal void @vr600() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(write, argmem: none, inaccessiblemem: none) {
  tail call void @llvm.memset.p0.i64(ptr align 1 getelementptr inbounds nuw (i8, ptr @dst, i64 3), i8 -63, i64 600, i1 false)
  ret void
}

define internal void @mv600(ptr noundef nocapture nofree writeonly initializes((0, 600)) "llrm.reads"="" "llrm.writes"="0:600" %0, ptr noundef nocapture nofree readonly "llrm.reads"="0:600" "llrm.writes"="" %1) noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode willreturn nofree nosync norecurse memory(argmem: readwrite) {
  tail call void @llvm.memmove.p0.p0.i64(ptr align 1 %0, ptr align 1 %1, i64 600, i1 false)
  ret void
}

define internal void @lc1() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  %5 = load i8, ptr %4, align 1
  store i8 %5, ptr %3, align 1
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %7 = getelementptr inbounds nuw i8, ptr %1, i64 2
  %8 = load i8, ptr %7, align 2
  store i8 %8, ptr %6, align 1
  %9 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %9, i8 51, i64 1, i1 false)
  br label %11

10:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

11:
  %12 = phi i64 [ 0, %0 ], [ %26, %11 ]
  %13 = getelementptr inbounds nuw i8, ptr %1, i64 %12
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = getelementptr inbounds nuw i8, ptr %2, i64 %12
  %16 = load i8, ptr %15, align 2, !tbaa !10
  %17 = xor i8 %16, %14
  %18 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %12
  store i8 %17, ptr %18, align 2, !tbaa !10
  %19 = or disjoint i64 %12, 1
  %20 = getelementptr inbounds nuw i8, ptr %1, i64 %19
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = getelementptr inbounds nuw i8, ptr %2, i64 %19
  %23 = load i8, ptr %22, align 1, !tbaa !10
  %24 = xor i8 %23, %21
  %25 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %19
  store i8 %24, ptr %25, align 1, !tbaa !10
  %26 = add nuw nsw i64 %12, 2
  %27 = icmp eq i64 %26, 700
  br i1 %27, label %10, label %11, !llvm.loop !21
}

define internal void @lr1() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 1)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 1)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 1)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !23
}

define internal void @lc2() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  %5 = load i16, ptr %4, align 1
  store i16 %5, ptr %3, align 1
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %7 = getelementptr inbounds nuw i8, ptr %1, i64 2
  %8 = load i16, ptr %7, align 2
  store i16 %8, ptr %6, align 1
  %9 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %9, i8 51, i64 2, i1 false)
  br label %11

10:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

11:
  %12 = phi i64 [ 0, %0 ], [ %26, %11 ]
  %13 = getelementptr inbounds nuw i8, ptr %1, i64 %12
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = getelementptr inbounds nuw i8, ptr %2, i64 %12
  %16 = load i8, ptr %15, align 2, !tbaa !10
  %17 = xor i8 %16, %14
  %18 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %12
  store i8 %17, ptr %18, align 2, !tbaa !10
  %19 = or disjoint i64 %12, 1
  %20 = getelementptr inbounds nuw i8, ptr %1, i64 %19
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = getelementptr inbounds nuw i8, ptr %2, i64 %19
  %23 = load i8, ptr %22, align 1, !tbaa !10
  %24 = xor i8 %23, %21
  %25 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %19
  store i8 %24, ptr %25, align 1, !tbaa !10
  %26 = add nuw nsw i64 %12, 2
  %27 = icmp eq i64 %26, 700
  br i1 %27, label %10, label %11, !llvm.loop !25
}

define internal void @lr2() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 2)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 2)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 2)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !27
}

define internal void @lc3() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 3, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %4, ptr align 2 getelementptr inbounds nuw (i8, ptr @src, i64 2), i64 3, i1 false)
  %5 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %5, i8 51, i64 3, i1 false)
  br label %7

6:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

7:
  %8 = phi i64 [ 0, %0 ], [ %22, %7 ]
  %9 = getelementptr inbounds nuw i8, ptr %1, i64 %8
  %10 = load i8, ptr %9, align 2, !tbaa !10
  %11 = getelementptr inbounds nuw i8, ptr %2, i64 %8
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = xor i8 %12, %10
  %14 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %8
  store i8 %13, ptr %14, align 2, !tbaa !10
  %15 = or disjoint i64 %8, 1
  %16 = getelementptr inbounds nuw i8, ptr %1, i64 %15
  %17 = load i8, ptr %16, align 1, !tbaa !10
  %18 = getelementptr inbounds nuw i8, ptr %2, i64 %15
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = xor i8 %19, %17
  %21 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %15
  store i8 %20, ptr %21, align 1, !tbaa !10
  %22 = add nuw nsw i64 %8, 2
  %23 = icmp eq i64 %22, 700
  br i1 %23, label %6, label %7, !llvm.loop !29
}

define internal void @lr3() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 3)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 3)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 3)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !31
}

define internal void @lc5() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 5, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 5, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 5, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !33
}

define internal void @lr5() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 5)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 5)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 5)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !35
}

define internal void @lc7() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 7, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 7, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 7, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !37
}

define internal void @lr7() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 7)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 7)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 7)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !39
}

define internal void @lc8() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  %5 = load i64, ptr %4, align 1
  store i64 %5, ptr %3, align 1
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %7 = getelementptr inbounds nuw i8, ptr %1, i64 2
  %8 = load i64, ptr %7, align 2
  store i64 %8, ptr %6, align 1
  %9 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %9, i8 51, i64 8, i1 false)
  br label %11

10:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

11:
  %12 = phi i64 [ 0, %0 ], [ %26, %11 ]
  %13 = getelementptr inbounds nuw i8, ptr %1, i64 %12
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = getelementptr inbounds nuw i8, ptr %2, i64 %12
  %16 = load i8, ptr %15, align 2, !tbaa !10
  %17 = xor i8 %16, %14
  %18 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %12
  store i8 %17, ptr %18, align 2, !tbaa !10
  %19 = or disjoint i64 %12, 1
  %20 = getelementptr inbounds nuw i8, ptr %1, i64 %19
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = getelementptr inbounds nuw i8, ptr %2, i64 %19
  %23 = load i8, ptr %22, align 1, !tbaa !10
  %24 = xor i8 %23, %21
  %25 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %19
  store i8 %24, ptr %25, align 1, !tbaa !10
  %26 = add nuw nsw i64 %12, 2
  %27 = icmp eq i64 %26, 700
  br i1 %27, label %10, label %11, !llvm.loop !41
}

define internal void @lr8() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 8)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 8)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 8)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !43
}

define internal void @lc9() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 9, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 9, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 9, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !45
}

define internal void @lr9() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 9)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 9)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 9)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !47
}

define internal void @lc15() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 15, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 15, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 15, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !49
}

define internal void @lr15() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 15)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 15)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 15)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !51
}

define internal void @lc16() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 16, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 16, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 16, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !53
}

define internal void @lr16() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 16)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 16)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 16)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !55
}

define internal void @lc17() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 17, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 17, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 17, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !57
}

define internal void @lr17() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 17)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 17)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 17)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !59
}

define internal void @lc24() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 24, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 24, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 24, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !61
}

define internal void @lr24() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 24)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 24)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 24)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !63
}

define internal void @lc31() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 31, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 31, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 31, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !65
}

define internal void @lr31() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 31)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 31)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 31)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !67
}

define internal void @lc32() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 32, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 32, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 32, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !69
}

define internal void @lr32() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 32)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 32)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 32)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !71
}

define internal void @lc33() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 33, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 33, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 33, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !73
}

define internal void @lr33() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 33)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 33)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 33)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !75
}

define internal void @lc40() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 40, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 40, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !77
}

define internal void @lr40() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 40)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 40)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !79
}

define internal void @lc63() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 63, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 63, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !81
}

define internal void @lr63() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 63)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 63)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !83
}

define internal void @lc64() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 64, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 64, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !85
}

define internal void @lr64() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 64)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 64)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !87
}

define internal void @lc65() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 65, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 65, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !89
}

define internal void @lr65() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 65)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 65)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !91
}

define internal void @lc100() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 100, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 100, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !93
}

define internal void @lr100() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 100)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 100)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !95
}

define internal void @lc127() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 127, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 127, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !97
}

define internal void @lr127() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 127)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 127)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !99
}

define internal void @lc128() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 128, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 128, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !101
}

define internal void @lr128() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 128)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 128)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !103
}

define internal void @lc129() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 129, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 129, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !105
}

define internal void @lr129() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 129)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 129)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !107
}

define internal void @lc200() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 200, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 200, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !109
}

define internal void @lr200() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 200)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 200)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !111
}

define internal void @lc255() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 255, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 255, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !113
}

define internal void @lr255() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 255)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 255)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !115
}

define internal void @lc256() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 256, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 256, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !117
}

define internal void @lr256() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 256)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 256)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !119
}

define internal void @lc257() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 257, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 257, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !121
}

define internal void @lr257() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 257)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 257)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !123
}

define internal void @lc300() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 300, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 300, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !125
}

define internal void @lr300() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 300)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 300)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !127
}

define internal void @lc511() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 511, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 511, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !129
}

define internal void @lr511() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 511)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 511)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !131
}

define internal void @lc512() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 512, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 512, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !133
}

define internal void @lr512() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 512)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 512)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !135
}

define internal void @lc513() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 513, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 513, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !137
}

define internal void @lr513() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 513)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 513)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !139
}

define internal void @lc600() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none, inaccessiblemem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %3, ptr align 1 getelementptr inbounds nuw (i8, ptr @src, i64 1), i64 600, i1 false)
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @llvm.memmove.p0.p0.i64(ptr nonnull align 1 %4, ptr nonnull align 2 %5, i64 600, i1 false)
  %6 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @llvm.memset.p0.i64(ptr nonnull align 2 %6, i8 51, i64 40, i1 false)
  br label %8

7:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

8:
  %9 = phi i64 [ 0, %0 ], [ %23, %8 ]
  %10 = getelementptr inbounds nuw i8, ptr %1, i64 %9
  %11 = load i8, ptr %10, align 2, !tbaa !10
  %12 = getelementptr inbounds nuw i8, ptr %2, i64 %9
  %13 = load i8, ptr %12, align 2, !tbaa !10
  %14 = xor i8 %13, %11
  %15 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %9
  store i8 %14, ptr %15, align 2, !tbaa !10
  %16 = or disjoint i64 %9, 1
  %17 = getelementptr inbounds nuw i8, ptr %1, i64 %16
  %18 = load i8, ptr %17, align 1, !tbaa !10
  %19 = getelementptr inbounds nuw i8, ptr %2, i64 %16
  %20 = load i8, ptr %19, align 1, !tbaa !10
  %21 = xor i8 %20, %18
  %22 = getelementptr inbounds nuw i8, ptr @check_locals.x, i64 %16
  store i8 %21, ptr %22, align 1, !tbaa !10
  %23 = add nuw nsw i64 %9, 2
  %24 = icmp eq i64 %23, 700
  br i1 %24, label %7, label %8, !llvm.loop !141
}

define internal void @lr600() noinline nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" nearcode nofree nosync norecurse willreturn memory(readwrite, argmem: none) {
  %1 = alloca [700 x i8], align 16
  %2 = alloca [700 x i8], align 16
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %1) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.memset.p0.i64(ptr nonnull align 16 %2, i8 -18, i64 700, i1 false)
  call void @llvm.memcpy.p0.p0.i64(ptr nonnull align 16 %1, ptr align 16 @src, i64 700, i1 false)
  %3 = getelementptr inbounds nuw i8, ptr %2, i64 3
  %4 = getelementptr inbounds nuw i8, ptr %1, i64 1
  call void @ref_copy(ptr noundef nonnull %3, ptr noundef nonnull %4, i32 noundef 600)
  %5 = getelementptr inbounds nuw i8, ptr %1, i64 5
  %6 = getelementptr inbounds nuw i8, ptr %1, i64 2
  call void @ref_move(ptr noundef nonnull %5, ptr noundef nonnull %6, i32 noundef 600)
  %7 = getelementptr inbounds nuw i8, ptr %2, i64 650
  call void @ref_fill(ptr noundef nonnull %7, i32 noundef 51, i32 noundef 40)
  br label %9

8:
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %2) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr nonnull %1) nounwind
  ret void

9:
  %10 = phi i64 [ 0, %0 ], [ %24, %9 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i64 %10
  %12 = load i8, ptr %11, align 2, !tbaa !10
  %13 = getelementptr inbounds nuw i8, ptr %2, i64 %10
  %14 = load i8, ptr %13, align 2, !tbaa !10
  %15 = xor i8 %14, %12
  %16 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %10
  store i8 %15, ptr %16, align 2, !tbaa !10
  %17 = or disjoint i64 %10, 1
  %18 = getelementptr inbounds nuw i8, ptr %1, i64 %17
  %19 = load i8, ptr %18, align 1, !tbaa !10
  %20 = getelementptr inbounds nuw i8, ptr %2, i64 %17
  %21 = load i8, ptr %20, align 1, !tbaa !10
  %22 = xor i8 %21, %19
  %23 = getelementptr inbounds nuw i8, ptr @check_locals.y, i64 %17
  store i8 %22, ptr %23, align 1, !tbaa !10
  %24 = add nuw nsw i64 %10, 2
  %25 = icmp eq i64 %24, 700
  br i1 %25, label %8, label %9, !llvm.loop !143
}

declare void @report(i64 noundef) "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic"

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memcpy.p0.p0.i64(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i64, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare void @llvm.memset.p0.i64(ptr nocapture writeonly, i8, i64, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memmove.p0.p0.i64(ptr nocapture writeonly, ptr nocapture readonly, i64, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare void @llvm.assume(i1 noundef) nocallback nofree nosync nounwind willreturn memory(inaccessiblemem: write)

!llvm.module.flags = !{!0, !1, !2}
!llvm.ident = !{!3}
!llvm.errno.tbaa = !{!4}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{i32 7, !"uwtable", i32 2}
!3 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!4 = !{!5, !6, i64 0}
!5 = !{!"__libc_errno", !6, i64 0}
!6 = !{!"int", !7, i64 0}
!7 = !{!"omnipotent char", !8, i64 0}
!8 = !{!"Simple C/C++ TBAA"}
!9 = !{!6, !6, i64 0}
!10 = !{!7, !7, i64 0}
!11 = distinct !{!11, !12}
!12 = !{!"llvm.loop.mustprogress"}
!13 = distinct !{!13, !12}
!14 = !{!15, !15, i64 0}
!15 = !{!"p1 omnipotent char", !16, i64 0}
!16 = !{!"any pointer", !7, i64 0}
!17 = distinct !{!17, !12}
!18 = distinct !{!18, !12}
!19 = !{!16, !16, i64 0}
!20 = distinct !{!20, !12}
!21 = distinct !{!21, !12}
!22 = distinct !{!22, !12}
!23 = distinct !{!23, !12}
!24 = distinct !{!24, !12}
!25 = distinct !{!25, !12}
!26 = distinct !{!26, !12}
!27 = distinct !{!27, !12}
!28 = distinct !{!28, !12}
!29 = distinct !{!29, !12}
!30 = distinct !{!30, !12}
!31 = distinct !{!31, !12}
!32 = distinct !{!32, !12}
!33 = distinct !{!33, !12}
!34 = distinct !{!34, !12}
!35 = distinct !{!35, !12}
!36 = distinct !{!36, !12}
!37 = distinct !{!37, !12}
!38 = distinct !{!38, !12}
!39 = distinct !{!39, !12}
!40 = distinct !{!40, !12}
!41 = distinct !{!41, !12}
!42 = distinct !{!42, !12}
!43 = distinct !{!43, !12}
!44 = distinct !{!44, !12}
!45 = distinct !{!45, !12}
!46 = distinct !{!46, !12}
!47 = distinct !{!47, !12}
!48 = distinct !{!48, !12}
!49 = distinct !{!49, !12}
!50 = distinct !{!50, !12}
!51 = distinct !{!51, !12}
!52 = distinct !{!52, !12}
!53 = distinct !{!53, !12}
!54 = distinct !{!54, !12}
!55 = distinct !{!55, !12}
!56 = distinct !{!56, !12}
!57 = distinct !{!57, !12}
!58 = distinct !{!58, !12}
!59 = distinct !{!59, !12}
!60 = distinct !{!60, !12}
!61 = distinct !{!61, !12}
!62 = distinct !{!62, !12}
!63 = distinct !{!63, !12}
!64 = distinct !{!64, !12}
!65 = distinct !{!65, !12}
!66 = distinct !{!66, !12}
!67 = distinct !{!67, !12}
!68 = distinct !{!68, !12}
!69 = distinct !{!69, !12}
!70 = distinct !{!70, !12}
!71 = distinct !{!71, !12}
!72 = distinct !{!72, !12}
!73 = distinct !{!73, !12}
!74 = distinct !{!74, !12}
!75 = distinct !{!75, !12}
!76 = distinct !{!76, !12}
!77 = distinct !{!77, !12}
!78 = distinct !{!78, !12}
!79 = distinct !{!79, !12}
!80 = distinct !{!80, !12}
!81 = distinct !{!81, !12}
!82 = distinct !{!82, !12}
!83 = distinct !{!83, !12}
!84 = distinct !{!84, !12}
!85 = distinct !{!85, !12}
!86 = distinct !{!86, !12}
!87 = distinct !{!87, !12}
!88 = distinct !{!88, !12}
!89 = distinct !{!89, !12}
!90 = distinct !{!90, !12}
!91 = distinct !{!91, !12}
!92 = distinct !{!92, !12}
!93 = distinct !{!93, !12}
!94 = distinct !{!94, !12}
!95 = distinct !{!95, !12}
!96 = distinct !{!96, !12}
!97 = distinct !{!97, !12}
!98 = distinct !{!98, !12}
!99 = distinct !{!99, !12}
!100 = distinct !{!100, !12}
!101 = distinct !{!101, !12}
!102 = distinct !{!102, !12}
!103 = distinct !{!103, !12}
!104 = distinct !{!104, !12}
!105 = distinct !{!105, !12}
!106 = distinct !{!106, !12}
!107 = distinct !{!107, !12}
!108 = distinct !{!108, !12}
!109 = distinct !{!109, !12}
!110 = distinct !{!110, !12}
!111 = distinct !{!111, !12}
!112 = distinct !{!112, !12}
!113 = distinct !{!113, !12}
!114 = distinct !{!114, !12}
!115 = distinct !{!115, !12}
!116 = distinct !{!116, !12}
!117 = distinct !{!117, !12}
!118 = distinct !{!118, !12}
!119 = distinct !{!119, !12}
!120 = distinct !{!120, !12}
!121 = distinct !{!121, !12}
!122 = distinct !{!122, !12}
!123 = distinct !{!123, !12}
!124 = distinct !{!124, !12}
!125 = distinct !{!125, !12}
!126 = distinct !{!126, !12}
!127 = distinct !{!127, !12}
!128 = distinct !{!128, !12}
!129 = distinct !{!129, !12}
!130 = distinct !{!130, !12}
!131 = distinct !{!131, !12}
!132 = distinct !{!132, !12}
!133 = distinct !{!133, !12}
!134 = distinct !{!134, !12}
!135 = distinct !{!135, !12}
!136 = distinct !{!136, !12}
!137 = distinct !{!137, !12}
!138 = distinct !{!138, !12}
!139 = distinct !{!139, !12}
!140 = distinct !{!140, !12}
!141 = distinct !{!141, !12}
!142 = distinct !{!142, !12}
!143 = distinct !{!143, !12}
!144 = distinct !{!144, !12}
