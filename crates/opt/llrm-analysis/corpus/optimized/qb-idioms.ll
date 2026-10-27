target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@"A%" = internal global [320 x i8] zeroinitializer, align 2
@"B%" = internal global [320 x i8] zeroinitializer, align 2
@"C%" = internal global [320 x i8] zeroinitializer, align 2
@"PAL%" = internal global [128 x i8] zeroinitializer, align 2
@"L&" = internal global [160 x i8] zeroinitializer, align 2
@"M&" = internal global [160 x i8] zeroinitializer, align 2
@"N%" = internal global [2 x i8] zeroinitializer

define internal cc 1000 void @__main() addrspace(1) "llrm-fp-eval"="x87-extended" noreturn norecurse {
b1:
  br label %0

0:
  %1 = phi i16 [ 0, %b1 ], [ %504, %501 ]
  tail call cc 1005 addrspace(1) void @llrm.qb.B$RDI2(ptr addrspace(1) addrspacecast (ptr @"N%" to ptr addrspace(1)))
  br label %2

2:
  %3 = phi i16 [ %17, %2 ], [ 1, %0 ]
  %4 = phi i16 [ %16, %2 ], [ 2, %0 ]
  %5 = phi i16 [ %15, %2 ], [ 1000, %0 ]
  %6 = shl nuw nsw i16 %5, 1
  %7 = getelementptr i8, ptr @"A%", i16 %6
  %8 = getelementptr i8, ptr %7, i16 -2000
  store i16 %5, ptr %8, !tbaa !2
  %9 = shl nuw nsw i16 %5, 1
  %10 = getelementptr i8, ptr @"B%", i16 %9
  %11 = getelementptr i8, ptr %10, i16 -2000
  store i16 %3, ptr %11, !tbaa !2
  %12 = shl nuw nsw i16 %5, 1
  %13 = getelementptr i8, ptr @"C%", i16 %12
  %14 = getelementptr i8, ptr %13, i16 -2000
  store i16 %4, ptr %14, !tbaa !2
  %15 = add nuw nsw i16 %5, 1
  %16 = add nuw nsw i16 %4, 5
  %17 = add nuw nsw i16 %3, 3
  %exitcond29 = icmp eq i16 %15, 1160
  br i1 %exitcond29, label %18, label %2

18:
  %19 = load i16, ptr @"N%", !tbaa !2
  %20 = sdiv i16 %19, 10
  %21 = icmp slt i16 %20, 1
  br i1 %21, label %28, label %22

22:
  br label %23

23:
  %24 = phi ptr [ @"A%", %22 ], [ %27, %23 ]
  %25 = phi i16 [ %20, %22 ], [ %26, %23 ]
  tail call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr %24, i16 4660, i16 16, i1 false)
  %26 = add nsw i16 %25, -1
  %27 = getelementptr i8, ptr %24, i16 32
  %exitcond28 = icmp eq i16 %26, 0
  br i1 %exitcond28, label %28, label %23

28:
  br label %29

29:
  %30 = phi i32 [ 1, %28 ], [ %40, %29 ]
  %31 = phi i16 [ -320, %28 ], [ %39, %29 ]
  %32 = phi i32 [ %38, %29 ], [ 0, %28 ]
  %33 = getelementptr i8, ptr @"A%", i16 %31
  %34 = getelementptr i8, ptr %33, i16 320
  %35 = load i16, ptr %34, !tbaa !2
  %36 = sext i16 %35 to i32
  %37 = mul nsw i32 %36, %30
  %38 = add i32 %37, %32
  %39 = add nsw i16 %31, 2
  %40 = add nuw nsw i32 %30, 1
  %exitcond27 = icmp eq i16 %39, 0
  br i1 %exitcond27, label %41, label %29

41:
  %42 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %38)
  %43 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %42)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %43)
  tail call void @llvm.memset.p0.i16(ptr align 2 @"PAL%", i8 -1, i16 128, i1 false)
  %44 = load i16, ptr @"N%"
  %45 = tail call i16 @llvm.smin.i16(i16 %44, i16 64)
  %46 = shl i16 %44, 8
  %47 = or disjoint i16 %46, 52
  %48 = add i16 %45, -1
  %49 = icmp slt i16 %48, 0
  br i1 %49, label %51, label %50

b6:
  tail call cc 1005 addrspace(1) void @llrm.qb.B$CEND()
  unreachable

50:
  tail call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr @"PAL%", i16 %47, i16 %45, i1 false)
  br label %51

51:
  br label %52

52:
  %53 = phi i32 [ 1, %51 ], [ %63, %52 ]
  %54 = phi i16 [ -128, %51 ], [ %62, %52 ]
  %55 = phi i32 [ %61, %52 ], [ 0, %51 ]
  %56 = getelementptr i8, ptr @"PAL%", i16 %54
  %57 = getelementptr i8, ptr %56, i16 128
  %58 = load i16, ptr %57, !tbaa !2
  %59 = sext i16 %58 to i32
  %60 = mul nsw i32 %59, %53
  %61 = add i32 %60, %55
  %62 = add nsw i16 %54, 2
  %63 = add nuw nsw i32 %53, 1
  %exitcond25 = icmp eq i16 %62, 0
  br i1 %exitcond25, label %64, label %52

64:
  %65 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %61)
  %66 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %65)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %66)
  br label %67

67:
  %68 = phi i16 [ %82, %67 ], [ 1, %64 ]
  %69 = phi i16 [ %81, %67 ], [ 2, %64 ]
  %70 = phi i16 [ %80, %67 ], [ 1000, %64 ]
  %71 = shl nuw nsw i16 %70, 1
  %72 = getelementptr i8, ptr @"A%", i16 %71
  %73 = getelementptr i8, ptr %72, i16 -2000
  store i16 %70, ptr %73, !tbaa !2
  %74 = shl nuw nsw i16 %70, 1
  %75 = getelementptr i8, ptr @"B%", i16 %74
  %76 = getelementptr i8, ptr %75, i16 -2000
  store i16 %68, ptr %76, !tbaa !2
  %77 = shl nuw nsw i16 %70, 1
  %78 = getelementptr i8, ptr @"C%", i16 %77
  %79 = getelementptr i8, ptr %78, i16 -2000
  store i16 %69, ptr %79, !tbaa !2
  %80 = add nuw nsw i16 %70, 1
  %81 = add nuw nsw i16 %69, 5
  %82 = add nuw nsw i16 %68, 3
  %exitcond24 = icmp eq i16 %80, 1160
  br i1 %exitcond24, label %83, label %67

