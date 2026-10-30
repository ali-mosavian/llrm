target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str3 = internal constant [17 x i8] c"\08\00\0A\00\0A\00no entries\00"
@$str4 = internal constant [13 x i8] c"\08\00\06\00\06\00first=\00"
@$str5 = internal constant [19 x i8] c"\08\00\0C\00\0C\00, remaining=\00"
@$str6 = internal constant [24 x i8] c"\08\00\11\00\11\00 positive, width=\00"
@$str7 = internal constant [12 x i8] c"\08\00\05\00\05\00width\00"

define cc 1005 range(i16 0, 2) i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [8 x i8]
  %2 = alloca [80 x i8]
  %3 = alloca [134 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %7)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %6)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %5)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %4)
  call void @llvm.lifetime.start.p0(i64 134, ptr nonnull %3)
  call void @llvm.lifetime.start.p0(i64 80, ptr nonnull %2)
  %8 = getelementptr inbounds nuw i8, ptr %2, i16 11
  call void @llvm.memset.p0.i16(ptr nonnull align 1 %8, i8 0, i16 69, i1 false)
  %9 = addrspacecast ptr %2 to ptr addrspace(5)
  %10 = addrspacecast ptr %2 to ptr addrspace(1)
  store i32 1431060822, ptr addrspace(5) %9
  %11 = getelementptr inbounds nuw i8, ptr addrspace(5) %9, i16 4
  store i32 1143886661, ptr addrspace(5) %11
  %12 = getelementptr inbounds nuw i8, ptr addrspace(5) %9, i16 8
  store i16 21569, ptr addrspace(5) %12
  %13 = getelementptr inbounds nuw i8, ptr addrspace(5) %9, i16 10
  store i8 0, ptr addrspace(5) %13, align 2
  %14 = call addrspace(1) i16 @LL$OPEN(ptr addrspace(1) %10, i8 zeroext 0)
  %15 = icmp slt i16 %14, 0
  call void @llvm.lifetime.end.p0(i64 80, ptr nonnull %2)
  br i1 %15, label %174, label %16

16:
  store i16 %14, ptr %3, !tbaa !2
  %17 = getelementptr inbounds nuw i8, ptr %3, i16 2
  %18 = addrspacecast ptr %17 to ptr addrspace(5)
  call void @llvm.memset.p5.i16(ptr addrspace(5) align 2 %18, i8 0, i16 132, i1 false)
  %19 = addrspacecast ptr %3 to ptr addrspace(5)
  %20 = addrspacecast ptr %3 to ptr addrspace(1)
  %21 = getelementptr inbounds nuw i8, ptr addrspace(1) %20, i16 2
  %22 = addrspacecast ptr %1 to ptr addrspace(1)
  br label %23

23:
  %24 = phi i16 [ 0, %16 ], [ %144, %139 ]
  %25 = phi i16 [ 0, %16 ], [ %142, %139 ]
  %26 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %16 ], [ %140, %139 ]
  %27 = icmp eq i16 %25, %24
  br i1 %27, label %70, label %74

28:
  %sink5 = phi ptr [ %92, %91 ], [ %68, %65 ]
  %29 = getelementptr i8, ptr %sink5, i16 -4
  %30 = load i16, ptr %29
  %31 = getelementptr inbounds i8, ptr %sink5, i16 %30
  store i8 0, ptr %31
  %32 = getelementptr i8, ptr addrspace(5) %19, i16 130
  %33 = load i16, ptr addrspace(5) %32
  %34 = getelementptr i8, ptr addrspace(5) %19, i16 132
  %35 = load i16, ptr addrspace(5) %34
  br label %36

36:
  %37 = phi i16 [ %54, %62 ], [ %81, %89 ], [ %35, %28 ]
  %38 = phi i16 [ %59, %62 ], [ %86, %89 ], [ %33, %28 ]
  %39 = phi ptr [ %39, %62 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %89 ], [ %sink5, %28 ]
  %40 = icmp eq i16 %38, %37
  br i1 %40, label %41, label %45

41:
  %42 = load i16, ptr addrspace(5) %19
  %43 = call addrspace(1) i32 @LL$READ(i16 %42, ptr addrspace(1) nonnull %21, i16 128)
  %44 = icmp slt i32 %43, 0
  br i1 %44, label %175, label %47

45:
  %46 = icmp ult i16 %38, 128
  br i1 %46, label %53, label %64

47:
  %48 = icmp eq i32 %43, 0
  br i1 %48, label %93, label %49

