; RUN: llrm-llc -O2 -m32 -march=i486 -fobject-format=omf -mabi=sysv %s -o /dev/null
; A split piece of `i - 3` (a lea of a register) was made again from the live def, but with the operands of the
; snapshot taken before `i` was split: the copy named a register nothing defined, and the object writer refused
; the lea (tests/run/c/idioms.c, x86_32-dos hosted, -O2). The RUN line is the check: it must compile.
target datalayout = "e-p:32:32-i8:8-i16:16-i32:32-i64:32-f32:32-f64:32-f80:32-n8:16:32-S32"

@_main.ns = internal unnamed_addr constant [6 x i16] [i16 0, i16 1, i16 2, i16 17, i16 64, i16 100], align 2
@_a = global [160 x i16] zeroinitializer, align 2
@_pal = global [64 x i16] zeroinitializer, align 2
@_c = global [160 x i16] zeroinitializer, align 2
@_b = global [160 x i16] zeroinitializer, align 2
@_L = global [40 x i32] zeroinitializer, align 4
@_M = global [40 x i32] zeroinitializer, align 4

; Function Attrs: nounwind
define cc1003 i32 @_main() #0 {
  %1 = getelementptr i8, ptr @_a, i32 2
  br label %2

2:                                                ; preds = %527, %0
  %3 = phi i32 [ 0, %0 ], [ %528, %527 ]
  %4 = getelementptr inbounds nuw [2 x i8], ptr @_main.ns, i32 %3
  %5 = load i16, ptr %4, align 2, !tbaa !7
  %wide.trip.count1 = zext nneg i16 %5 to i32
  %6 = zext i16 %5 to i32
  %7 = zext i16 %5 to i32
  %8 = zext i16 %5 to i32
  %wide.trip.count3 = zext nneg i16 %5 to i32
  %wide.trip.count4 = zext nneg i16 %5 to i32
  %9 = zext i16 %5 to i32
  %10 = zext i16 %5 to i32
  %11 = zext i16 %5 to i32
  %12 = sext i16 %5 to i32
  br label %13

13:                                               ; preds = %13, %2
  %14 = phi i16 [ %29, %13 ], [ 1, %2 ]
  %15 = phi i16 [ %28, %13 ], [ 2, %2 ]
  %16 = phi i32 [ %math, %13 ], [ -160, %2 ]
  %17 = shl nsw i32 %16, 1
  %18 = getelementptr i8, ptr @_a, i32 %17
  %19 = getelementptr i8, ptr %18, i32 320
  %20 = add i32 %16, 1160
  %tmp = trunc i32 %20 to i16
  store i16 %tmp, ptr %19, align 2, !tbaa !7
  %21 = shl nsw i32 %16, 1
  %22 = getelementptr i8, ptr @_b, i32 %21
  %23 = getelementptr i8, ptr %22, i32 320
  store i16 %14, ptr %23, align 2, !tbaa !7
  %24 = shl nsw i32 %16, 1
  %25 = getelementptr i8, ptr @_c, i32 %24
  %26 = getelementptr i8, ptr %25, i32 320
  store i16 %15, ptr %26, align 2, !tbaa !7
  %27 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %16, i32 1)
  %math = extractvalue { i32, i1 } %27, 0
  %ov = extractvalue { i32, i1 } %27, 1
  %28 = add nuw nsw i16 %15, 5
  %29 = add nuw nsw i16 %14, 3
  br i1 %ov, label %30, label %13, !llvm.loop !9

30:                                               ; preds = %13
  %31 = trunc i32 %wide.trip.count1 to i16
  %32 = sdiv i16 %31, 10
  %33 = trunc i32 %3 to i16
  %34 = add i16 %33, -3
  %35 = icmp ult i16 %34, 3
  br i1 %35, label %36, label %530

36:                                               ; preds = %30
  %wide.trip.count6 = zext i16 %32 to i32
  %37 = shl nuw nsw i32 %wide.trip.count6, 5
  br label %38

38:                                               ; preds = %38, %36
  %39 = phi i32 [ %71, %38 ], [ 0, %36 ]
  %40 = getelementptr i8, ptr @_a, i32 %39
  store i16 4660, ptr %40, align 2, !tbaa !7
  %41 = getelementptr i8, ptr @_a, i32 %39
  %42 = getelementptr i8, ptr %41, i32 2
  store i16 4660, ptr %42, align 2, !tbaa !7
  %43 = getelementptr i8, ptr @_a, i32 %39
  %44 = getelementptr i8, ptr %43, i32 4
  store i16 4660, ptr %44, align 2, !tbaa !7
  %45 = getelementptr i8, ptr @_a, i32 %39
  %46 = getelementptr i8, ptr %45, i32 6
  store i16 4660, ptr %46, align 2, !tbaa !7
  %47 = getelementptr i8, ptr @_a, i32 %39
  %48 = getelementptr i8, ptr %47, i32 8
  store i16 4660, ptr %48, align 2, !tbaa !7
  %49 = getelementptr i8, ptr @_a, i32 %39
  %50 = getelementptr i8, ptr %49, i32 10
  store i16 4660, ptr %50, align 2, !tbaa !7
  %51 = getelementptr i8, ptr @_a, i32 %39
  %52 = getelementptr i8, ptr %51, i32 12
  store i16 4660, ptr %52, align 2, !tbaa !7
  %53 = getelementptr i8, ptr @_a, i32 %39
  %54 = getelementptr i8, ptr %53, i32 14
  store i16 4660, ptr %54, align 2, !tbaa !7
  %55 = getelementptr i8, ptr @_a, i32 %39
  %56 = getelementptr i8, ptr %55, i32 16
  store i16 4660, ptr %56, align 2, !tbaa !7
  %57 = getelementptr i8, ptr @_a, i32 %39
  %58 = getelementptr i8, ptr %57, i32 18
  store i16 4660, ptr %58, align 2, !tbaa !7
  %59 = getelementptr i8, ptr @_a, i32 %39
  %60 = getelementptr i8, ptr %59, i32 20
  store i16 4660, ptr %60, align 2, !tbaa !7
  %61 = getelementptr i8, ptr @_a, i32 %39
  %62 = getelementptr i8, ptr %61, i32 22
  store i16 4660, ptr %62, align 2, !tbaa !7
  %63 = getelementptr i8, ptr @_a, i32 %39
  %64 = getelementptr i8, ptr %63, i32 24
  store i16 4660, ptr %64, align 2, !tbaa !7
  %65 = getelementptr i8, ptr @_a, i32 %39
  %66 = getelementptr i8, ptr %65, i32 26
  store i16 4660, ptr %66, align 2, !tbaa !7
  %67 = getelementptr i8, ptr @_a, i32 %39
  %68 = getelementptr i8, ptr %67, i32 28
  store i16 4660, ptr %68, align 2, !tbaa !7
  %69 = getelementptr i8, ptr @_a, i32 %39
  %70 = getelementptr i8, ptr %69, i32 30
  store i16 4660, ptr %70, align 2, !tbaa !7
  %71 = add nuw i32 %39, 32
  %exitcond110 = icmp eq i32 %37, %71
  br i1 %exitcond110, label %530, label %38, !llvm.loop !11