83:
  %84 = load i16, ptr @"N%"
  %85 = add i16 %84, -1
  %86 = icmp slt i16 %85, 0
  br i1 %86, label %89, label %87

87:
  %88 = shl nuw i16 %84, 1
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 @"A%", ptr align 2 @"B%", i16 %88, i1 false)
  br label %89

89:
  br label %90

90:
  %91 = phi i32 [ 1, %89 ], [ %101, %90 ]
  %92 = phi i16 [ -320, %89 ], [ %100, %90 ]
  %93 = phi i32 [ %99, %90 ], [ 0, %89 ]
  %94 = getelementptr i8, ptr @"A%", i16 %92
  %95 = getelementptr i8, ptr %94, i16 320
  %96 = load i16, ptr %95, !tbaa !2
  %97 = sext i16 %96 to i32
  %98 = mul nsw i32 %97, %91
  %99 = add i32 %98, %93
  %100 = add nsw i16 %92, 2
  %101 = add nuw nsw i32 %91, 1
  %exitcond23 = icmp eq i16 %100, 0
  br i1 %exitcond23, label %102, label %90

102:
  %103 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %99)
  %104 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %103)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %104)
  br label %105

105:
  %106 = phi i16 [ %120, %105 ], [ 1, %102 ]
  %107 = phi i16 [ %119, %105 ], [ 2, %102 ]
  %108 = phi i16 [ %118, %105 ], [ 1000, %102 ]
  %109 = shl nuw nsw i16 %108, 1
  %110 = getelementptr i8, ptr @"A%", i16 %109
  %111 = getelementptr i8, ptr %110, i16 -2000
  store i16 %108, ptr %111, !tbaa !2
  %112 = shl nuw nsw i16 %108, 1
  %113 = getelementptr i8, ptr @"B%", i16 %112
  %114 = getelementptr i8, ptr %113, i16 -2000
  store i16 %106, ptr %114, !tbaa !2
  %115 = shl nuw nsw i16 %108, 1
  %116 = getelementptr i8, ptr @"C%", i16 %115
  %117 = getelementptr i8, ptr %116, i16 -2000
  store i16 %107, ptr %117, !tbaa !2
  %118 = add nuw nsw i16 %108, 1
  %119 = add nuw nsw i16 %107, 5
  %120 = add nuw nsw i16 %106, 3
  %exitcond22 = icmp eq i16 %118, 1160
  br i1 %exitcond22, label %121, label %105

121:
  %122 = load i16, ptr @"N%"
  %123 = add i16 %122, -1
  %124 = icmp slt i16 %123, 0
  br i1 %124, label %127, label %125

125:
  %126 = shl nuw i16 %122, 1
  tail call void @llvm.memmove.p0.p0.i16(ptr align 2 @"A%", ptr align 2 getelementptr inbounds nuw (i8, ptr @"A%", i16 32), i16 %126, i1 false), !llrm.forward !5
  br label %127

127:
  br label %128

128:
  %129 = phi i32 [ 1, %127 ], [ %139, %128 ]
  %130 = phi i16 [ -320, %127 ], [ %138, %128 ]
  %131 = phi i32 [ %137, %128 ], [ 0, %127 ]
  %132 = getelementptr i8, ptr @"A%", i16 %130
  %133 = getelementptr i8, ptr %132, i16 320
  %134 = load i16, ptr %133, !tbaa !2
  %135 = sext i16 %134 to i32
  %136 = mul nsw i32 %135, %129
  %137 = add i32 %136, %131
  %138 = add nsw i16 %130, 2
  %139 = add nuw nsw i32 %129, 1
  %exitcond21 = icmp eq i16 %138, 0
  br i1 %exitcond21, label %140, label %128

140:
  %141 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %137)
  %142 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %141)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %142)
  br label %143

143:
  %144 = phi i16 [ %158, %143 ], [ 1, %140 ]
  %145 = phi i16 [ %157, %143 ], [ 2, %140 ]
  %146 = phi i16 [ %156, %143 ], [ 1000, %140 ]
  %147 = shl nuw nsw i16 %146, 1
  %148 = getelementptr i8, ptr @"A%", i16 %147
  %149 = getelementptr i8, ptr %148, i16 -2000
  store i16 %146, ptr %149, !tbaa !2
  %150 = shl nuw nsw i16 %146, 1
  %151 = getelementptr i8, ptr @"B%", i16 %150
  %152 = getelementptr i8, ptr %151, i16 -2000
  store i16 %144, ptr %152, !tbaa !2
  %153 = shl nuw nsw i16 %146, 1
  %154 = getelementptr i8, ptr @"C%", i16 %153
  %155 = getelementptr i8, ptr %154, i16 -2000
  store i16 %145, ptr %155, !tbaa !2
  %156 = add nuw nsw i16 %146, 1
  %157 = add nuw nsw i16 %145, 5
  %158 = add nuw nsw i16 %144, 3
  %exitcond20 = icmp eq i16 %156, 1160
  br i1 %exitcond20, label %159, label %143

159:
  %160 = load i16, ptr @"N%"
  %161 = add i16 %160, -1
  %162 = icmp sgt i16 %161, -1
  br i1 %162, label %163, label %165

163:
  %164 = shl nuw i16 %160, 1
  tail call void @llvm.memmove.p0.p0.i16(ptr align 2 getelementptr inbounds nuw (i8, ptr @"A%", i16 32), ptr align 2 @"A%", i16 %164, i1 false), !llrm.backward !6
  br label %165

165:
  br label %166

166:
  %167 = phi i32 [ 1, %165 ], [ %177, %166 ]
  %168 = phi i16 [ -320, %165 ], [ %176, %166 ]
  %169 = phi i32 [ %175, %166 ], [ 0, %165 ]
  %170 = getelementptr i8, ptr @"A%", i16 %168
  %171 = getelementptr i8, ptr %170, i16 320
  %172 = load i16, ptr %171, !tbaa !2
  %173 = sext i16 %172 to i32
  %174 = mul nsw i32 %173, %167
  %175 = add i32 %174, %169
  %176 = add nsw i16 %168, 2
  %177 = add nuw nsw i32 %167, 1
  %exitcond19 = icmp eq i16 %176, 0
  br i1 %exitcond19, label %178, label %166