49:
  %50 = getelementptr i8, ptr addrspace(5) %19, i16 130
  store i16 0, ptr addrspace(5) %50
  %51 = trunc i32 %43 to i16
  %52 = getelementptr i8, ptr addrspace(5) %19, i16 132
  store i16 %51, ptr addrspace(5) %52
  br label %53

53:
  %54 = phi i16 [ %51, %49 ], [ %37, %45 ]
  %55 = phi i16 [ 0, %49 ], [ %38, %45 ]
  %56 = getelementptr i8, ptr addrspace(5) %19, i16 %55
  %57 = getelementptr i8, ptr addrspace(5) %56, i16 2
  %58 = load i8, ptr addrspace(5) %57
  %59 = add nuw nsw i16 %55, 1
  %60 = getelementptr i8, ptr addrspace(5) %19, i16 130
  store i16 %59, ptr addrspace(5) %60
  %61 = icmp eq i8 %58, 10
  br i1 %61, label %93, label %62

62:
  %63 = icmp eq i8 %58, 13
  br i1 %63, label %36, label %65

64:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

65:
  %66 = getelementptr i8, ptr %39, i16 -4
  %67 = load i16, ptr %66
  %68 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %39, i16 1, i16 1)
  %69 = getelementptr inbounds i8, ptr %68, i16 %67
  store i8 %58, ptr %69
  br label %28

70:
  %71 = load i16, ptr addrspace(5) %19
  %72 = call addrspace(1) i32 @LL$READ(i16 %71, ptr addrspace(1) nonnull %21, i16 128)
  %73 = icmp slt i32 %72, 1
  br i1 %73, label %175, label %76

74:
  %75 = icmp ult i16 %25, 128
  br i1 %75, label %80, label %64

76:
  %77 = getelementptr i8, ptr addrspace(5) %19, i16 130
  store i16 0, ptr addrspace(5) %77
  %78 = trunc i32 %72 to i16
  %79 = getelementptr i8, ptr addrspace(5) %19, i16 132
  store i16 %78, ptr addrspace(5) %79
  br label %80

80:
  %81 = phi i16 [ %78, %76 ], [ %24, %74 ]
  %82 = phi i16 [ 0, %76 ], [ %25, %74 ]
  %83 = getelementptr i8, ptr addrspace(5) %19, i16 %82
  %84 = getelementptr i8, ptr addrspace(5) %83, i16 2
  %85 = load i8, ptr addrspace(5) %84
  %86 = add nuw nsw i16 %82, 1
  %87 = getelementptr i8, ptr addrspace(5) %19, i16 130
  store i16 %86, ptr addrspace(5) %87
  %88 = icmp eq i8 %85, 10
  br i1 %88, label %93, label %89

89:
  %90 = icmp eq i8 %85, 13
  br i1 %90, label %36, label %91

91:
  %92 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 1, i16 1)
  store i8 %85, ptr %92
  br label %28

93:
  %.ph1 = phi ptr [ %39, %53 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %80 ], [ %39, %47 ]
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %94 = getelementptr i8, ptr %.ph1, i16 -4
  %95 = load i16, ptr %94
  %96 = icmp eq i16 %95, 0
  br i1 %96, label %139, label %97

97:
  %98 = addrspacecast ptr %.ph1 to ptr addrspace(1)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %1)
  br label %105

99:
  %100 = add nuw i16 %106, 1
  %exitcond11 = icmp eq i16 %95, %100
  br i1 %exitcond11, label %101, label %105

101:
  %102 = phi i16 [ %106, %105 ], [ %95, %99 ]
  %103 = add i16 %102, 1
  %104 = icmp ult i16 %103, %95
  br i1 %104, label %110, label %116

105:
  %106 = phi i16 [ 0, %97 ], [ %100, %99 ]
  %107 = getelementptr i8, ptr %.ph1, i16 %106
  %108 = load i8, ptr %107
  %109 = icmp eq i8 %108, 61
  br i1 %109, label %101, label %99

110:
  %111 = getelementptr inbounds i8, ptr %.ph1, i16 %103
  %112 = load i8, ptr %111
  %113 = icmp eq i8 %112, 45
  %114 = add i16 %102, 2
  %cond.fr = freeze i1 %113
  %115 = select i1 %cond.fr, i16 %114, i16 %103
  br label %116