72:                                               ; preds = %530, %72
  %73 = phi i32 [ 2, %530 ], [ %82, %72 ]
  %74 = phi i32 [ %81, %72 ], [ %532, %530 ]
  %75 = shl nuw nsw i32 %73, 1
  %76 = getelementptr i8, ptr @_a, i32 %75
  %77 = getelementptr i8, ptr %76, i32 -2
  %78 = load i16, ptr %77, align 2, !tbaa !7
  %79 = sext i16 %78 to i32
  %80 = mul nsw i32 %73, %79
  %81 = add nsw i32 %80, %74
  %82 = add nuw nsw i32 %73, 1
  %exitcond29 = icmp eq i32 %82, 161
  br i1 %exitcond29, label %83, label %72, !llvm.loop !12

83:                                               ; preds = %72
  %84 = trunc i32 %wide.trip.count1 to i16
  tail call cc1003 void @_report(i32 noundef %81)
  tail call void @llvm.memset.p0.i32(ptr align 2 @_pal, i8 -1, i32 128, i1 false)
  %85 = shl i16 %84, 8
  %86 = or disjoint i16 %85, 52
  %87 = icmp eq i32 %3, 0
  br i1 %87, label %533, label %88

88:                                               ; preds = %83
  %89 = trunc i32 %wide.trip.count1 to i16
  %90 = tail call i16 @llvm.smin.i16(i16 %89, i16 64)
  %wide.trip.count5 = zext nneg i16 %90 to i32
  %91 = mul nsw i32 %wide.trip.count5, -1
  %92 = shl nuw nsw i32 %wide.trip.count5, 1
  %93 = getelementptr i8, ptr @_pal, i32 %92
  br label %94

94:                                               ; preds = %94, %88
  %95 = phi i32 [ %math1, %94 ], [ %91, %88 ]
  %96 = shl i32 %95, 1
  %97 = getelementptr i8, ptr %93, i32 %96
  store i16 %86, ptr %97, align 2, !tbaa !7
  %98 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %95, i32 1)
  %math1 = extractvalue { i32, i1 } %98, 0
  %ov1 = extractvalue { i32, i1 } %98, 1
  br i1 %ov1, label %533, label %94, !llvm.loop !13

99:                                               ; preds = %533, %99
  %100 = phi i32 [ 2, %533 ], [ %109, %99 ]
  %101 = phi i32 [ %108, %99 ], [ %535, %533 ]
  %102 = shl nuw nsw i32 %100, 1
  %103 = getelementptr i8, ptr @_pal, i32 %102
  %104 = getelementptr i8, ptr %103, i32 -2
  %105 = load i16, ptr %104, align 2, !tbaa !7
  %106 = sext i16 %105 to i32
  %107 = mul nsw i32 %100, %106
  %108 = add nsw i32 %107, %101
  %109 = add nuw nsw i32 %100, 1
  %exitcond27 = icmp eq i32 %109, 65
  br i1 %exitcond27, label %110, label %99, !llvm.loop !12

110:                                              ; preds = %99
  tail call cc1003 void @_report(i32 noundef %108)
  br label %111

111:                                              ; preds = %111, %110
  %112 = phi i16 [ %127, %111 ], [ 1, %110 ]
  %113 = phi i16 [ %126, %111 ], [ 2, %110 ]
  %114 = phi i32 [ %math2, %111 ], [ -160, %110 ]
  %115 = shl nsw i32 %114, 1
  %116 = getelementptr i8, ptr @_a, i32 %115
  %117 = getelementptr i8, ptr %116, i32 320
  %118 = add i32 %114, 1160
  %tmp1 = trunc i32 %118 to i16
  store i16 %tmp1, ptr %117, align 2, !tbaa !7
  %119 = shl nsw i32 %114, 1
  %120 = getelementptr i8, ptr @_b, i32 %119
  %121 = getelementptr i8, ptr %120, i32 320
  store i16 %112, ptr %121, align 2, !tbaa !7
  %122 = shl nsw i32 %114, 1
  %123 = getelementptr i8, ptr @_c, i32 %122
  %124 = getelementptr i8, ptr %123, i32 320
  store i16 %113, ptr %124, align 2, !tbaa !7
  %125 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %114, i32 1)
  %math2 = extractvalue { i32, i1 } %125, 0
  %ov2 = extractvalue { i32, i1 } %125, 1
  %126 = add nuw nsw i16 %113, 5
  %127 = add nuw nsw i16 %112, 3
  br i1 %ov2, label %128, label %111, !llvm.loop !9

128:                                              ; preds = %111
  %129 = icmp eq i32 %3, 0
  br i1 %129, label %536, label %130

130:                                              ; preds = %128
  %131 = shl nuw i32 %11, 1
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 2 @_a, ptr align 2 @_b, i32 %131, i1 false)
  br label %536