178:
  %179 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %175)
  %180 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %179)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %180)
  br label %181

181:
  %182 = phi i16 [ %196, %181 ], [ 1, %178 ]
  %183 = phi i16 [ %195, %181 ], [ 2, %178 ]
  %184 = phi i16 [ %194, %181 ], [ 1000, %178 ]
  %185 = shl nuw nsw i16 %184, 1
  %186 = getelementptr i8, ptr @"A%", i16 %185
  %187 = getelementptr i8, ptr %186, i16 -2000
  store i16 %184, ptr %187, !tbaa !2
  %188 = shl nuw nsw i16 %184, 1
  %189 = getelementptr i8, ptr @"B%", i16 %188
  %190 = getelementptr i8, ptr %189, i16 -2000
  store i16 %182, ptr %190, !tbaa !2
  %191 = shl nuw nsw i16 %184, 1
  %192 = getelementptr i8, ptr @"C%", i16 %191
  %193 = getelementptr i8, ptr %192, i16 -2000
  store i16 %183, ptr %193, !tbaa !2
  %194 = add nuw nsw i16 %184, 1
  %195 = add nuw nsw i16 %183, 5
  %196 = add nuw nsw i16 %182, 3
  %exitcond18 = icmp eq i16 %194, 1160
  br i1 %exitcond18, label %197, label %181

197:
  %198 = load i16, ptr @"N%"
  %199 = add i16 %198, -1
  %200 = icmp slt i16 %199, 0
  br i1 %200, label %203, label %201

201:
  %202 = load i16, ptr @"A%", !tbaa !2
  tail call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr getelementptr inbounds nuw (i8, ptr @"A%", i16 2), i16 %202, i16 %198, i1 false)
  br label %203

203:
  br label %204

204:
  %205 = phi i32 [ 1, %203 ], [ %215, %204 ]
  %206 = phi i16 [ -320, %203 ], [ %214, %204 ]
  %207 = phi i32 [ %213, %204 ], [ 0, %203 ]
  %208 = getelementptr i8, ptr @"A%", i16 %206
  %209 = getelementptr i8, ptr %208, i16 320
  %210 = load i16, ptr %209, !tbaa !2
  %211 = sext i16 %210 to i32
  %212 = mul nsw i32 %211, %205
  %213 = add i32 %212, %207
  %214 = add nsw i16 %206, 2
  %215 = add nuw nsw i32 %205, 1
  %exitcond17 = icmp eq i16 %214, 0
  br i1 %exitcond17, label %216, label %204

216:
  %217 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %213)
  %218 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %217)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %218)
  br label %219

219:
  %220 = phi i16 [ %234, %219 ], [ 1, %216 ]
  %221 = phi i16 [ %233, %219 ], [ 2, %216 ]
  %222 = phi i16 [ %232, %219 ], [ 1000, %216 ]
  %223 = shl nuw nsw i16 %222, 1
  %224 = getelementptr i8, ptr @"A%", i16 %223
  %225 = getelementptr i8, ptr %224, i16 -2000
  store i16 %222, ptr %225, !tbaa !2
  %226 = shl nuw nsw i16 %222, 1
  %227 = getelementptr i8, ptr @"B%", i16 %226
  %228 = getelementptr i8, ptr %227, i16 -2000
  store i16 %220, ptr %228, !tbaa !2
  %229 = shl nuw nsw i16 %222, 1
  %230 = getelementptr i8, ptr @"C%", i16 %229
  %231 = getelementptr i8, ptr %230, i16 -2000
  store i16 %221, ptr %231, !tbaa !2
  %232 = add nuw nsw i16 %222, 1
  %233 = add nuw nsw i16 %221, 5
  %234 = add nuw nsw i16 %220, 3
  %exitcond16 = icmp eq i16 %232, 1160
  br i1 %exitcond16, label %235, label %219

235:
  %236 = load i16, ptr @"N%"
  %237 = add i16 %236, -1
  %238 = icmp slt i16 %237, 0
  br i1 %238, label %249, label %239

239:
  br label %240

240:
  %241 = phi i16 [ 0, %239 ], [ %248, %240 ]
  %242 = phi i16 [ %236, %239 ], [ %247, %240 ]
  %243 = getelementptr i8, ptr @"B%", i16 %241
  %244 = load i16, ptr %243, !tbaa !2
  %245 = add i16 %244, 1
  %246 = getelementptr i8, ptr @"A%", i16 %241
  store i16 %245, ptr %246, !tbaa !2
  %247 = add i16 %242, -1
  %248 = add nuw i16 %241, 2
  %exitcond15 = icmp eq i16 %247, 0
  br i1 %exitcond15, label %249, label %240

249:
  br label %250

250:
  %251 = phi i32 [ 1, %249 ], [ %261, %250 ]
  %252 = phi i16 [ -320, %249 ], [ %260, %250 ]
  %253 = phi i32 [ %259, %250 ], [ 0, %249 ]
  %254 = getelementptr i8, ptr @"A%", i16 %252
  %255 = getelementptr i8, ptr %254, i16 320
  %256 = load i16, ptr %255, !tbaa !2
  %257 = sext i16 %256 to i32
  %258 = mul nsw i32 %257, %251
  %259 = add i32 %258, %253
  %260 = add nsw i16 %252, 2
  %261 = add nuw nsw i32 %251, 1
  %exitcond14 = icmp eq i16 %260, 0
  br i1 %exitcond14, label %262, label %250

262:
  %263 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %259)
  %264 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %263)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %264)
  br label %265