116:
  %117 = phi i1 [ %113, %110 ], [ false, %101 ]
  %118 = phi i16 [ %115, %110 ], [ %103, %101 ]
  %119 = icmp ne i16 %102, 0
  %120 = icmp ult i16 %118, %95
  %121 = and i1 %119, %120
  br i1 %121, label %124, label %145

122:
  %123 = icmp ugt i16 %102, %95
  br i1 %123, label %138, label %147

124:
  %125 = phi i16 [ %137, %133 ], [ %118, %116 ]
  %126 = phi i16 [ %136, %133 ], [ 0, %116 ]
  %127 = getelementptr inbounds i8, ptr %.ph1, i16 %125
  %128 = load i8, ptr %127
  %129 = zext nneg i8 %128 to i16
  %130 = icmp ult i16 %129, 48
  %131 = icmp ugt i16 %129, 57
  %132 = or i1 %130, %131
  br i1 %132, label %145, label %133

133:
  %134 = mul i16 %126, 10
  %135 = add i16 %134, -48
  %136 = add i16 %135, %129
  %137 = add i16 %125, 1
  %exitcond3 = icmp eq i16 %137, %95
  br i1 %exitcond3, label %122, label %124

138:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

139:
  %140 = phi ptr [ %26, %93 ], [ %157, %147 ]
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %.ph1)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %141 = getelementptr i8, ptr addrspace(5) %19, i16 130
  %142 = load i16, ptr addrspace(5) %141
  %143 = getelementptr i8, ptr addrspace(5) %19, i16 132
  %144 = load i16, ptr addrspace(5) %143
  br label %23

145:
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %1)
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %.ph1)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %146 = icmp eq ptr %26, null
  br i1 %146, label %161, label %164

147:
  store i16 %102, ptr %1, !tbaa !2
  %148 = getelementptr i8, ptr %1, i16 2
  store i16 %102, ptr %148, !tbaa !2
  %149 = getelementptr i8, ptr %1, i16 4
  store ptr addrspace(1) %98, ptr %149, !tbaa !2
  %150 = call cc 1005 addrspace(1) ptr @N$VCPY(ptr addrspace(1) %22)
  %151 = sub i16 0, %136
  %152 = select i1 %117, i16 %151, i16 %136
  %153 = ptrtoint ptr %150 to i16
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %1)
  %154 = inttoptr i16 %153 to ptr
  %155 = getelementptr i8, ptr %26, i16 -4
  %156 = load i16, ptr %155
  %157 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %26, i16 1, i16 4)
  %158 = shl i16 %156, 2
  %159 = getelementptr inbounds i8, ptr %157, i16 %158
  store ptr %154, ptr %159
  %160 = getelementptr i8, ptr %159, i16 2
  store i16 %152, ptr %160
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  br label %139

161:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %26)
  %162 = load i16, ptr addrspace(5) %19
  %163 = call addrspace(1) i16 @LL$CLOSE(i16 %162)
  br label %174

164:
  %165 = getelementptr i8, ptr %26, i16 -4
  %166 = load i16, ptr %165
  %167 = icmp eq i16 %166, 0
  br i1 %167, label %161, label %168

168:
  %169 = phi i16 [ %173, %168 ], [ 0, %164 ]
  %170 = shl i16 %169, 2
  %171 = getelementptr inbounds i8, ptr %26, i16 %170
  %172 = load ptr, ptr %171
  call cc 1005 addrspace(1) void @N$BDRP(ptr %172)
  %173 = add nuw i16 %169, 1
  %exitcond31 = icmp eq i16 %173, %166
  br i1 %exitcond31, label %161, label %168

174:
  call void @llvm.lifetime.end.p0(i64 134, ptr nonnull %3)
  br label %381

175:
  %sink1 = phi ptr [ %39, %41 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %70 ]
  call cc 1005 addrspace(1) void @N$BDRP(ptr %sink1)
  %176 = ptrtoint ptr %26 to i16
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %177 = load i16, ptr addrspace(5) %19
  %178 = call addrspace(1) i16 @LL$CLOSE(i16 %177)
  call void @llvm.lifetime.end.p0(i64 134, ptr nonnull %3)
  %179 = inttoptr i16 %176 to ptr
  %180 = getelementptr i8, ptr %179, i16 -4
  %181 = load i16, ptr %180
  %182 = icmp eq i16 %181, 0
  br i1 %182, label %307, label %183

183:
  %184 = addrspacecast ptr %7 to ptr addrspace(1)
  %185 = addrspacecast ptr %6 to ptr addrspace(1)
  br label %186