132:                                              ; preds = %536, %132
  %133 = phi i32 [ 2, %536 ], [ %142, %132 ]
  %134 = phi i32 [ %141, %132 ], [ %538, %536 ]
  %135 = shl nuw nsw i32 %133, 1
  %136 = getelementptr i8, ptr @_a, i32 %135
  %137 = getelementptr i8, ptr %136, i32 -2
  %138 = load i16, ptr %137, align 2, !tbaa !7
  %139 = sext i16 %138 to i32
  %140 = mul nsw i32 %133, %139
  %141 = add nsw i32 %140, %134
  %142 = add nuw nsw i32 %133, 1
  %exitcond25 = icmp eq i32 %142, 161
  br i1 %exitcond25, label %143, label %132, !llvm.loop !12

143:                                              ; preds = %132
  tail call cc1003 void @_report(i32 noundef %141)
  br label %144

144:                                              ; preds = %144, %143
  %145 = phi i16 [ %160, %144 ], [ 1, %143 ]
  %146 = phi i16 [ %159, %144 ], [ 2, %143 ]
  %147 = phi i32 [ %math3, %144 ], [ -160, %143 ]
  %148 = shl nsw i32 %147, 1
  %149 = getelementptr i8, ptr @_a, i32 %148
  %150 = getelementptr i8, ptr %149, i32 320
  %151 = add i32 %147, 1160
  %tmp2 = trunc i32 %151 to i16
  store i16 %tmp2, ptr %150, align 2, !tbaa !7
  %152 = shl nsw i32 %147, 1
  %153 = getelementptr i8, ptr @_b, i32 %152
  %154 = getelementptr i8, ptr %153, i32 320
  store i16 %145, ptr %154, align 2, !tbaa !7
  %155 = shl nsw i32 %147, 1
  %156 = getelementptr i8, ptr @_c, i32 %155
  %157 = getelementptr i8, ptr %156, i32 320
  store i16 %146, ptr %157, align 2, !tbaa !7
  %158 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %147, i32 1)
  %math3 = extractvalue { i32, i1 } %158, 0
  %ov3 = extractvalue { i32, i1 } %158, 1
  %159 = add nuw nsw i16 %146, 5
  %160 = add nuw nsw i16 %145, 3
  br i1 %ov3, label %161, label %144, !llvm.loop !9

161:                                              ; preds = %144
  %162 = icmp eq i32 %3, 0
  br i1 %162, label %539, label %163

163:                                              ; preds = %161
  %164 = shl nuw i32 %10, 1
  tail call void @llvm.memmove.p0.p0.i32(ptr align 2 @_a, ptr align 2 getelementptr inbounds nuw (i8, ptr @_a, i32 32), i32 %164, i1 false), !llrm.forward !14
  br label %539

165:                                              ; preds = %539, %165
  %166 = phi i32 [ 2, %539 ], [ %175, %165 ]
  %167 = phi i32 [ %174, %165 ], [ %541, %539 ]
  %168 = shl nuw nsw i32 %166, 1
  %169 = getelementptr i8, ptr @_a, i32 %168
  %170 = getelementptr i8, ptr %169, i32 -2
  %171 = load i16, ptr %170, align 2, !tbaa !7
  %172 = sext i16 %171 to i32
  %173 = mul nsw i32 %166, %172
  %174 = add nsw i32 %173, %167
  %175 = add nuw nsw i32 %166, 1
  %exitcond23 = icmp eq i32 %175, 161
  br i1 %exitcond23, label %176, label %165, !llvm.loop !12

176:                                              ; preds = %165
  tail call cc1003 void @_report(i32 noundef %174)
  br label %177

177:                                              ; preds = %177, %176
  %178 = phi i16 [ %193, %177 ], [ 1, %176 ]
  %179 = phi i16 [ %192, %177 ], [ 2, %176 ]
  %180 = phi i32 [ %math4, %177 ], [ -160, %176 ]
  %181 = shl nsw i32 %180, 1
  %182 = getelementptr i8, ptr @_a, i32 %181
  %183 = getelementptr i8, ptr %182, i32 320
  %184 = add i32 %180, 1160
  %tmp3 = trunc i32 %184 to i16
  store i16 %tmp3, ptr %183, align 2, !tbaa !7
  %185 = shl nsw i32 %180, 1
  %186 = getelementptr i8, ptr @_b, i32 %185
  %187 = getelementptr i8, ptr %186, i32 320
  store i16 %178, ptr %187, align 2, !tbaa !7
  %188 = shl nsw i32 %180, 1
  %189 = getelementptr i8, ptr @_c, i32 %188
  %190 = getelementptr i8, ptr %189, i32 320
  store i16 %179, ptr %190, align 2, !tbaa !7
  %191 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %180, i32 1)
  %math4 = extractvalue { i32, i1 } %191, 0
  %ov4 = extractvalue { i32, i1 } %191, 1
  %192 = add nuw nsw i16 %179, 5
  %193 = add nuw nsw i16 %178, 3
  br i1 %ov4, label %194, label %177, !llvm.loop !9

194:                                              ; preds = %177
  %195 = trunc i32 %wide.trip.count1 to i16
  %196 = add i16 %195, -1
  %197 = icmp sgt i16 %196, -1
  br i1 %197, label %198, label %542

198:                                              ; preds = %194
  %199 = shl nuw nsw i32 %9, 1
  tail call void @llvm.memmove.p0.p0.i32(ptr align 2 getelementptr inbounds nuw (i8, ptr @_a, i32 32), ptr align 2 @_a, i32 %199, i1 false), !llrm.backward !14
  br label %542