265:
  %266 = phi i16 [ %280, %265 ], [ 1, %262 ]
  %267 = phi i16 [ %279, %265 ], [ 2, %262 ]
  %268 = phi i16 [ %278, %265 ], [ 1000, %262 ]
  %269 = shl nuw nsw i16 %268, 1
  %270 = getelementptr i8, ptr @"A%", i16 %269
  %271 = getelementptr i8, ptr %270, i16 -2000
  store i16 %268, ptr %271, !tbaa !2
  %272 = shl nuw nsw i16 %268, 1
  %273 = getelementptr i8, ptr @"B%", i16 %272
  %274 = getelementptr i8, ptr %273, i16 -2000
  store i16 %266, ptr %274, !tbaa !2
  %275 = shl nuw nsw i16 %268, 1
  %276 = getelementptr i8, ptr @"C%", i16 %275
  %277 = getelementptr i8, ptr %276, i16 -2000
  store i16 %267, ptr %277, !tbaa !2
  %278 = add nuw nsw i16 %268, 1
  %279 = add nuw nsw i16 %267, 5
  %280 = add nuw nsw i16 %266, 3
  %exitcond13 = icmp eq i16 %278, 1160
  br i1 %exitcond13, label %281, label %265

281:
  %282 = load i16, ptr @"N%"
  %283 = add i16 %282, -1
  %284 = icmp slt i16 %283, 0
  br i1 %284, label %287, label %285

285:
  %286 = shl nuw i16 %282, 1
  tail call void @llvm.memset.p0.i16(ptr align 2 @"C%", i8 0, i16 %286, i1 false)
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 @"A%", ptr align 2 @"B%", i16 %286, i1 false)
  br label %287

287:
  br label %288

288:
  %289 = phi i32 [ 1, %287 ], [ %299, %288 ]
  %290 = phi i16 [ -320, %287 ], [ %298, %288 ]
  %291 = phi i32 [ %297, %288 ], [ 0, %287 ]
  %292 = getelementptr i8, ptr @"A%", i16 %290
  %293 = getelementptr i8, ptr %292, i16 320
  %294 = load i16, ptr %293, !tbaa !2
  %295 = sext i16 %294 to i32
  %296 = mul nsw i32 %295, %289
  %297 = add i32 %296, %291
  %298 = add nsw i16 %290, 2
  %299 = add nuw nsw i32 %289, 1
  %exitcond12 = icmp eq i16 %298, 0
  br i1 %exitcond12, label %300, label %288

300:
  br label %301

301:
  %302 = phi i32 [ 1, %300 ], [ %312, %301 ]
  %303 = phi i16 [ -320, %300 ], [ %311, %301 ]
  %304 = phi i32 [ %310, %301 ], [ 0, %300 ]
  %305 = getelementptr i8, ptr @"C%", i16 %303
  %306 = getelementptr i8, ptr %305, i16 320
  %307 = load i16, ptr %306, !tbaa !2
  %308 = sext i16 %307 to i32
  %309 = mul nsw i32 %308, %302
  %310 = add i32 %309, %304
  %311 = add nsw i16 %303, 2
  %312 = add nuw nsw i32 %302, 1
  %exitcond11 = icmp eq i16 %311, 0
  br i1 %exitcond11, label %313, label %301

313:
  %314 = add i32 %310, %297
  %315 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %314)
  %316 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %315)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %316)
  br label %317

317:
  %318 = phi i16 [ %332, %317 ], [ 1, %313 ]
  %319 = phi i16 [ %331, %317 ], [ 2, %313 ]
  %320 = phi i16 [ %330, %317 ], [ 1000, %313 ]
  %321 = shl nuw nsw i16 %320, 1
  %322 = getelementptr i8, ptr @"A%", i16 %321
  %323 = getelementptr i8, ptr %322, i16 -2000
  store i16 %320, ptr %323, !tbaa !2
  %324 = shl nuw nsw i16 %320, 1
  %325 = getelementptr i8, ptr @"B%", i16 %324
  %326 = getelementptr i8, ptr %325, i16 -2000
  store i16 %318, ptr %326, !tbaa !2
  %327 = shl nuw nsw i16 %320, 1
  %328 = getelementptr i8, ptr @"C%", i16 %327
  %329 = getelementptr i8, ptr %328, i16 -2000
  store i16 %319, ptr %329, !tbaa !2
  %330 = add nuw nsw i16 %320, 1
  %331 = add nuw nsw i16 %319, 5
  %332 = add nuw nsw i16 %318, 3
  %exitcond10 = icmp eq i16 %330, 1160
  br i1 %exitcond10, label %333, label %317

333:
  %334 = load i16, ptr @"N%"
  %335 = sdiv i16 %334, 2
  %336 = icmp sgt i16 %335, 0
  br i1 %336, label %337, label %346

337:
  br label %338

338:
  %339 = phi ptr [ @"B%", %337 ], [ %345, %338 ]
  %340 = phi ptr [ @"A%", %337 ], [ %344, %338 ]
  %341 = phi i16 [ %335, %337 ], [ %343, %338 ]
  %342 = load i16, ptr %339, !tbaa !2
  store i16 %342, ptr %340, !tbaa !2
  %343 = add nsw i16 %341, -1
  %344 = getelementptr i8, ptr %340, i16 4
  %345 = getelementptr i8, ptr %339, i16 2
  %exitcond9 = icmp eq i16 %343, 0
  br i1 %exitcond9, label %346, label %338

346:
  br label %347

347:
  %348 = phi i32 [ 1, %346 ], [ %358, %347 ]
  %349 = phi i16 [ -320, %346 ], [ %357, %347 ]
  %350 = phi i32 [ %356, %347 ], [ 0, %346 ]
  %351 = getelementptr i8, ptr @"A%", i16 %349
  %352 = getelementptr i8, ptr %351, i16 320
  %353 = load i16, ptr %352, !tbaa !2
  %354 = sext i16 %353 to i32
  %355 = mul nsw i32 %354, %348
  %356 = add i32 %355, %350
  %357 = add nsw i16 %349, 2
  %358 = add nuw nsw i32 %348, 1
  %exitcond8 = icmp eq i16 %357, 0
  br i1 %exitcond8, label %359, label %347

359:
  %360 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %356)
  %361 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %360)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %361)
  br label %362

