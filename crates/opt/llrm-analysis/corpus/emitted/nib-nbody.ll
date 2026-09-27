target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [10 x i8] c"\08\00\03\00\03\00PX=\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00PY=\00"
@$str3 = internal constant [10 x i8] c"\08\00\03\00\03\00VX=\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00VY=\00"
@$str5 = internal constant [11 x i8] c"\08\00\04\00\04\00DONE\00"

define internal i32 @nbody(i32 %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i32
  %4 = alloca i32
  %5 = alloca [8 x i8]
  %6 = alloca i16
  %7 = alloca [8 x i8]
  %8 = alloca i16
  %9 = alloca i32
  %10 = alloca i32
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca [96 x i8]
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i32 0, ptr %3
  store i32 0, ptr %4
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  store i16 0, ptr %6
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  store i16 0, ptr %8
  store i32 0, ptr %9
  store i32 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  call void @llvm.memset.p0.i16(ptr %13, i8 0, i16 96, i1 false)
  store i16 6, ptr %11, !tbaa !2
  store i16 6, ptr %12, !tbaa !2
  %14 = sub i16 0, 0
  %15 = getelementptr inbounds [16 x i8], ptr %13, i16 %14
  store i32 -7680, ptr %15, !tbaa !2
  %16 = sub i16 0, 0
  %17 = getelementptr inbounds [16 x i8], ptr %13, i16 %16
  %18 = getelementptr inbounds i8, ptr %17, i16 4
  store i32 -6144, ptr %18, !tbaa !2
  %19 = sub i16 0, 0
  %20 = getelementptr inbounds [16 x i8], ptr %13, i16 %19
  %21 = getelementptr inbounds i8, ptr %20, i16 8
  store i32 0, ptr %21, !tbaa !2
  %22 = sub i16 0, 0
  %23 = getelementptr inbounds [16 x i8], ptr %13, i16 %22
  %24 = getelementptr inbounds i8, ptr %23, i16 12
  store i32 0, ptr %24, !tbaa !2
  %25 = sub i16 1, 0
  %26 = getelementptr inbounds [16 x i8], ptr %13, i16 %25
  store i32 -4096, ptr %26, !tbaa !2
  %27 = sub i16 1, 0
  %28 = getelementptr inbounds [16 x i8], ptr %13, i16 %27
  %29 = getelementptr inbounds i8, ptr %28, i16 4
  store i32 -3584, ptr %29, !tbaa !2
  %30 = sub i16 1, 0
  %31 = getelementptr inbounds [16 x i8], ptr %13, i16 %30
  %32 = getelementptr inbounds i8, ptr %31, i16 8
  store i32 0, ptr %32, !tbaa !2
  %33 = sub i16 1, 0
  %34 = getelementptr inbounds [16 x i8], ptr %13, i16 %33
  %35 = getelementptr inbounds i8, ptr %34, i16 12
  store i32 0, ptr %35, !tbaa !2
  %36 = sub i16 2, 0
  %37 = getelementptr inbounds [16 x i8], ptr %13, i16 %36
  store i32 -512, ptr %37, !tbaa !2
  %38 = sub i16 2, 0
  %39 = getelementptr inbounds [16 x i8], ptr %13, i16 %38
  %40 = getelementptr inbounds i8, ptr %39, i16 4
  store i32 -1024, ptr %40, !tbaa !2
  %41 = sub i16 2, 0
  %42 = getelementptr inbounds [16 x i8], ptr %13, i16 %41
  %43 = getelementptr inbounds i8, ptr %42, i16 8
  store i32 0, ptr %43, !tbaa !2
  %44 = sub i16 2, 0
  %45 = getelementptr inbounds [16 x i8], ptr %13, i16 %44
  %46 = getelementptr inbounds i8, ptr %45, i16 12
  store i32 0, ptr %46, !tbaa !2
  %47 = sub i16 3, 0
  %48 = getelementptr inbounds [16 x i8], ptr %13, i16 %47
  store i32 3072, ptr %48, !tbaa !2
  %49 = sub i16 3, 0
  %50 = getelementptr inbounds [16 x i8], ptr %13, i16 %49
  %51 = getelementptr inbounds i8, ptr %50, i16 4
  store i32 1536, ptr %51, !tbaa !2
  %52 = sub i16 3, 0
  %53 = getelementptr inbounds [16 x i8], ptr %13, i16 %52
  %54 = getelementptr inbounds i8, ptr %53, i16 8
  store i32 0, ptr %54, !tbaa !2
  %55 = sub i16 3, 0
  %56 = getelementptr inbounds [16 x i8], ptr %13, i16 %55
  %57 = getelementptr inbounds i8, ptr %56, i16 12
  store i32 0, ptr %57, !tbaa !2
  %58 = sub i16 4, 0
  %59 = getelementptr inbounds [16 x i8], ptr %13, i16 %58
  store i32 6656, ptr %59, !tbaa !2
  %60 = sub i16 4, 0
  %61 = getelementptr inbounds [16 x i8], ptr %13, i16 %60
  %62 = getelementptr inbounds i8, ptr %61, i16 4
  store i32 4096, ptr %62, !tbaa !2
  %63 = sub i16 4, 0
  %64 = getelementptr inbounds [16 x i8], ptr %13, i16 %63
  %65 = getelementptr inbounds i8, ptr %64, i16 8
  store i32 0, ptr %65, !tbaa !2
  %66 = sub i16 4, 0
  %67 = getelementptr inbounds [16 x i8], ptr %13, i16 %66
  %68 = getelementptr inbounds i8, ptr %67, i16 12
  store i32 0, ptr %68, !tbaa !2
  %69 = sub i16 5, 0
  %70 = getelementptr inbounds [16 x i8], ptr %13, i16 %69
  store i32 10240, ptr %70, !tbaa !2
  %71 = sub i16 5, 0
  %72 = getelementptr inbounds [16 x i8], ptr %13, i16 %71
  %73 = getelementptr inbounds i8, ptr %72, i16 4
  store i32 6656, ptr %73, !tbaa !2
  %74 = sub i16 5, 0
  %75 = getelementptr inbounds [16 x i8], ptr %13, i16 %74
  %76 = getelementptr inbounds i8, ptr %75, i16 8
  store i32 0, ptr %76, !tbaa !2
  %77 = sub i16 5, 0
  %78 = getelementptr inbounds [16 x i8], ptr %13, i16 %77
  %79 = getelementptr inbounds i8, ptr %78, i16 12
  store i32 0, ptr %79, !tbaa !2
  store i32 0, ptr %10, !tbaa !2
  store i32 %0, ptr %9, !tbaa !2
  br label %b2

b2:
  %80 = load i32, ptr %10, !tbaa !2
  %81 = load i32, ptr %9, !tbaa !2
  %82 = icmp slt i32 %80, %81
  %83 = sext i1 %82 to i8
  %84 = icmp ne i8 %83, 0
  br i1 %84, label %b3, label %b5

b3:
  store i16 0, ptr %8, !tbaa !2
  br label %b6

b4:
  %85 = load i32, ptr %10, !tbaa !2
  %86 = add i32 %85, 1
  store i32 %86, ptr %10, !tbaa !2
  br label %b2

b5:
  store i16 0, ptr %1, !tbaa !2
  br label %b21

b6:
  %87 = load i16, ptr %8, !tbaa !2
  %88 = icmp ult i16 %87, 6
  %89 = sext i1 %88 to i8
  %90 = icmp ne i8 %89, 0
  br i1 %90, label %b7, label %b9

b7:
  store i32 0, ptr %7, !tbaa !2
  %91 = getelementptr inbounds i8, ptr %7, i16 4
  store i32 0, ptr %91, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  br label %b10

b8:
  %92 = load i16, ptr %8, !tbaa !2
  %93 = add i16 %92, 1
  store i16 %93, ptr %8, !tbaa !2
  br label %b6

b9:
  store i16 0, ptr %2, !tbaa !2
  br label %b17

b10:
  %94 = load i16, ptr %6, !tbaa !2
  %95 = icmp ult i16 %94, 6
  %96 = sext i1 %95 to i8
  %97 = icmp ne i8 %96, 0
  br i1 %97, label %b11, label %b13

b11:
  %98 = icmp ne i16 %87, %94
  %99 = sext i1 %98 to i8
  %100 = icmp ne i8 %99, 0
  br i1 %100, label %b14, label %b15

b12:
  %101 = load i16, ptr %6, !tbaa !2
  %102 = add i16 %101, 1
  store i16 %102, ptr %6, !tbaa !2
  br label %b10

b13:
  %103 = sub i16 %87, 0
  %104 = getelementptr inbounds [16 x i8], ptr %13, i16 %103
  %105 = getelementptr inbounds i8, ptr %104, i16 8
  %106 = load i32, ptr %105, !tbaa !2
  %107 = load i32, ptr %7, !tbaa !2
  %108 = add i32 %106, %107
  %109 = sub i16 %87, 0
  %110 = getelementptr inbounds [16 x i8], ptr %13, i16 %109
  %111 = getelementptr inbounds i8, ptr %110, i16 8
  store i32 %108, ptr %111, !tbaa !2
  %112 = sub i16 %87, 0
  %113 = getelementptr inbounds [16 x i8], ptr %13, i16 %112
  %114 = getelementptr inbounds i8, ptr %113, i16 12
  %115 = load i32, ptr %114, !tbaa !2
  %116 = getelementptr inbounds i8, ptr %7, i16 4
  %117 = load i32, ptr %116, !tbaa !2
  %118 = add i32 %115, %117
  %119 = sub i16 %87, 0
  %120 = getelementptr inbounds [16 x i8], ptr %13, i16 %119
  %121 = getelementptr inbounds i8, ptr %120, i16 12
  store i32 %118, ptr %121, !tbaa !2
  %122 = sub i16 %87, 0
  %123 = getelementptr inbounds [16 x i8], ptr %13, i16 %122
  %124 = getelementptr inbounds i8, ptr %123, i16 8
  %125 = load i32, ptr %124, !tbaa !2
  %126 = sub i16 %87, 0
  %127 = getelementptr inbounds [16 x i8], ptr %13, i16 %126
  %128 = getelementptr inbounds i8, ptr %127, i16 8
  %129 = load i32, ptr %128, !tbaa !2
  %130 = sext i32 %129 to i64
  %131 = sext i32 8192 to i64
  %132 = shl i64 %130, 9
  %133 = sdiv i64 %132, %131
  %134 = trunc i64 %133 to i32
  %135 = sub i32 %125, %134
  %136 = sub i16 %87, 0
  %137 = getelementptr inbounds [16 x i8], ptr %13, i16 %136
  %138 = getelementptr inbounds i8, ptr %137, i16 8
  store i32 %135, ptr %138, !tbaa !2
  %139 = sub i16 %87, 0
  %140 = getelementptr inbounds [16 x i8], ptr %13, i16 %139
  %141 = getelementptr inbounds i8, ptr %140, i16 12
  %142 = load i32, ptr %141, !tbaa !2
  %143 = sub i16 %87, 0
  %144 = getelementptr inbounds [16 x i8], ptr %13, i16 %143
  %145 = getelementptr inbounds i8, ptr %144, i16 12
  %146 = load i32, ptr %145, !tbaa !2
  %147 = sext i32 %146 to i64
  %148 = sext i32 8192 to i64
  %149 = shl i64 %147, 9
  %150 = sdiv i64 %149, %148
  %151 = trunc i64 %150 to i32
  %152 = sub i32 %142, %151
  %153 = sub i16 %87, 0
  %154 = getelementptr inbounds [16 x i8], ptr %13, i16 %153
  %155 = getelementptr inbounds i8, ptr %154, i16 12
  store i32 %152, ptr %155, !tbaa !2
  br label %b8

b14:
  %156 = sub i16 %94, 0
  %157 = getelementptr inbounds [16 x i8], ptr %13, i16 %156
  %158 = load i32, ptr %157, !tbaa !2
  %159 = sub i16 %87, 0
  %160 = getelementptr inbounds [16 x i8], ptr %13, i16 %159
  %161 = load i32, ptr %160, !tbaa !2
  %162 = sub i32 %158, %161
  %163 = sub i16 %94, 0
  %164 = getelementptr inbounds [16 x i8], ptr %13, i16 %163
  %165 = getelementptr inbounds i8, ptr %164, i16 4
  %166 = load i32, ptr %165, !tbaa !2
  %167 = sub i16 %87, 0
  %168 = getelementptr inbounds [16 x i8], ptr %13, i16 %167
  %169 = getelementptr inbounds i8, ptr %168, i16 4
  %170 = load i32, ptr %169, !tbaa !2
  %171 = sub i32 %166, %170
  store i32 %162, ptr %5, !tbaa !2
  %172 = getelementptr inbounds i8, ptr %5, i16 4
  store i32 %171, ptr %172, !tbaa !2
  %173 = load i32, ptr %5, !tbaa !2
  %174 = load i32, ptr %5, !tbaa !2
  %175 = sext i32 %173 to i64
  %176 = sext i32 %174 to i64
  %177 = mul i64 %175, %176
  %178 = ashr i64 %177, 9
  %179 = trunc i64 %178 to i32
  %180 = getelementptr inbounds i8, ptr %5, i16 4
  %181 = load i32, ptr %180, !tbaa !2
  %182 = getelementptr inbounds i8, ptr %5, i16 4
  %183 = load i32, ptr %182, !tbaa !2
  %184 = sext i32 %181 to i64
  %185 = sext i32 %183 to i64
  %186 = mul i64 %184, %185
  %187 = ashr i64 %186, 9
  %188 = trunc i64 %187 to i32
  %189 = add i32 %179, %188
  %190 = add i32 %189, 512
  store i32 %190, ptr %4, !tbaa !2
  %191 = load i32, ptr %4, !tbaa !2
  %192 = sext i32 512 to i64
  %193 = sext i32 %191 to i64
  %194 = shl i64 %192, 9
  %195 = sdiv i64 %194, %193
  %196 = trunc i64 %195 to i32
  store i32 %196, ptr %3, !tbaa !2
  %197 = load i32, ptr %7, !tbaa !2
  %198 = load i32, ptr %5, !tbaa !2
  %199 = load i32, ptr %3, !tbaa !2
  %200 = sext i32 %198 to i64
  %201 = sext i32 %199 to i64
  %202 = mul i64 %200, %201
  %203 = ashr i64 %202, 9
  %204 = trunc i64 %203 to i32
  %205 = add i32 %197, %204
  store i32 %205, ptr %7, !tbaa !2
  %206 = getelementptr inbounds i8, ptr %7, i16 4
  %207 = load i32, ptr %206, !tbaa !2
  %208 = getelementptr inbounds i8, ptr %5, i16 4
  %209 = load i32, ptr %208, !tbaa !2
  %210 = load i32, ptr %3, !tbaa !2
  %211 = sext i32 %209 to i64
  %212 = sext i32 %210 to i64
  %213 = mul i64 %211, %212
  %214 = ashr i64 %213, 9
  %215 = trunc i64 %214 to i32
  %216 = add i32 %207, %215
  %217 = getelementptr inbounds i8, ptr %7, i16 4
  store i32 %216, ptr %217, !tbaa !2
  br label %b16

b15:
  br label %b16

b16:
  br label %b12

b17:
  %218 = load i16, ptr %2, !tbaa !2
  %219 = icmp ult i16 %218, 6
  %220 = sext i1 %219 to i8
  %221 = icmp ne i8 %220, 0
  br i1 %221, label %b18, label %b20

b18:
  %222 = sub i16 %218, 0
  %223 = getelementptr inbounds [16 x i8], ptr %13, i16 %222
  %224 = load i32, ptr %223, !tbaa !2
  %225 = sub i16 %218, 0
  %226 = getelementptr inbounds [16 x i8], ptr %13, i16 %225
  %227 = getelementptr inbounds i8, ptr %226, i16 8
  %228 = load i32, ptr %227, !tbaa !2
  %229 = add i32 %224, %228
  %230 = sub i16 %218, 0
  %231 = getelementptr inbounds [16 x i8], ptr %13, i16 %230
  store i32 %229, ptr %231, !tbaa !2
  %232 = sub i16 %218, 0
  %233 = getelementptr inbounds [16 x i8], ptr %13, i16 %232
  %234 = getelementptr inbounds i8, ptr %233, i16 4
  %235 = load i32, ptr %234, !tbaa !2
  %236 = sub i16 %218, 0
  %237 = getelementptr inbounds [16 x i8], ptr %13, i16 %236
  %238 = getelementptr inbounds i8, ptr %237, i16 12
  %239 = load i32, ptr %238, !tbaa !2
  %240 = add i32 %235, %239
  %241 = sub i16 %218, 0
  %242 = getelementptr inbounds [16 x i8], ptr %13, i16 %241
  %243 = getelementptr inbounds i8, ptr %242, i16 4
  store i32 %240, ptr %243, !tbaa !2
  br label %b19

b19:
  %244 = load i16, ptr %2, !tbaa !2
  %245 = add i16 %244, 1
  store i16 %245, ptr %2, !tbaa !2
  br label %b17

b20:
  br label %b4

b21:
  %246 = load i16, ptr %1, !tbaa !2
  %247 = icmp ult i16 %246, 6
  %248 = sext i1 %247 to i8
  %249 = icmp ne i8 %248, 0
  br i1 %249, label %b22, label %b24

b22:
  %250 = getelementptr i8, ptr @$str1, i16 6
  call addrspace(1) void @N$PS(ptr %250)
  %251 = sub i16 %246, 0
  %252 = getelementptr inbounds [16 x i8], ptr %13, i16 %251
  %253 = load i32, ptr %252, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %253, i8 9)
  call addrspace(1) void @N$PN()
  %254 = getelementptr i8, ptr @$str2, i16 6
  call addrspace(1) void @N$PS(ptr %254)
  %255 = sub i16 %246, 0
  %256 = getelementptr inbounds [16 x i8], ptr %13, i16 %255
  %257 = getelementptr inbounds i8, ptr %256, i16 4
  %258 = load i32, ptr %257, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %258, i8 9)
  call addrspace(1) void @N$PN()
  %259 = getelementptr i8, ptr @$str3, i16 6
  call addrspace(1) void @N$PS(ptr %259)
  %260 = sub i16 %246, 0
  %261 = getelementptr inbounds [16 x i8], ptr %13, i16 %260
  %262 = getelementptr inbounds i8, ptr %261, i16 8
  %263 = load i32, ptr %262, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %263, i8 9)
  call addrspace(1) void @N$PN()
  %264 = getelementptr i8, ptr @$str4, i16 6
  call addrspace(1) void @N$PS(ptr %264)
  %265 = sub i16 %246, 0
  %266 = getelementptr inbounds [16 x i8], ptr %13, i16 %265
  %267 = getelementptr inbounds i8, ptr %266, i16 12
  %268 = load i32, ptr %267, !tbaa !2
  call addrspace(1) void @N$PQ4(i32 %268, i8 9)
  call addrspace(1) void @N$PN()
  br label %b23