200:                                              ; preds = %542, %200
  %201 = phi i32 [ 2, %542 ], [ %210, %200 ]
  %202 = phi i32 [ %209, %200 ], [ %544, %542 ]
  %203 = shl nuw nsw i32 %201, 1
  %204 = getelementptr i8, ptr @_a, i32 %203
  %205 = getelementptr i8, ptr %204, i32 -2
  %206 = load i16, ptr %205, align 2, !tbaa !7
  %207 = sext i16 %206 to i32
  %208 = mul nsw i32 %201, %207
  %209 = add nsw i32 %208, %202
  %210 = add nuw nsw i32 %201, 1
  %exitcond21 = icmp eq i32 %210, 161
  br i1 %exitcond21, label %211, label %200, !llvm.loop !12

211:                                              ; preds = %200
  tail call cc1003 void @_report(i32 noundef %209)
  br label %212

212:                                              ; preds = %212, %211
  %213 = phi i16 [ %228, %212 ], [ 1, %211 ]
  %214 = phi i16 [ %227, %212 ], [ 2, %211 ]
  %215 = phi i32 [ %math5, %212 ], [ -160, %211 ]
  %216 = shl nsw i32 %215, 1
  %217 = getelementptr i8, ptr @_a, i32 %216
  %218 = getelementptr i8, ptr %217, i32 320
  %219 = add i32 %215, 1160
  %tmp4 = trunc i32 %219 to i16
  store i16 %tmp4, ptr %218, align 2, !tbaa !7
  %220 = shl nsw i32 %215, 1
  %221 = getelementptr i8, ptr @_b, i32 %220
  %222 = getelementptr i8, ptr %221, i32 320
  store i16 %213, ptr %222, align 2, !tbaa !7
  %223 = shl nsw i32 %215, 1
  %224 = getelementptr i8, ptr @_c, i32 %223
  %225 = getelementptr i8, ptr %224, i32 320
  store i16 %214, ptr %225, align 2, !tbaa !7
  %226 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %215, i32 1)
  %math5 = extractvalue { i32, i1 } %226, 0
  %ov5 = extractvalue { i32, i1 } %226, 1
  %227 = add nuw nsw i16 %214, 5
  %228 = add nuw nsw i16 %213, 3
  br i1 %ov5, label %229, label %212, !llvm.loop !9

229:                                              ; preds = %212
  %230 = icmp eq i32 %3, 0
  br i1 %230, label %545, label %231

231:                                              ; preds = %229
  %load_initial = load i16, ptr @_a, align 2
  %232 = mul nsw i32 %wide.trip.count4, -1
  %233 = shl nuw nsw i32 %wide.trip.count4, 1
  %234 = getelementptr i8, ptr %1, i32 %233
  br label %235

235:                                              ; preds = %235, %231
  %236 = phi i32 [ %math6, %235 ], [ %232, %231 ]
  %237 = shl i32 %236, 1
  %238 = getelementptr i8, ptr %234, i32 %237
  store i16 %load_initial, ptr %238, align 2, !tbaa !7
  %239 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %236, i32 1)
  %math6 = extractvalue { i32, i1 } %239, 0
  %ov6 = extractvalue { i32, i1 } %239, 1
  br i1 %ov6, label %545, label %235, !llvm.loop !15

240:                                              ; preds = %545, %240
  %241 = phi i32 [ 2, %545 ], [ %250, %240 ]
  %242 = phi i32 [ %249, %240 ], [ %547, %545 ]
  %243 = shl nuw nsw i32 %241, 1
  %244 = getelementptr i8, ptr @_a, i32 %243
  %245 = getelementptr i8, ptr %244, i32 -2
  %246 = load i16, ptr %245, align 2, !tbaa !7
  %247 = sext i16 %246 to i32
  %248 = mul nsw i32 %241, %247
  %249 = add nsw i32 %248, %242
  %250 = add nuw nsw i32 %241, 1
  %exitcond18 = icmp eq i32 %250, 161
  br i1 %exitcond18, label %251, label %240, !llvm.loop !12

251:                                              ; preds = %240
  tail call cc1003 void @_report(i32 noundef %249)
  br label %252

252:                                              ; preds = %252, %251
  %253 = phi i16 [ %268, %252 ], [ 1, %251 ]
  %254 = phi i16 [ %267, %252 ], [ 2, %251 ]
  %255 = phi i32 [ %math7, %252 ], [ -160, %251 ]
  %256 = shl nsw i32 %255, 1
  %257 = getelementptr i8, ptr @_a, i32 %256
  %258 = getelementptr i8, ptr %257, i32 320
  %259 = add i32 %255, 1160
  %tmp5 = trunc i32 %259 to i16
  store i16 %tmp5, ptr %258, align 2, !tbaa !7
  %260 = shl nsw i32 %255, 1
  %261 = getelementptr i8, ptr @_b, i32 %260
  %262 = getelementptr i8, ptr %261, i32 320
  store i16 %253, ptr %262, align 2, !tbaa !7
  %263 = shl nsw i32 %255, 1
  %264 = getelementptr i8, ptr @_c, i32 %263
  %265 = getelementptr i8, ptr %264, i32 320
  store i16 %254, ptr %265, align 2, !tbaa !7
  %266 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %255, i32 1)
  %math7 = extractvalue { i32, i1 } %266, 0
  %ov7 = extractvalue { i32, i1 } %266, 1
  %267 = add nuw nsw i16 %254, 5
  %268 = add nuw nsw i16 %253, 3
  br i1 %ov7, label %269, label %252, !llvm.loop !9

269:                                              ; preds = %252
  %270 = icmp eq i32 %3, 0
  br i1 %270, label %548, label %271

271:                                              ; preds = %269
  %272 = mul nsw i32 %wide.trip.count3, -1
  %273 = shl nuw nsw i32 %wide.trip.count3, 1
  br label %274

274:                                              ; preds = %274, %271
  %275 = phi i32 [ %math8, %274 ], [ %272, %271 ]
  %276 = shl i32 %275, 1
  %277 = add i32 %273, %276
  %278 = getelementptr i8, ptr @_b, i32 %277
  %279 = load i16, ptr %278, align 2, !tbaa !7
  %280 = add i16 %279, 1
  %281 = shl i32 %275, 1
  %282 = add i32 %273, %281
  %283 = getelementptr i8, ptr @_a, i32 %282
  store i16 %280, ptr %283, align 2, !tbaa !7
  %284 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %275, i32 1)
  %math8 = extractvalue { i32, i1 } %284, 0
  %ov8 = extractvalue { i32, i1 } %284, 1
  br i1 %ov8, label %548, label %274, !llvm.loop !16