362:
  %363 = phi i16 [ %377, %362 ], [ 1, %359 ]
  %364 = phi i16 [ %376, %362 ], [ 2, %359 ]
  %365 = phi i16 [ %375, %362 ], [ 1000, %359 ]
  %366 = shl nuw nsw i16 %365, 1
  %367 = getelementptr i8, ptr @"A%", i16 %366
  %368 = getelementptr i8, ptr %367, i16 -2000
  store i16 %365, ptr %368, !tbaa !2
  %369 = shl nuw nsw i16 %365, 1
  %370 = getelementptr i8, ptr @"B%", i16 %369
  %371 = getelementptr i8, ptr %370, i16 -2000
  store i16 %363, ptr %371, !tbaa !2
  %372 = shl nuw nsw i16 %365, 1
  %373 = getelementptr i8, ptr @"C%", i16 %372
  %374 = getelementptr i8, ptr %373, i16 -2000
  store i16 %364, ptr %374, !tbaa !2
  %375 = add nuw nsw i16 %365, 1
  %376 = add nuw nsw i16 %364, 5
  %377 = add nuw nsw i16 %363, 3
  %exitcond7 = icmp eq i16 %375, 1160
  br i1 %exitcond7, label %378, label %362

378:
  %379 = load i16, ptr @"N%"
  %380 = add i16 %379, -1
  %381 = icmp slt i16 %380, 0
  br i1 %381, label %384, label %382

382:
  %383 = shl nuw i16 %379, 1
  tail call void @llvm.memset.p0.i16(ptr align 2 @"B%", i8 0, i16 %383, i1 false)
  tail call void @llvm.memset.p0.i16(ptr align 2 @"A%", i8 0, i16 %383, i1 false)
  br label %384

384:
  br label %385

385:
  %386 = phi i32 [ 1, %384 ], [ %396, %385 ]
  %387 = phi i16 [ -320, %384 ], [ %395, %385 ]
  %388 = phi i32 [ %394, %385 ], [ 0, %384 ]
  %389 = getelementptr i8, ptr @"A%", i16 %387
  %390 = getelementptr i8, ptr %389, i16 320
  %391 = load i16, ptr %390, !tbaa !2
  %392 = sext i16 %391 to i32
  %393 = mul nsw i32 %392, %386
  %394 = add i32 %393, %388
  %395 = add nsw i16 %387, 2
  %396 = add nuw nsw i32 %386, 1
  %exitcond6 = icmp eq i16 %395, 0
  br i1 %exitcond6, label %397, label %385

397:
  br label %398

398:
  %399 = phi i32 [ 1, %397 ], [ %409, %398 ]
  %400 = phi i16 [ -320, %397 ], [ %408, %398 ]
  %401 = phi i32 [ %407, %398 ], [ 0, %397 ]
  %402 = getelementptr i8, ptr @"B%", i16 %400
  %403 = getelementptr i8, ptr %402, i16 320
  %404 = load i16, ptr %403, !tbaa !2
  %405 = sext i16 %404 to i32
  %406 = mul nsw i32 %405, %399
  %407 = add i32 %406, %401
  %408 = add nsw i16 %400, 2
  %409 = add nuw nsw i32 %399, 1
  %exitcond5 = icmp eq i16 %408, 0
  br i1 %exitcond5, label %410, label %398

410:
  %411 = add i32 %407, %394
  %412 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %411)
  %413 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %412)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %413)
  br label %414

414:
  %415 = phi i16 [ %429, %414 ], [ 1, %410 ]
  %416 = phi i16 [ %428, %414 ], [ 2, %410 ]
  %417 = phi i16 [ %427, %414 ], [ 1000, %410 ]
  %418 = shl nuw nsw i16 %417, 1
  %419 = getelementptr i8, ptr @"A%", i16 %418
  %420 = getelementptr i8, ptr %419, i16 -2000
  store i16 %417, ptr %420, !tbaa !2
  %421 = shl nuw nsw i16 %417, 1
  %422 = getelementptr i8, ptr @"B%", i16 %421
  %423 = getelementptr i8, ptr %422, i16 -2000
  store i16 %415, ptr %423, !tbaa !2
  %424 = shl nuw nsw i16 %417, 1
  %425 = getelementptr i8, ptr @"C%", i16 %424
  %426 = getelementptr i8, ptr %425, i16 -2000
  store i16 %416, ptr %426, !tbaa !2
  %427 = add nuw nsw i16 %417, 1
  %428 = add nuw nsw i16 %416, 5
  %429 = add nuw nsw i16 %415, 3
  %exitcond4 = icmp eq i16 %427, 1160
  br i1 %exitcond4, label %430, label %414

430:
  %431 = load i16, ptr @"N%", !tbaa !2
  %432 = sdiv i16 %431, 10
  %433 = icmp slt i16 %432, 1
  br i1 %433, label %436, label %434

434:
  %435 = shl nuw nsw i16 %432, 1
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 @"A%", ptr align 2 @"B%", i16 %435, i1 false)
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 getelementptr inbounds nuw (i8, ptr @"A%", i16 32), ptr align 2 getelementptr inbounds nuw (i8, ptr @"B%", i16 40), i16 %435, i1 false)
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 getelementptr inbounds nuw (i8, ptr @"A%", i16 64), ptr align 2 getelementptr inbounds nuw (i8, ptr @"B%", i16 80), i16 %435, i1 false)
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 getelementptr inbounds nuw (i8, ptr @"A%", i16 96), ptr align 2 getelementptr inbounds nuw (i8, ptr @"B%", i16 120), i16 %435, i1 false)
  br label %436

436:
  br label %437

437:
  %438 = phi i32 [ 1, %436 ], [ %448, %437 ]
  %439 = phi i16 [ -320, %436 ], [ %447, %437 ]
  %440 = phi i32 [ %446, %437 ], [ 0, %436 ]
  %441 = getelementptr i8, ptr @"A%", i16 %439
  %442 = getelementptr i8, ptr %441, i16 320
  %443 = load i16, ptr %442, !tbaa !2
  %444 = sext i16 %443 to i32
  %445 = mul nsw i32 %444, %438
  %446 = add i32 %445, %440
  %447 = add nsw i16 %439, 2
  %448 = add nuw nsw i32 %438, 1
  %exitcond3 = icmp eq i16 %447, 0
  br i1 %exitcond3, label %449, label %437

449:
  %450 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %446)
  %451 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %450)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %451)
  br label %b15