186:
  %187 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %183 ], [ %228, %227 ]
  %188 = phi i16 [ 0, %183 ], [ %229, %227 ]
  %189 = ptrtoint ptr %26 to i16
  %190 = inttoptr i16 %189 to ptr
  %191 = shl i16 %188, 2
  %192 = getelementptr inbounds i8, ptr %190, i16 %191
  %193 = getelementptr i8, ptr %192, i16 2
  %194 = load i16, ptr %193
  %195 = icmp sgt i16 %194, 0
  br i1 %195, label %201, label %227

196:
  %197 = inttoptr i16 %176 to ptr
  %198 = getelementptr i8, ptr %197, i16 -4
  %199 = load i16, ptr %198
  %200 = icmp eq i16 %199, 0
  br i1 %200, label %307, label %302

201:
  %202 = inttoptr i16 %189 to ptr
  %203 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %187, i16 6)
  %204 = getelementptr i8, ptr %202, i16 %191
  %205 = load ptr, ptr %204
  %206 = call cc 1005 addrspace(1) ptr @N$BCLN(ptr %205, i16 1)
  %207 = getelementptr i8, ptr %206, i16 -4
  %208 = load i16, ptr %207
  %209 = addrspacecast ptr %206 to ptr addrspace(1)
  %210 = icmp eq i16 %208, 0
  br i1 %210, label %222, label %211

211:
  %212 = phi i16 [ %218, %211 ], [ 5381, %201 ]
  %213 = phi i16 [ %219, %211 ], [ 0, %201 ]
  %214 = getelementptr inbounds i8, ptr %206, i16 %213
  %215 = mul i16 %212, 33
  %216 = load i8, ptr %214
  %217 = zext i8 %216 to i16
  %218 = xor i16 %215, %217
  %219 = add nuw i16 %213, 1
  %exitcond1 = icmp eq i16 %219, %208
  br i1 %exitcond1, label %220, label %211

220:
  %221 = or i16 %218, 1
  br label %222

222:
  %223 = phi i16 [ 5381, %201 ], [ %221, %220 ]
  %224 = getelementptr i8, ptr %203, i16 -4
  %225 = load i16, ptr %224
  %226 = icmp eq i16 %225, 0
  br i1 %226, label %273, label %230

227:
  %228 = phi ptr [ %203, %287 ], [ %187, %186 ]
  %229 = add nuw i16 %188, 1
  %exitcond6 = icmp eq i16 %229, %181
  br i1 %exitcond6, label %196, label %186

230:
  %231 = add i16 %225, -1
  %232 = and i16 %231, %223
  br label %233

233:
  %234 = phi i16 [ %225, %230 ], [ %242, %241 ]
  %235 = phi i16 [ %232, %230 ], [ %244, %241 ]
  %236 = mul i16 %235, 6
  %237 = getelementptr inbounds i8, ptr %203, i16 %236
  %238 = load i16, ptr %237
  %239 = icmp eq i16 %238, 0
  br i1 %239, label %266, label %246

240:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

241:
  %242 = phi i16 [ %265, %263 ], [ %234, %246 ]
  %243 = add nuw i16 %235, 1
  %244 = and i16 %243, %231
  %245 = icmp ult i16 %244, %242
  br i1 %245, label %233, label %240

246:
  %247 = icmp eq i16 %238, %223
  br i1 %247, label %248, label %241

248:
  %249 = getelementptr i8, ptr %203, i16 %236
  %250 = getelementptr i8, ptr %249, i16 2
  %251 = load ptr, ptr %250
  %252 = getelementptr i8, ptr %251, i16 -4
  %253 = load i16, ptr %252
  %254 = addrspacecast ptr %251 to ptr addrspace(1)
  store i16 %253, ptr %7, !tbaa !2
  %255 = getelementptr i8, ptr %7, i16 2
  store i16 %253, ptr %255, !tbaa !2
  %256 = getelementptr i8, ptr %7, i16 4
  store ptr addrspace(1) %254, ptr %256, !tbaa !2
  %257 = getelementptr i8, ptr %206, i16 -4
  %258 = load i16, ptr %257
  store i16 %258, ptr %6, !tbaa !2
  %259 = getelementptr i8, ptr %6, i16 2
  store i16 %258, ptr %259, !tbaa !2
  %260 = getelementptr i8, ptr %6, i16 4
  store ptr addrspace(1) %209, ptr %260, !tbaa !2
  %261 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %184, ptr addrspace(1) %185)
  %262 = icmp eq i8 %261, 0
  br i1 %262, label %282, label %263