285:                                              ; preds = %548, %285
  %286 = phi i32 [ 2, %548 ], [ %295, %285 ]
  %287 = phi i32 [ %294, %285 ], [ %550, %548 ]
  %288 = shl nuw nsw i32 %286, 1
  %289 = getelementptr i8, ptr @_a, i32 %288
  %290 = getelementptr i8, ptr %289, i32 -2
  %291 = load i16, ptr %290, align 2, !tbaa !7
  %292 = sext i16 %291 to i32
  %293 = mul nsw i32 %286, %292
  %294 = add nsw i32 %293, %287
  %295 = add nuw nsw i32 %286, 1
  %exitcond15 = icmp eq i32 %295, 161
  br i1 %exitcond15, label %296, label %285, !llvm.loop !12

296:                                              ; preds = %285
  tail call cc1003 void @_report(i32 noundef %294)
  br label %297

297:                                              ; preds = %297, %296
  %298 = phi i16 [ %313, %297 ], [ 1, %296 ]
  %299 = phi i16 [ %312, %297 ], [ 2, %296 ]
  %300 = phi i32 [ %math9, %297 ], [ -160, %296 ]
  %301 = shl nsw i32 %300, 1
  %302 = getelementptr i8, ptr @_a, i32 %301
  %303 = getelementptr i8, ptr %302, i32 320
  %304 = add i32 %300, 1160
  %tmp6 = trunc i32 %304 to i16
  store i16 %tmp6, ptr %303, align 2, !tbaa !7
  %305 = shl nsw i32 %300, 1
  %306 = getelementptr i8, ptr @_b, i32 %305
  %307 = getelementptr i8, ptr %306, i32 320
  store i16 %298, ptr %307, align 2, !tbaa !7
  %308 = shl nsw i32 %300, 1
  %309 = getelementptr i8, ptr @_c, i32 %308
  %310 = getelementptr i8, ptr %309, i32 320
  store i16 %299, ptr %310, align 2, !tbaa !7
  %311 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %300, i32 1)
  %math9 = extractvalue { i32, i1 } %311, 0
  %ov9 = extractvalue { i32, i1 } %311, 1
  %312 = add nuw nsw i16 %299, 5
  %313 = add nuw nsw i16 %298, 3
  br i1 %ov9, label %314, label %297, !llvm.loop !9

314:                                              ; preds = %297
  %315 = icmp eq i32 %3, 0
  br i1 %315, label %551, label %316

316:                                              ; preds = %314
  %317 = shl nuw i32 %8, 1
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 2 @_a, ptr align 2 @_b, i32 %317, i1 false)
  tail call void @llvm.memset.p0.i32(ptr align 2 @_c, i8 0, i32 %317, i1 false)
  br label %551

318:                                              ; preds = %551, %318
  %319 = phi i32 [ 2, %551 ], [ %328, %318 ]
  %320 = phi i32 [ %327, %318 ], [ %553, %551 ]
  %321 = shl nuw nsw i32 %319, 1
  %322 = getelementptr i8, ptr @_a, i32 %321
  %323 = getelementptr i8, ptr %322, i32 -2
  %324 = load i16, ptr %323, align 2, !tbaa !7
  %325 = sext i16 %324 to i32
  %326 = mul nsw i32 %319, %325
  %327 = add nsw i32 %326, %320
  %328 = add nuw nsw i32 %319, 1
  %exitcond13 = icmp eq i32 %328, 161
  br i1 %exitcond13, label %554, label %318, !llvm.loop !12

329:                                              ; preds = %554, %329
  %330 = phi i32 [ 2, %554 ], [ %339, %329 ]
  %331 = phi i32 [ %338, %329 ], [ %556, %554 ]
  %332 = shl nuw nsw i32 %330, 1
  %333 = getelementptr i8, ptr @_c, i32 %332
  %334 = getelementptr i8, ptr %333, i32 -2
  %335 = load i16, ptr %334, align 2, !tbaa !7
  %336 = sext i16 %335 to i32
  %337 = mul nsw i32 %330, %336
  %338 = add nsw i32 %337, %331
  %339 = add nuw nsw i32 %330, 1
  %exitcond12 = icmp eq i32 %339, 161
  br i1 %exitcond12, label %340, label %329, !llvm.loop !12

340:                                              ; preds = %329
  %341 = add nsw i32 %338, %327
  tail call cc1003 void @_report(i32 noundef %341)
  br label %342

342:                                              ; preds = %342, %340
  %343 = phi i16 [ %358, %342 ], [ 1, %340 ]
  %344 = phi i16 [ %357, %342 ], [ 2, %340 ]
  %345 = phi i32 [ %math10, %342 ], [ -160, %340 ]
  %346 = shl nsw i32 %345, 1
  %347 = getelementptr i8, ptr @_a, i32 %346
  %348 = getelementptr i8, ptr %347, i32 320
  %349 = add i32 %345, 1160
  %tmp7 = trunc i32 %349 to i16
  store i16 %tmp7, ptr %348, align 2, !tbaa !7
  %350 = shl nsw i32 %345, 1
  %351 = getelementptr i8, ptr @_b, i32 %350
  %352 = getelementptr i8, ptr %351, i32 320
  store i16 %343, ptr %352, align 2, !tbaa !7
  %353 = shl nsw i32 %345, 1
  %354 = getelementptr i8, ptr @_c, i32 %353
  %355 = getelementptr i8, ptr %354, i32 320
  store i16 %344, ptr %355, align 2, !tbaa !7
  %356 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %345, i32 1)
  %math10 = extractvalue { i32, i1 } %356, 0
  %ov10 = extractvalue { i32, i1 } %356, 1
  %357 = add nuw nsw i16 %344, 5
  %358 = add nuw nsw i16 %343, 3
  br i1 %ov10, label %359, label %342, !llvm.loop !9