b15:
  %452 = phi i32 [ %459, %b15 ], [ 8192, %449 ]
  %453 = phi i16 [ %458, %b15 ], [ 0, %449 ]
  %454 = phi i32 [ %457, %b15 ], [ 4096, %449 ]
  %455 = getelementptr i8, ptr @"L&", i16 %453
  store i32 %454, ptr %455, !tbaa !2
  %456 = getelementptr i8, ptr @"M&", i16 %453
  store i32 %452, ptr %456, !tbaa !2
  %457 = add nuw nsw i32 %454, 1
  %tmp = trunc i32 %457 to i16
  %458 = add nuw nsw i16 %453, 4
  %459 = add nuw nsw i32 %452, 3
  %exitcond30 = icmp eq i16 %tmp, 4136
  br i1 %exitcond30, label %460, label %b15

460:
  %461 = load i16, ptr @"N%"
  %462 = tail call i16 @llvm.smin.i16(i16 %461, i16 40)
  %463 = add i16 %462, -1
  %464 = icmp slt i16 %463, 0
  br i1 %464, label %467, label %465

465:
  %466 = shl nuw i16 %462, 2
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 @"L&", ptr align 2 @"M&", i16 %466, i1 false)
  br label %467

467:
  br label %468

468:
  %469 = phi i32 [ 1, %467 ], [ %478, %468 ]
  %470 = phi i16 [ -160, %467 ], [ %477, %468 ]
  %471 = phi i32 [ %476, %468 ], [ 0, %467 ]
  %472 = getelementptr i8, ptr @"L&", i16 %470
  %473 = getelementptr i8, ptr %472, i16 160
  %474 = load i32, ptr %473, !tbaa !2
  %475 = mul i32 %474, %469
  %476 = add i32 %475, %471
  %477 = add nsw i16 %470, 4
  %478 = add nuw nsw i32 %469, 1
  %exitcond2 = icmp eq i16 %477, 0
  br i1 %exitcond2, label %479, label %468

479:
  %480 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %476)
  %481 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %480)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %481)
  tail call void @llvm.memset.p0.i16(ptr align 2 @"L&", i8 -1, i16 160, i1 false)
  %482 = load i16, ptr @"N%"
  %483 = tail call i16 @llvm.smin.i16(i16 %482, i16 40)
  %484 = sext i16 %482 to i32
  %485 = add nsw i32 %484, 1193046
  %486 = add i16 %483, -1
  %487 = icmp slt i16 %486, 0
  br i1 %487, label %489, label %488

488:
  tail call void @llvm.experimental.memset.pattern.p0.i32.i16(ptr @"L&", i32 %485, i16 %483, i1 false)
  br label %489

489:
  br label %490

490:
  %491 = phi i32 [ 1, %489 ], [ %500, %490 ]
  %492 = phi i16 [ -160, %489 ], [ %499, %490 ]
  %493 = phi i32 [ %498, %490 ], [ 0, %489 ]
  %494 = getelementptr i8, ptr @"L&", i16 %492
  %495 = getelementptr i8, ptr %494, i16 160
  %496 = load i32, ptr %495, !tbaa !2
  %497 = mul i32 %496, %491
  %498 = add i32 %497, %493
  %499 = add nsw i16 %492, 4
  %500 = add nuw nsw i32 %491, 1
  %exitcond = icmp eq i16 %499, 0
  br i1 %exitcond, label %501, label %490

501:
  %502 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$STI4(i32 %498)
  %503 = tail call cc 1005 addrspace(1) ptr @llrm.qb.B$LTRM(ptr %502)
  tail call cc 1005 addrspace(1) void @llrm.qb.B$PESD(ptr %503)
  %504 = add nuw nsw i16 %1, 1
  %exitcond31 = icmp eq i16 %504, 6
  br i1 %exitcond31, label %b6, label %0
}

define cc 1000 i16 @LIM(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(2) nocapture readonly %1) addrspace(1) "llrm-fp-eval"="x87-extended" memory(argmem: read) willreturn nounwind nofree nosync norecurse {
b1:
  %2 = load i16, ptr %0
  %3 = load i16, ptr %1
  %4 = tail call i16 @llvm.smin.i16(i16 %2, i16 %3)
  ret i16 %4
}

define cc 1000 void @INIT() addrspace(1) "llrm-fp-eval"="x87-extended" memory(write, argmem: none, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  br label %b5

b5:
  %0 = phi i16 [ %14, %b5 ], [ 1, %b1 ]
  %1 = phi i16 [ %13, %b5 ], [ 2, %b1 ]
  %2 = phi i16 [ %12, %b5 ], [ 1000, %b1 ]
  %3 = shl nuw nsw i16 %2, 1
  %4 = getelementptr i8, ptr @"A%", i16 %3
  %5 = getelementptr i8, ptr %4, i16 -2000
  store i16 %2, ptr %5, !tbaa !2
  %6 = shl nuw nsw i16 %2, 1
  %7 = getelementptr i8, ptr @"B%", i16 %6
  %8 = getelementptr i8, ptr %7, i16 -2000
  store i16 %0, ptr %8, !tbaa !2
  %9 = shl nuw nsw i16 %2, 1
  %10 = getelementptr i8, ptr @"C%", i16 %9
  %11 = getelementptr i8, ptr %10, i16 -2000
  store i16 %1, ptr %11, !tbaa !2
  %12 = add nuw nsw i16 %2, 1
  %13 = add nuw nsw i16 %1, 5
  %14 = add nuw nsw i16 %0, 3
  %exitcond = icmp eq i16 %12, 1160
  br i1 %exitcond, label %b6, label %b5

b6:
  ret void
}

define cc 1000 i32 @"SUMA&"() addrspace(1) "llrm-fp-eval"="x87-extended" memory(read, argmem: none, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  br label %b5

b5:
  %0 = phi i32 [ %10, %b5 ], [ 1, %b1 ]
  %1 = phi i16 [ %9, %b5 ], [ -320, %b1 ]
  %2 = phi i32 [ 0, %b1 ], [ %8, %b5 ]
  %3 = getelementptr i8, ptr @"A%", i16 %1
  %4 = getelementptr i8, ptr %3, i16 320
  %5 = load i16, ptr %4, !tbaa !2
  %6 = sext i16 %5 to i32
  %7 = mul nsw i32 %6, %0
  %8 = add i32 %7, %2
  %9 = add nsw i16 %1, 2
  %10 = add nuw nsw i32 %0, 1
  %exitcond = icmp eq i16 %9, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret i32 %8
}