263:
  %264 = getelementptr i8, ptr %203, i16 -4
  %265 = load i16, ptr %264
  br label %241

266:
  %267 = icmp ult i16 %235, %234
  br i1 %267, label %268, label %273

268:
  %269 = getelementptr i8, ptr %203, i16 %236
  store i16 %223, ptr %269
  %270 = getelementptr i8, ptr %203, i16 -4
  %271 = load i16, ptr %270
  %272 = icmp ult i16 %235, %271
  br i1 %272, label %275, label %274

273:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

274:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

275:
  %276 = getelementptr i8, ptr %203, i16 %236
  %277 = getelementptr i8, ptr %276, i16 2
  %278 = load ptr, ptr %277
  call cc 1005 addrspace(1) void @N$BDRP(ptr %278)
  store ptr %206, ptr %277
  %279 = getelementptr i8, ptr %203, i16 -2
  %280 = load i16, ptr %279
  %281 = add i16 %280, 1
  store i16 %281, ptr %279
  br label %282

282:
  %283 = phi ptr [ null, %275 ], [ %206, %248 ]
  %284 = getelementptr i8, ptr %203, i16 -4
  %285 = load i16, ptr %284
  %286 = icmp ult i16 %235, %285
  br i1 %286, label %287, label %294

287:
  %288 = inttoptr i16 %189 to ptr
  %289 = getelementptr i8, ptr %288, i16 %191
  %290 = getelementptr i8, ptr %289, i16 2
  %291 = load i16, ptr %290
  %292 = getelementptr i8, ptr %203, i16 %236
  %293 = getelementptr i8, ptr %292, i16 4
  store i16 %291, ptr %293
  call cc 1005 addrspace(1) void @N$BDRP(ptr %283)
  br label %227

294:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

295:
  %296 = phi ptr [ %308, %307 ], [ %228, %302 ]
  call cc 1005 addrspace(1) void @N$PN()
  %297 = getelementptr i8, ptr %296, i16 -2
  %298 = load i16, ptr %297
  call cc 1005 addrspace(1) void @N$PU2(i16 %298)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  %299 = getelementptr i8, ptr %296, i16 -4
  %300 = load i16, ptr %299
  %301 = icmp eq i16 %300, 0
  br i1 %301, label %345, label %309

302:
  %303 = ptrtoint ptr %26 to i16
  %304 = inttoptr i16 %303 to ptr
  %305 = add i16 %199, -1
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  %306 = load ptr, ptr %304
  call cc 1005 addrspace(1) void @N$PS(ptr %306)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %305)
  br label %295

307:
  %308 = phi ptr [ %228, %196 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %175 ]
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  br label %295

309:
  %310 = add i16 %300, -1
  %311 = and i16 %310, 29987
  %312 = addrspacecast ptr %5 to ptr addrspace(1)
  %313 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %314

314:
  %315 = phi i16 [ %300, %309 ], [ %327, %326 ]
  %316 = phi i16 [ %311, %309 ], [ %329, %326 ]
  %317 = mul i16 %316, 6
  %318 = getelementptr inbounds i8, ptr %296, i16 %317
  %319 = load i16, ptr %318
  %320 = icmp eq i16 %319, 0
  br i1 %320, label %345, label %321

321:
  %322 = icmp eq i16 %319, 29987
  br i1 %322, label %331, label %326

323:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

324:
  %325 = icmp ult i16 %316, %344
  br i1 %325, label %351, label %346

326:
  %327 = phi i16 [ %344, %331 ], [ %315, %321 ]
  %328 = add nuw i16 %316, 1
  %329 = and i16 %328, %310
  %330 = icmp ult i16 %329, %327
  br i1 %330, label %314, label %323

331:
  %332 = getelementptr i8, ptr %296, i16 %317
  %333 = getelementptr i8, ptr %332, i16 2
  %334 = load ptr, ptr %333
  %335 = getelementptr i8, ptr %334, i16 -4
  %336 = load i16, ptr %335
  %337 = addrspacecast ptr %334 to ptr addrspace(1)
  store i16 %336, ptr %5, !tbaa !2
  %338 = getelementptr i8, ptr %5, i16 2
  store i16 %336, ptr %338, !tbaa !2
  %339 = getelementptr i8, ptr %5, i16 4
  store ptr addrspace(1) %337, ptr %339, !tbaa !2
  store i32 327685, ptr %4
  %340 = getelementptr i8, ptr %4, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6) to ptr addrspace(1)), ptr %340, !tbaa !2
  %341 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %312, ptr addrspace(1) %313)
  %342 = icmp eq i8 %341, 0
  %343 = getelementptr i8, ptr %296, i16 -4
  %344 = load i16, ptr %343
  br i1 %342, label %324, label %326