359:                                              ; preds = %342
  %360 = icmp samesign ugt i32 %3, 1
  br i1 %360, label %361, label %557

361:                                              ; preds = %359
  %362 = lshr i32 %7, 1
  %363 = mul nsw i32 %362, -1
  %364 = shl nuw nsw i32 %362, 2
  %365 = getelementptr i8, ptr @_a, i32 %364
  %366 = and i32 %6, -2
  br label %367

367:                                              ; preds = %367, %361
  %368 = phi i32 [ %math11, %367 ], [ %363, %361 ]
  %369 = getelementptr i8, ptr @_b, i32 %366
  %370 = mul i32 %368, 2
  %371 = getelementptr i8, ptr %369, i32 %370
  %372 = load i16, ptr %371, align 2, !tbaa !7
  %373 = shl i32 %368, 2
  %374 = getelementptr i8, ptr %365, i32 %373
  store i16 %372, ptr %374, align 2, !tbaa !7
  %375 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %368, i32 1)
  %math11 = extractvalue { i32, i1 } %375, 0
  %ov11 = extractvalue { i32, i1 } %375, 1
  br i1 %ov11, label %557, label %367, !llvm.loop !17

376:                                              ; preds = %557, %376
  %377 = phi i32 [ 2, %557 ], [ %386, %376 ]
  %378 = phi i32 [ %385, %376 ], [ %559, %557 ]
  %379 = shl nuw nsw i32 %377, 1
  %380 = getelementptr i8, ptr @_a, i32 %379
  %381 = getelementptr i8, ptr %380, i32 -2
  %382 = load i16, ptr %381, align 2, !tbaa !7
  %383 = sext i16 %382 to i32
  %384 = mul nsw i32 %377, %383
  %385 = add nsw i32 %384, %378
  %386 = add nuw nsw i32 %377, 1
  %exitcond9 = icmp eq i32 %386, 161
  br i1 %exitcond9, label %387, label %376, !llvm.loop !12

387:                                              ; preds = %376
  tail call cc1003 void @_report(i32 noundef %385)
  br label %388

388:                                              ; preds = %388, %387
  %389 = phi i16 [ %404, %388 ], [ 1, %387 ]
  %390 = phi i16 [ %403, %388 ], [ 2, %387 ]
  %391 = phi i32 [ %math12, %388 ], [ -160, %387 ]
  %392 = shl nsw i32 %391, 1
  %393 = getelementptr i8, ptr @_a, i32 %392
  %394 = getelementptr i8, ptr %393, i32 320
  %395 = add i32 %391, 1160
  %tmp8 = trunc i32 %395 to i16
  store i16 %tmp8, ptr %394, align 2, !tbaa !7
  %396 = shl nsw i32 %391, 1
  %397 = getelementptr i8, ptr @_b, i32 %396
  %398 = getelementptr i8, ptr %397, i32 320
  store i16 %389, ptr %398, align 2, !tbaa !7
  %399 = shl nsw i32 %391, 1
  %400 = getelementptr i8, ptr @_c, i32 %399
  %401 = getelementptr i8, ptr %400, i32 320
  store i16 %390, ptr %401, align 2, !tbaa !7
  %402 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %391, i32 1)
  %math12 = extractvalue { i32, i1 } %402, 0
  %ov12 = extractvalue { i32, i1 } %402, 1
  %403 = add nuw nsw i16 %390, 5
  %404 = add nuw nsw i16 %389, 3
  br i1 %ov12, label %405, label %388, !llvm.loop !9

405:                                              ; preds = %388
  %406 = icmp eq i32 %3, 0
  br i1 %406, label %560, label %407

407:                                              ; preds = %405
  %408 = mul nsw i32 %wide.trip.count1, -1
  %409 = shl nuw nsw i32 %wide.trip.count1, 1
  br label %410

410:                                              ; preds = %410, %407
  %411 = phi i32 [ %math13, %410 ], [ %408, %407 ]
  %412 = shl i32 %411, 1
  %413 = add i32 %409, %412
  %414 = getelementptr i8, ptr @_b, i32 %413
  store i16 0, ptr %414, align 2, !tbaa !7
  %415 = shl i32 %411, 1
  %416 = add i32 %409, %415
  %417 = getelementptr i8, ptr @_a, i32 %416
  store i16 0, ptr %417, align 2, !tbaa !7
  %418 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %411, i32 1)
  %math13 = extractvalue { i32, i1 } %418, 0
  %ov13 = extractvalue { i32, i1 } %418, 1
  br i1 %ov13, label %560, label %410, !llvm.loop !18

419:                                              ; preds = %560, %419
  %420 = phi i32 [ 2, %560 ], [ %429, %419 ]
  %421 = phi i32 [ %428, %419 ], [ %562, %560 ]
  %422 = shl nuw nsw i32 %420, 1
  %423 = getelementptr i8, ptr @_a, i32 %422
  %424 = getelementptr i8, ptr %423, i32 -2
  %425 = load i16, ptr %424, align 2, !tbaa !7
  %426 = sext i16 %425 to i32
  %427 = mul nsw i32 %420, %426
  %428 = add nsw i32 %427, %421
  %429 = add nuw nsw i32 %420, 1
  %exitcond6 = icmp eq i32 %429, 161
  br i1 %exitcond6, label %563, label %419, !llvm.loop !12

430:                                              ; preds = %563, %430
  %431 = phi i32 [ 2, %563 ], [ %440, %430 ]
  %432 = phi i32 [ %439, %430 ], [ %565, %563 ]
  %433 = shl nuw nsw i32 %431, 1
  %434 = getelementptr i8, ptr @_b, i32 %433
  %435 = getelementptr i8, ptr %434, i32 -2
  %436 = load i16, ptr %435, align 2, !tbaa !7
  %437 = sext i16 %436 to i32
  %438 = mul nsw i32 %431, %437
  %439 = add nsw i32 %438, %432
  %440 = add nuw nsw i32 %431, 1
  %exitcond5 = icmp eq i32 %440, 161
  br i1 %exitcond5, label %441, label %430, !llvm.loop !12