define cc 1000 i32 @"SUMB&"() addrspace(1) "llrm-fp-eval"="x87-extended" memory(read, argmem: none, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  br label %b5

b5:
  %0 = phi i32 [ %10, %b5 ], [ 1, %b1 ]
  %1 = phi i16 [ %9, %b5 ], [ -320, %b1 ]
  %2 = phi i32 [ 0, %b1 ], [ %8, %b5 ]
  %3 = getelementptr i8, ptr @"B%", i16 %1
  %4 = getelementptr i8, ptr %3, i16 320
  %5 = load i16, ptr %4, !tbaa !2
  %6 = sext i16 %5 to i32
  %7 = mul nsw i32 %6, %0
  %8 = add i32 %7, %2
  %9 = add nsw i16 %1, 2
  %10 = add nuw nsw i32 %0, 1
  %exitcond = icmp eq i16 %9, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret i32 %8
}

define cc 1000 i32 @"SUMC&"() addrspace(1) "llrm-fp-eval"="x87-extended" memory(read, argmem: none, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  br label %b5

b5:
  %0 = phi i32 [ %10, %b5 ], [ 1, %b1 ]
  %1 = phi i16 [ %9, %b5 ], [ -320, %b1 ]
  %2 = phi i32 [ 0, %b1 ], [ %8, %b5 ]
  %3 = getelementptr i8, ptr @"C%", i16 %1
  %4 = getelementptr i8, ptr %3, i16 320
  %5 = load i16, ptr %4, !tbaa !2
  %6 = sext i16 %5 to i32
  %7 = mul nsw i32 %6, %0
  %8 = add i32 %7, %2
  %9 = add nsw i16 %1, 2
  %10 = add nuw nsw i32 %0, 1
  %exitcond = icmp eq i16 %9, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret i32 %8
}

define cc 1000 i32 @"SUMPAL&"() addrspace(1) "llrm-fp-eval"="x87-extended" memory(read, argmem: none, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  br label %b5

b5:
  %0 = phi i32 [ %10, %b5 ], [ 1, %b1 ]
  %1 = phi i16 [ %9, %b5 ], [ -128, %b1 ]
  %2 = phi i32 [ 0, %b1 ], [ %8, %b5 ]
  %3 = getelementptr i8, ptr @"PAL%", i16 %1
  %4 = getelementptr i8, ptr %3, i16 128
  %5 = load i16, ptr %4, !tbaa !2
  %6 = sext i16 %5 to i32
  %7 = mul nsw i32 %6, %0
  %8 = add i32 %7, %2
  %9 = add nsw i16 %1, 2
  %10 = add nuw nsw i32 %0, 1
  %exitcond = icmp eq i16 %9, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret i32 %8
}

define cc 1000 i32 @"SUML&"() addrspace(1) "llrm-fp-eval"="x87-extended" memory(read, argmem: none, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  br label %b5

b5:
  %0 = phi i32 [ %9, %b5 ], [ 1, %b1 ]
  %1 = phi i16 [ %8, %b5 ], [ -160, %b1 ]
  %2 = phi i32 [ 0, %b1 ], [ %7, %b5 ]
  %3 = getelementptr i8, ptr @"L&", i16 %1
  %4 = getelementptr i8, ptr %3, i16 160
  %5 = load i32, ptr %4, !tbaa !2
  %6 = mul i32 %5, %0
  %7 = add i32 %6, %2
  %8 = add nsw i16 %1, 4
  %9 = add nuw nsw i32 %0, 1
  %exitcond = icmp eq i16 %8, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret i32 %7
}

define cc 1000 void @CLEARROWS(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(write, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = add i16 %1, -1
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b6, label %4

4:
  br label %b5

b5:
  %5 = phi ptr [ @"A%", %4 ], [ %8, %b5 ]
  %6 = phi i16 [ %1, %4 ], [ %7, %b5 ]
  tail call void @llvm.experimental.memset.pattern.p0.i16.i16(ptr %5, i16 4660, i16 16, i1 false)
  %7 = add i16 %6, -1
  %8 = getelementptr i8, ptr %5, i16 32
  %exitcond = icmp eq i16 %7, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret void
}

define cc 1000 void @SETPAL(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(2) nocapture readonly %1) addrspace(1) "llrm-fp-eval"="x87-extended" memory(write, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %2 = load i16, ptr %0
  %3 = add i16 %2, -1
  %4 = icmp slt i16 %3, 0
  br i1 %4, label %b6, label %5

5:
  %6 = load i16, ptr %1
  br label %b5

b5:
  %7 = phi ptr [ %10, %b5 ], [ @"PAL%", %5 ]
  %8 = phi i16 [ %9, %b5 ], [ %2, %5 ]
  store i16 %6, ptr %7, !tbaa !2
  %9 = add i16 %8, -1
  %10 = getelementptr i8, ptr %7, i16 2
  %exitcond = icmp eq i16 %9, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret void
}

define cc 1000 void @DOCOPY(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = add i16 %1, -1
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b6, label %4

4:
  %5 = shl nuw i16 %1, 1
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 @"A%", ptr align 2 @"B%", i16 %5, i1 false)
  br label %b6

b6:
  ret void
}

define cc 1000 void @SCROLLUP(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = add i16 %1, -1
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b6, label %4

4:
  %5 = shl nuw i16 %1, 1
  tail call void @llvm.memmove.p0.p0.i16(ptr align 2 @"A%", ptr align 2 getelementptr inbounds nuw (i8, ptr @"A%", i16 32), i16 %5, i1 false), !llrm.forward !5
  br label %b6

b6:
  ret void
}

define cc 1000 void @SCROLLDOWN(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = add i16 %1, -1
  %3 = icmp sgt i16 %2, -1
  br i1 %3, label %4, label %b6

4:
  %5 = shl nuw i16 %1, 1
  tail call void @llvm.memmove.p0.p0.i16(ptr align 2 getelementptr inbounds nuw (i8, ptr @"A%", i16 32), ptr align 2 @"A%", i16 %5, i1 false), !llrm.backward !6
  br label %b6

b6:
  ret void
}