b23:
  %269 = load i16, ptr %1, !tbaa !2
  %270 = add i16 %269, 1
  store i16 %270, ptr %1, !tbaa !2
  br label %b21

b24:
  %271 = getelementptr i8, ptr @$str5, i16 6
  call addrspace(1) void @N$PS(ptr %271)
  call addrspace(1) void @N$PN()
  %272 = sub i16 0, 0
  %273 = getelementptr inbounds [16 x i8], ptr %13, i16 %272
  %274 = load i32, ptr %273, !tbaa !2
  %275 = sub i16 1, 0
  %276 = getelementptr inbounds [16 x i8], ptr %13, i16 %275
  %277 = getelementptr inbounds i8, ptr %276, i16 4
  %278 = load i32, ptr %277, !tbaa !2
  %279 = add i32 %274, %278
  %280 = sub i16 2, 0
  %281 = getelementptr inbounds [16 x i8], ptr %13, i16 %280
  %282 = load i32, ptr %281, !tbaa !2
  %283 = add i32 %279, %282
  %284 = sub i16 3, 0
  %285 = getelementptr inbounds [16 x i8], ptr %13, i16 %284
  %286 = getelementptr inbounds i8, ptr %285, i16 4
  %287 = load i32, ptr %286, !tbaa !2
  %288 = add i32 %283, %287
  ret i32 %288
}

define internal i16 @main() addrspace(1) {
b1:
  %0 = call addrspace(1) i32 @nbody(i32 1)
  ret i16 0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PQ4(i32, i8) addrspace(1)

declare void @N$PN() addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