441:                                              ; preds = %430
  %442 = add nsw i32 %439, %428
  tail call cc1003 void @_report(i32 noundef %442)
  br label %443

443:                                              ; preds = %443, %441
  %444 = phi i16 [ %459, %443 ], [ 1, %441 ]
  %445 = phi i16 [ %458, %443 ], [ 2, %441 ]
  %446 = phi i32 [ %math14, %443 ], [ -160, %441 ]
  %447 = shl nsw i32 %446, 1
  %448 = getelementptr i8, ptr @_a, i32 %447
  %449 = getelementptr i8, ptr %448, i32 320
  %450 = add i32 %446, 1160
  %tmp9 = trunc i32 %450 to i16
  store i16 %tmp9, ptr %449, align 2, !tbaa !7
  %451 = shl nsw i32 %446, 1
  %452 = getelementptr i8, ptr @_b, i32 %451
  %453 = getelementptr i8, ptr %452, i32 320
  store i16 %444, ptr %453, align 2, !tbaa !7
  %454 = shl nsw i32 %446, 1
  %455 = getelementptr i8, ptr @_c, i32 %454
  %456 = getelementptr i8, ptr %455, i32 320
  store i16 %445, ptr %456, align 2, !tbaa !7
  %457 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %446, i32 1)
  %math14 = extractvalue { i32, i1 } %457, 0
  %ov14 = extractvalue { i32, i1 } %457, 1
  %458 = add nuw nsw i16 %445, 5
  %459 = add nuw nsw i16 %444, 3
  br i1 %ov14, label %460, label %443, !llvm.loop !9

460:                                              ; preds = %443
  %461 = icmp ult i16 %34, 3
  br i1 %461, label %462, label %566

462:                                              ; preds = %460
  %463 = shl nuw nsw i16 %32, 1
  %464 = zext nneg i16 %463 to i32
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 2 @_a, ptr align 2 @_b, i32 %464, i1 false)
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 2 getelementptr inbounds nuw (i8, ptr @_a, i32 32), ptr align 2 getelementptr inbounds nuw (i8, ptr @_b, i32 40), i32 %464, i1 false)
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 2 getelementptr inbounds nuw (i8, ptr @_a, i32 64), ptr align 2 getelementptr inbounds nuw (i8, ptr @_b, i32 80), i32 %464, i1 false)
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 2 getelementptr inbounds nuw (i8, ptr @_a, i32 96), ptr align 2 getelementptr inbounds nuw (i8, ptr @_b, i32 120), i32 %464, i1 false)
  br label %566

465:                                              ; preds = %566, %465
  %466 = phi i32 [ 2, %566 ], [ %475, %465 ]
  %467 = phi i32 [ %474, %465 ], [ %568, %566 ]
  %468 = shl nuw nsw i32 %466, 1
  %469 = getelementptr i8, ptr @_a, i32 %468
  %470 = getelementptr i8, ptr %469, i32 -2
  %471 = load i16, ptr %470, align 2, !tbaa !7
  %472 = sext i16 %471 to i32
  %473 = mul nsw i32 %466, %472
  %474 = add nsw i32 %473, %467
  %475 = add nuw nsw i32 %466, 1
  %exitcond3 = icmp eq i32 %475, 161
  br i1 %exitcond3, label %476, label %465, !llvm.loop !12

476:                                              ; preds = %465
  tail call cc1003 void @_report(i32 noundef %474)
  br label %477

477:                                              ; preds = %477, %476
  %478 = phi i32 [ %487, %477 ], [ 8192, %476 ]
  %479 = phi i32 [ %486, %477 ], [ 4096, %476 ]
  %480 = shl nuw nsw i32 %479, 2
  %481 = getelementptr i8, ptr @_L, i32 %480
  %482 = getelementptr i8, ptr %481, i32 -16384
  store i32 %479, ptr %482, align 4, !tbaa !19
  %483 = shl nuw nsw i32 %479, 2
  %484 = getelementptr i8, ptr @_M, i32 %483
  %485 = getelementptr i8, ptr %484, i32 -16384
  store i32 %478, ptr %485, align 4, !tbaa !19
  %486 = add nuw nsw i32 %479, 1
  %487 = add nuw nsw i32 %478, 3
  %exitcond31 = icmp eq i32 %486, 4136
  br i1 %exitcond31, label %488, label %477, !llvm.loop !21

488:                                              ; preds = %477
  %489 = trunc i32 %wide.trip.count1 to i16
  %490 = icmp eq i32 %3, 0
  %491 = tail call i16 @llvm.smin.i16(i16 %489, i16 40)
  br i1 %490, label %569, label %492

492:                                              ; preds = %488
  %493 = zext nneg i16 %491 to i32
  %494 = shl nuw nsw i32 %493, 2
  tail call void @llvm.memcpy.p0.p0.i32(ptr align 4 @_L, ptr align 4 @_M, i32 %494, i1 false)
  br label %569

495:                                              ; preds = %569, %495
  %496 = phi i32 [ 2, %569 ], [ %504, %495 ]
  %497 = phi i32 [ %503, %495 ], [ %570, %569 ]
  %498 = shl nuw nsw i32 %496, 2
  %499 = getelementptr i8, ptr @_L, i32 %498
  %500 = getelementptr i8, ptr %499, i32 -4
  %501 = load i32, ptr %500, align 4, !tbaa !19
  %502 = mul nsw i32 %501, %496
  %503 = add nsw i32 %502, %497
  %504 = add nuw nsw i32 %496, 1
  %exitcond2 = icmp eq i32 %504, 41
  br i1 %exitcond2, label %505, label %495, !llvm.loop !22