define cc 1000 void @SMEAR(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse {
b1:
  %1 = load i16, ptr %0
  %2 = add i16 %1, -1
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b6, label %4

4:
  %5 = load i16, ptr @"A%", !tbaa !2
  br label %b5

b5:
  %6 = phi i16 [ 0, %4 ], [ %7, %b5 ]
  %7 = add i16 %6, 1
  %8 = shl i16 %6, 1
  %9 = getelementptr i8, ptr @"A%", i16 %8
  %10 = getelementptr i8, ptr %9, i16 2
  store i16 %5, ptr %10, !tbaa !2
  %11 = icmp sgt i16 %7, %2
  br i1 %11, label %b6, label %b5

b6:
  ret void
}

define cc 1000 void @CHANGED(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = add i16 %1, -1
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b6, label %4

4:
  br label %b5

b5:
  %5 = phi i16 [ 0, %4 ], [ %12, %b5 ]
  %6 = phi i16 [ %1, %4 ], [ %11, %b5 ]
  %7 = getelementptr i8, ptr @"B%", i16 %5
  %8 = load i16, ptr %7, !tbaa !2
  %9 = add i16 %8, 1
  %10 = getelementptr i8, ptr @"A%", i16 %5
  store i16 %9, ptr %10, !tbaa !2
  %11 = add i16 %6, -1
  %12 = add nuw i16 %5, 2
  %exitcond = icmp eq i16 %11, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret void
}

define cc 1000 void @TWOSTORES(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = add i16 %1, -1
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b6, label %4

4:
  %5 = shl nuw i16 %1, 1
  tail call void @llvm.memset.p0.i16(ptr align 2 @"C%", i8 0, i16 %5, i1 false)
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 @"A%", ptr align 2 @"B%", i16 %5, i1 false)
  br label %b6

b6:
  ret void
}

define cc 1000 void @STRIDES(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = sdiv i16 %1, 2
  %3 = icmp sgt i16 %2, 0
  br i1 %3, label %4, label %b6

4:
  br label %b5

b5:
  %5 = phi ptr [ @"B%", %4 ], [ %11, %b5 ]
  %6 = phi ptr [ @"A%", %4 ], [ %10, %b5 ]
  %7 = phi i16 [ %2, %4 ], [ %9, %b5 ]
  %8 = load i16, ptr %5, !tbaa !2
  store i16 %8, ptr %6, !tbaa !2
  %9 = add nsw i16 %7, -1
  %10 = getelementptr i8, ptr %6, i16 4
  %11 = getelementptr i8, ptr %5, i16 2
  %exitcond = icmp eq i16 %9, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret void
}

define cc 1000 void @WRITTEN(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" nounwind nofree nosync norecurse memory(write, argmem: read, inaccessiblemem: none) willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = add i16 %1, -1
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b6, label %4

4:
  %5 = shl nuw i16 %1, 1
  tail call void @llvm.memset.p0.i16(ptr align 2 @"B%", i8 0, i16 %5, i1 false)
  tail call void @llvm.memset.p0.i16(ptr align 2 @"A%", i8 0, i16 %5, i1 false)
  br label %b6

b6:
  ret void
}

define cc 1000 void @BLK(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = add i16 %1, -1
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %6, label %4

4:
  %5 = shl nuw i16 %1, 1
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 @"A%", ptr align 2 @"B%", i16 %5, i1 false)
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 getelementptr inbounds nuw (i8, ptr @"A%", i16 32), ptr align 2 getelementptr inbounds nuw (i8, ptr @"B%", i16 40), i16 %5, i1 false)
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 getelementptr inbounds nuw (i8, ptr @"A%", i16 64), ptr align 2 getelementptr inbounds nuw (i8, ptr @"B%", i16 80), i16 %5, i1 false)
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 getelementptr inbounds nuw (i8, ptr @"A%", i16 96), ptr align 2 getelementptr inbounds nuw (i8, ptr @"B%", i16 120), i16 %5, i1 false)
  br label %6

6:
  ret void
}

define cc 1000 void @COPYLONG(ptr dereferenceable(2) nocapture readonly %0) addrspace(1) "llrm-fp-eval"="x87-extended" memory(readwrite, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %1 = load i16, ptr %0
  %2 = add i16 %1, -1
  %3 = icmp slt i16 %2, 0
  br i1 %3, label %b6, label %4

4:
  %5 = shl nuw i16 %1, 2
  tail call void @llvm.memcpy.p0.p0.i16(ptr align 2 @"L&", ptr align 2 @"M&", i16 %5, i1 false)
  br label %b6

b6:
  ret void
}

define cc 1000 void @FILLLONG(ptr dereferenceable(2) nocapture readonly %0, ptr dereferenceable(4) nocapture readonly %1) addrspace(1) "llrm-fp-eval"="x87-extended" memory(write, argmem: read, inaccessiblemem: none) nounwind nofree nosync norecurse willreturn {
b1:
  %2 = load i16, ptr %0
  %3 = add i16 %2, -1
  %4 = icmp slt i16 %3, 0
  br i1 %4, label %b6, label %5

5:
  %6 = load i32, ptr %1
  br label %b5

b5:
  %7 = phi ptr [ %10, %b5 ], [ @"L&", %5 ]
  %8 = phi i16 [ %9, %b5 ], [ %2, %5 ]
  store i32 %6, ptr %7, !tbaa !2
  %9 = add i16 %8, -1
  %10 = getelementptr i8, ptr %7, i16 4
  %exitcond = icmp eq i16 %9, 0
  br i1 %exitcond, label %b6, label %b5

b6:
  ret void
}

declare cc 1005 void @llrm.qb.B$RDI2(ptr addrspace(1) nocapture noretain) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$STI4(i32) addrspace(1) nocallback

declare cc 1005 ptr @llrm.qb.B$LTRM(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$PESD(ptr) addrspace(1) nocallback

declare cc 1005 void @llrm.qb.B$CEND() addrspace(1) nocallback nounwind noreturn

declare i16 @llvm.smin.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.experimental.memset.pattern.p0.i16.i16(ptr nocapture writeonly, i16, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memcpy.p0.p0.i16(ptr noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare void @llvm.memmove.p0.p0.i16(ptr nocapture writeonly, ptr nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.experimental.memset.pattern.p0.i32.i16(ptr nocapture writeonly, i32, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{}
!6 = !{}