345:
  call cc 1005 addrspace(1) void @N$EKEY()
  unreachable

346:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

347:
  %348 = ptrtoint ptr %26 to i16
  %349 = inttoptr i16 %348 to ptr
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %296)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %350 = icmp eq ptr %349, null
  br i1 %350, label %365, label %368

351:
  %352 = getelementptr i8, ptr %296, i16 %317
  %353 = getelementptr i8, ptr %352, i16 4
  %354 = load i16, ptr %353
  call cc 1005 addrspace(1) void @N$PI2(i16 %354)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  %355 = getelementptr i8, ptr %296, i16 -4
  %356 = load i16, ptr %355
  %357 = icmp eq i16 %356, 0
  br i1 %357, label %347, label %358

358:
  %359 = phi i16 [ %364, %358 ], [ 0, %351 ]
  %360 = mul i16 %359, 6
  %361 = getelementptr inbounds i8, ptr %296, i16 %360
  %362 = getelementptr i8, ptr %361, i16 2
  %363 = load ptr, ptr %362
  call cc 1005 addrspace(1) void @N$BDRP(ptr %363)
  %364 = add nuw i16 %359, 1
  %exitcond5 = icmp eq i16 %364, %356
  br i1 %exitcond5, label %347, label %358

365:
  %366 = ptrtoint ptr %26 to i16
  %367 = inttoptr i16 %366 to ptr
  call cc 1005 addrspace(1) void @N$BDRP(ptr %367)
  br label %381

368:
  %369 = inttoptr i16 %176 to ptr
  %370 = getelementptr i8, ptr %369, i16 -4
  %371 = load i16, ptr %370
  %372 = icmp eq i16 %371, 0
  br i1 %372, label %365, label %373

373:
  %374 = phi i16 [ %380, %373 ], [ 0, %368 ]
  %375 = ptrtoint ptr %26 to i16
  %376 = inttoptr i16 %375 to ptr
  %377 = shl i16 %374, 2
  %378 = getelementptr inbounds i8, ptr %376, i16 %377
  %379 = load ptr, ptr %378
  call cc 1005 addrspace(1) void @N$BDRP(ptr %379)
  %380 = add nuw i16 %374, 1
  %exitcond4 = icmp eq i16 %380, %371
  br i1 %exitcond4, label %365, label %373

381:
  %382 = phi i16 [ 1, %174 ], [ 0, %365 ]
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %4)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %5)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %6)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %7)
  ret i16 %382
}

declare i32 @LL$WRITE_FILE(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) noalias nocapture writeonly, ptr noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare i16 @LL$OPEN(ptr addrspace(1), i8) addrspace(1)

declare i16 @LL$CREATE(ptr addrspace(1)) addrspace(1)

declare i32 @LL$READ(i16, ptr addrspace(1), i16) addrspace(1)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare i16 @LL$CLOSE(i16) addrspace(1)

declare cc 1005 ptr @N$VCPY(ptr addrspace(1)) addrspace(1)

declare cc 1005 ptr @N$DRES(ptr, i16) addrspace(1)

declare cc 1005 ptr @N$BCLN(ptr, i16) addrspace(1)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$EKEY() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

declare void @llvm.memset.p1.i16(ptr addrspace(1) nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) noalias nocapture writeonly, ptr addrspace(1) noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

declare void @llvm.memset.p5.i16(ptr addrspace(5) nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
!5 = !{i8 0, i8 2}
!6 = !{i8 0, i8 2}
!7 = !{i8 0, i8 2}
!8 = !{i8 0, i8 2}
!9 = !{i8 0, i8 2}
!10 = !{i8 0, i8 2}
!11 = !{i8 0, i8 2}
!12 = !{i8 0, i8 2}
!13 = !{i8 0, i8 2}
!14 = !{i8 0, i8 2}
!15 = !{i8 0, i8 2}
!16 = !{i8 0, i8 2}
!17 = !{i8 0, i8 3}
!18 = !{i8 0, i8 2}
!19 = !{i8 0, i8 2}