505:                                              ; preds = %495
  %506 = icmp eq i32 %3, 0
  tail call cc1003 void @_report(i32 noundef %503)
  tail call void @llvm.memset.p0.i32(ptr align 4 @_L, i8 -1, i32 160, i1 false)
  %507 = add nsw i32 %12, 1193046
  br i1 %506, label %571, label %508

508:                                              ; preds = %505
  %wide.trip.count = zext nneg i16 %491 to i32
  %509 = mul nsw i32 %wide.trip.count, -1
  %510 = shl nuw nsw i32 %wide.trip.count, 2
  %511 = getelementptr i8, ptr @_L, i32 %510
  br label %512

512:                                              ; preds = %512, %508
  %513 = phi i32 [ %math15, %512 ], [ %509, %508 ]
  %514 = shl i32 %513, 2
  %515 = getelementptr i8, ptr %511, i32 %514
  store i32 %507, ptr %515, align 4, !tbaa !19
  %516 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %513, i32 1)
  %math15 = extractvalue { i32, i1 } %516, 0
  %ov15 = extractvalue { i32, i1 } %516, 1
  br i1 %ov15, label %571, label %512, !llvm.loop !23

517:                                              ; preds = %571, %517
  %518 = phi i32 [ 2, %571 ], [ %526, %517 ]
  %519 = phi i32 [ %525, %517 ], [ %572, %571 ]
  %520 = shl nuw nsw i32 %518, 2
  %521 = getelementptr i8, ptr @_L, i32 %520
  %522 = getelementptr i8, ptr %521, i32 -4
  %523 = load i32, ptr %522, align 4, !tbaa !19
  %524 = mul nsw i32 %523, %518
  %525 = add nsw i32 %524, %519
  %526 = add nuw nsw i32 %518, 1
  %exitcond = icmp eq i32 %526, 41
  br i1 %exitcond, label %527, label %517, !llvm.loop !22

527:                                              ; preds = %517
  tail call cc1003 void @_report(i32 noundef %525)
  %528 = add nuw nsw i32 %3, 1
  %exitcond32 = icmp eq i32 %528, 6
  br i1 %exitcond32, label %529, label %2, !llvm.loop !24

529:                                              ; preds = %527
  ret i32 0

530:                                              ; preds = %38, %30
  %531 = load i16, ptr @_a, align 2, !tbaa !7
  %532 = sext i16 %531 to i32
  br label %72

533:                                              ; preds = %94, %83
  %534 = load i16, ptr @_pal, align 2, !tbaa !7
  %535 = sext i16 %534 to i32
  br label %99

536:                                              ; preds = %130, %128
  %537 = load i16, ptr @_a, align 2, !tbaa !7
  %538 = sext i16 %537 to i32
  br label %132

539:                                              ; preds = %163, %161
  %540 = load i16, ptr @_a, align 2, !tbaa !7
  %541 = sext i16 %540 to i32
  br label %165

542:                                              ; preds = %198, %194
  %543 = load i16, ptr @_a, align 2, !tbaa !7
  %544 = sext i16 %543 to i32
  br label %200

545:                                              ; preds = %235, %229
  %546 = load i16, ptr @_a, align 2, !tbaa !7
  %547 = sext i16 %546 to i32
  br label %240

548:                                              ; preds = %274, %269
  %549 = load i16, ptr @_a, align 2, !tbaa !7
  %550 = sext i16 %549 to i32
  br label %285

551:                                              ; preds = %316, %314
  %552 = load i16, ptr @_a, align 2, !tbaa !7
  %553 = sext i16 %552 to i32
  br label %318

554:                                              ; preds = %318
  %555 = load i16, ptr @_c, align 2, !tbaa !7
  %556 = sext i16 %555 to i32
  br label %329

557:                                              ; preds = %367, %359
  %558 = load i16, ptr @_a, align 2, !tbaa !7
  %559 = sext i16 %558 to i32
  br label %376

560:                                              ; preds = %410, %405
  %561 = load i16, ptr @_a, align 2, !tbaa !7
  %562 = sext i16 %561 to i32
  br label %419

563:                                              ; preds = %419
  %564 = load i16, ptr @_b, align 2, !tbaa !7
  %565 = sext i16 %564 to i32
  br label %430

566:                                              ; preds = %462, %460
  %567 = load i16, ptr @_a, align 2, !tbaa !7
  %568 = sext i16 %567 to i32
  br label %465

569:                                              ; preds = %492, %488
  %570 = load i32, ptr @_L, align 4, !tbaa !19
  br label %495

571:                                              ; preds = %512, %505
  %572 = load i32, ptr @_L, align 4, !tbaa !19
  br label %517
}

declare cc1003 void @_report(i32 noundef) #1

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.smin.i32(i32, i32) #2

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.smin.i16(i16, i16) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memcpy.p0.p0.i32(ptr noalias writeonly captures(none), ptr noalias readonly captures(none), i32, i1 immarg) #3

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.memmove.p0.p0.i32(ptr writeonly captures(none), ptr readonly captures(none), i32, i1 immarg) #3

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: write)
declare void @llvm.memset.p0.i32(ptr writeonly captures(none), i8, i32, i1 immarg) #4

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare { i32, i1 } @llvm.uadd.with.overflow.i32(i32, i32) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #3

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #3

attributes #0 = { nounwind "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #1 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="i486" "target-features"="+x87" }
attributes #2 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #4 = { nocallback nofree nosync nounwind willreturn memory(argmem: write) }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"short", !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
!12 = distinct !{!12, !10}
!13 = distinct !{!13, !10}
!14 = !{}
!15 = distinct !{!15, !10}
!16 = distinct !{!16, !10}
!17 = distinct !{!17, !10}
!18 = distinct !{!18, !10}
!19 = !{!20, !20, i64 0}
!20 = !{!"long", !5, i64 0}
!21 = distinct !{!21, !10}
!22 = distinct !{!22, !10}
!23 = distinct !{!23, !10}
!24 = distinct !{!24, !10}
