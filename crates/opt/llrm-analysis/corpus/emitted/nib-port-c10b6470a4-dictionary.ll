target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i8
  %2 = alloca i16
  %3 = alloca i8
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i8
  %9 = alloca i16
  %10 = alloca i8
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i8
  %16 = alloca i16
  %17 = alloca i8
  %18 = alloca i16
  %19 = alloca i16
  %20 = alloca i16
  %21 = alloca ptr
  %22 = alloca i8
  %23 = alloca i16
  %24 = alloca i8
  %25 = alloca i16
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca ptr
  %30 = alloca i16
  %31 = alloca i16
  %32 = alloca [8 x i8]
  store i16 0, ptr %0
  store i8 0, ptr %1
  store i16 0, ptr %2
  store i8 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i8 0, ptr %8
  store i16 0, ptr %9
  store i8 0, ptr %10
  store i16 0, ptr %11
  store i16 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i8 0, ptr %15
  store i16 0, ptr %16
  store i8 0, ptr %17
  store i16 0, ptr %18
  store i16 0, ptr %19
  store i16 0, ptr %20
  store ptr null, ptr %21
  store i8 0, ptr %22
  store i16 0, ptr %23
  store i8 0, ptr %24
  store i16 0, ptr %25
  store i16 0, ptr %26
  store i16 0, ptr %27
  store i16 0, ptr %28
  store ptr null, ptr %29
  store i16 0, ptr %30
  store i16 0, ptr %31
  call void @llvm.memset.p0.i16(ptr %32, i8 0, i16 8, i1 false)
  store i16 4, ptr %30, !tbaa !2
  store i16 4, ptr %31, !tbaa !2
  %33 = sub i16 0, 0
  %34 = getelementptr inbounds i16, ptr %32, i16 %33
  store i16 1, ptr %34, !tbaa !2
  %35 = sub i16 1, 0
  %36 = getelementptr inbounds i16, ptr %32, i16 %35
  store i16 2, ptr %36, !tbaa !2
  %37 = sub i16 2, 0
  %38 = getelementptr inbounds i16, ptr %32, i16 %37
  store i16 1, ptr %38, !tbaa !2
  %39 = sub i16 3, 0
  %40 = getelementptr inbounds i16, ptr %32, i16 %39
  store i16 3, ptr %40, !tbaa !2
  %41 = getelementptr i8, ptr @$str1, i16 6
  store ptr %41, ptr %29, !tbaa !2
  store i16 0, ptr %28, !tbaa !2
  br label %b2

b2:
  %42 = load i16, ptr %28, !tbaa !2
  %43 = icmp ult i16 %42, 4
  %44 = sext i1 %43 to i8
  %45 = icmp ne i8 %44, 0
  br i1 %45, label %b3, label %b5

b3:
  %46 = load ptr, ptr %29, !tbaa !2
  %47 = call addrspace(1) ptr @N$DRES(ptr %46, i16 6)
  store ptr %47, ptr %29, !tbaa !2
  %48 = sub i16 %42, 0
  %49 = getelementptr inbounds i16, ptr %32, i16 %48
  %50 = load i16, ptr %49, !tbaa !2
  store i16 %50, ptr %27, !tbaa !2
  %51 = load i16, ptr %27, !tbaa !2
  %52 = call addrspace(1) i16 @i16.hash(i16 %51)
  %53 = or i16 %52, 1
  store i16 %53, ptr %26, !tbaa !2
  store i16 0, ptr %25, !tbaa !2
  store i8 0, ptr %24, !tbaa !2
  %54 = getelementptr i8, ptr %47, i16 -4
  %55 = load i16, ptr %54
  %56 = icmp ne i16 %55, 0
  %57 = sext i1 %56 to i8
  %58 = icmp ne i8 %57, 0
  br i1 %58, label %b6, label %b7

b4:
  %59 = load i16, ptr %28, !tbaa !2
  %60 = add i16 %59, 1
  store i16 %60, ptr %28, !tbaa !2
  br label %b2

b5:
  %61 = load ptr, ptr %29, !tbaa !2
  store ptr null, ptr %29, !tbaa !2
  store ptr %61, ptr %21, !tbaa !2
  %62 = load ptr, ptr %21, !tbaa !2
  store i16 1, ptr %20, !tbaa !2
  %63 = load i16, ptr %20, !tbaa !2
  %64 = call addrspace(1) i16 @i16.hash(i16 %63)
  %65 = or i16 %64, 1
  store i16 %65, ptr %19, !tbaa !2
  store i16 0, ptr %18, !tbaa !2
  store i8 0, ptr %17, !tbaa !2
  %66 = getelementptr i8, ptr %62, i16 -4
  %67 = load i16, ptr %66
  %68 = icmp ne i16 %67, 0
  %69 = sext i1 %68 to i8
  %70 = icmp ne i8 %69, 0
  br i1 %70, label %b34, label %b35

b6:
  %71 = getelementptr i8, ptr %47, i16 -4
  %72 = load i16, ptr %71
  %73 = sub i16 %72, 1
  store i16 %73, ptr %23, !tbaa !2
  %74 = load i16, ptr %26, !tbaa !2
  %75 = load i16, ptr %23, !tbaa !2
  %76 = and i16 %74, %75
  store i16 %76, ptr %25, !tbaa !2
  br label %b9

b7:
  br label %b8

b8:
  %77 = load i8, ptr %24, !tbaa !2
  %78 = xor i8 %77, -1
  %79 = icmp ne i8 %78, 0
  br i1 %79, label %b23, label %b24

b9:
  %80 = load i16, ptr %25, !tbaa !2
  %81 = getelementptr i8, ptr %47, i16 -4
  %82 = load i16, ptr %81
  %83 = icmp ult i16 %80, %82
  %84 = sext i1 %83 to i8
  %85 = icmp ne i8 %84, 0
  br i1 %85, label %b12, label %b13

b10:
  %86 = load i16, ptr %25, !tbaa !2
  %87 = getelementptr i8, ptr %47, i16 -4
  %88 = load i16, ptr %87
  %89 = icmp ult i16 %86, %88
  %90 = sext i1 %89 to i8
  %91 = icmp ne i8 %90, 0
  br i1 %91, label %b14, label %b15

b11:
  br label %b8

b12:
  %92 = mul i16 %80, 6
  %93 = getelementptr i8, ptr %47, i16 %92
  %94 = load i16, ptr %93
  %95 = icmp ne i16 %94, 0
  %96 = sext i1 %95 to i8
  %97 = icmp ne i8 %96, 0
  br i1 %97, label %b10, label %b11

b13:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %98 = mul i16 %86, 6
  %99 = getelementptr i8, ptr %47, i16 %98
  %100 = load i16, ptr %99
  %101 = load i16, ptr %26, !tbaa !2
  %102 = icmp eq i16 %100, %101
  %103 = sext i1 %102 to i8
  store i8 %103, ptr %22, !tbaa !2
  %104 = icmp ne i8 %103, 0
  br i1 %104, label %b16, label %b17

b15:
  call addrspace(1) void @N$EBND()
  unreachable

b16:
  %105 = load i16, ptr %25, !tbaa !2
  %106 = getelementptr i8, ptr %47, i16 -4
  %107 = load i16, ptr %106
  %108 = icmp ult i16 %105, %107
  %109 = sext i1 %108 to i8
  %110 = icmp ne i8 %109, 0
  br i1 %110, label %b18, label %b19

b17:
  %111 = load i8, ptr %22, !tbaa !2
  %112 = icmp ne i8 %111, 0
  br i1 %112, label %b20, label %b21

b18:
  %113 = mul i16 %105, 6
  %114 = getelementptr i8, ptr %47, i16 %113
  %115 = getelementptr i8, ptr %114, i16 2
  %116 = load i16, ptr %115
  %117 = load i16, ptr %27, !tbaa !2
  %118 = call addrspace(1) i8 @i16.eq(i16 %116, i16 %117)
  store i8 %118, ptr %22, !tbaa !2
  br label %b17

b19:
  call addrspace(1) void @N$EBND()
  unreachable

b20:
  store i8 -1, ptr %24, !tbaa !2
  br label %b11

b21:
  br label %b22

b22:
  %119 = load i16, ptr %25, !tbaa !2
  %120 = add i16 %119, 1
  %121 = load i16, ptr %23, !tbaa !2
  %122 = and i16 %120, %121
  store i16 %122, ptr %25, !tbaa !2
  br label %b9

b23:
  %123 = load i16, ptr %25, !tbaa !2
  %124 = getelementptr i8, ptr %47, i16 -4
  %125 = load i16, ptr %124
  %126 = icmp ult i16 %123, %125
  %127 = sext i1 %126 to i8
  %128 = icmp ne i8 %127, 0
  br i1 %128, label %b26, label %b27

b24:
  br label %b25

b25:
  %129 = load i8, ptr %24, !tbaa !2
  %130 = icmp ne i8 %129, 0
  br i1 %130, label %b31, label %b30

b26:
  %131 = mul i16 %123, 6
  %132 = getelementptr i8, ptr %47, i16 %131
  %133 = load i16, ptr %26, !tbaa !2
  store i16 %133, ptr %132
  %134 = load i16, ptr %25, !tbaa !2
  %135 = getelementptr i8, ptr %47, i16 -4
  %136 = load i16, ptr %135
  %137 = icmp ult i16 %134, %136
  %138 = sext i1 %137 to i8
  %139 = icmp ne i8 %138, 0
  br i1 %139, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %140 = mul i16 %134, 6
  %141 = getelementptr i8, ptr %47, i16 %140
  %142 = load i16, ptr %27, !tbaa !2
  %143 = getelementptr i8, ptr %141, i16 2
  store i16 %142, ptr %143
  br label %b25

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  %144 = getelementptr i8, ptr %47, i16 -2
  %145 = load i16, ptr %144
  %146 = add i16 %145, 1
  %147 = getelementptr i8, ptr %47, i16 -2
  store i16 %146, ptr %147
  br label %b31

b31:
  %148 = load i16, ptr %25, !tbaa !2
  %149 = getelementptr i8, ptr %47, i16 -4
  %150 = load i16, ptr %149
  %151 = icmp ult i16 %148, %150
  %152 = sext i1 %151 to i8
  %153 = icmp ne i8 %152, 0
  br i1 %153, label %b32, label %b33

b32:
  %154 = mul i16 %148, 6
  %155 = getelementptr i8, ptr %47, i16 %154
  %156 = sub i16 %42, 0
  %157 = getelementptr inbounds i16, ptr %32, i16 %156
  %158 = load i16, ptr %157, !tbaa !2
  %159 = mul i16 %158, 10
  %160 = getelementptr i8, ptr %155, i16 4
  store i16 %159, ptr %160
  br label %b4

b33:
  call addrspace(1) void @N$EBND()
  unreachable

b34:
  %161 = getelementptr i8, ptr %62, i16 -4
  %162 = load i16, ptr %161
  %163 = sub i16 %162, 1
  store i16 %163, ptr %16, !tbaa !2
  %164 = load i16, ptr %19, !tbaa !2
  %165 = load i16, ptr %16, !tbaa !2
  %166 = and i16 %164, %165
  store i16 %166, ptr %18, !tbaa !2
  br label %b37

b35:
  br label %b36

b36:
  %167 = load i8, ptr %17, !tbaa !2
  %168 = icmp ne i8 %167, 0
  br i1 %168, label %b51, label %b52

b37:
  %169 = load i16, ptr %18, !tbaa !2
  %170 = getelementptr i8, ptr %62, i16 -4
  %171 = load i16, ptr %170
  %172 = icmp ult i16 %169, %171
  %173 = sext i1 %172 to i8
  %174 = icmp ne i8 %173, 0
  br i1 %174, label %b40, label %b41

b38:
  %175 = load i16, ptr %18, !tbaa !2
  %176 = getelementptr i8, ptr %62, i16 -4
  %177 = load i16, ptr %176
  %178 = icmp ult i16 %175, %177
  %179 = sext i1 %178 to i8
  %180 = icmp ne i8 %179, 0
  br i1 %180, label %b42, label %b43

b39:
  br label %b36

b40:
  %181 = mul i16 %169, 6
  %182 = getelementptr i8, ptr %62, i16 %181
  %183 = load i16, ptr %182
  %184 = icmp ne i16 %183, 0
  %185 = sext i1 %184 to i8
  %186 = icmp ne i8 %185, 0
  br i1 %186, label %b38, label %b39

b41:
  call addrspace(1) void @N$EBND()
  unreachable

b42:
  %187 = mul i16 %175, 6
  %188 = getelementptr i8, ptr %62, i16 %187
  %189 = load i16, ptr %188
  %190 = load i16, ptr %19, !tbaa !2
  %191 = icmp eq i16 %189, %190
  %192 = sext i1 %191 to i8
  store i8 %192, ptr %15, !tbaa !2
  %193 = icmp ne i8 %192, 0
  br i1 %193, label %b44, label %b45

b43:
  call addrspace(1) void @N$EBND()
  unreachable

b44:
  %194 = load i16, ptr %18, !tbaa !2
  %195 = getelementptr i8, ptr %62, i16 -4
  %196 = load i16, ptr %195
  %197 = icmp ult i16 %194, %196
  %198 = sext i1 %197 to i8
  %199 = icmp ne i8 %198, 0
  br i1 %199, label %b46, label %b47

b45:
  %200 = load i8, ptr %15, !tbaa !2
  %201 = icmp ne i8 %200, 0
  br i1 %201, label %b48, label %b49

b46:
  %202 = mul i16 %194, 6
  %203 = getelementptr i8, ptr %62, i16 %202
  %204 = getelementptr i8, ptr %203, i16 2
  %205 = load i16, ptr %204
  %206 = load i16, ptr %20, !tbaa !2
  %207 = call addrspace(1) i8 @i16.eq(i16 %205, i16 %206)
  store i8 %207, ptr %15, !tbaa !2
  br label %b45

b47:
  call addrspace(1) void @N$EBND()
  unreachable

b48:
  store i8 -1, ptr %17, !tbaa !2
  br label %b39

b49:
  br label %b50

b50:
  %208 = load i16, ptr %18, !tbaa !2
  %209 = add i16 %208, 1
  %210 = load i16, ptr %16, !tbaa !2
  %211 = and i16 %209, %210
  store i16 %211, ptr %18, !tbaa !2
  br label %b37

b51:
  %212 = load i16, ptr %18, !tbaa !2
  %213 = getelementptr i8, ptr %62, i16 -4
  %214 = load i16, ptr %213
  %215 = icmp ult i16 %212, %214
  %216 = sext i1 %215 to i8
  %217 = icmp ne i8 %216, 0
  br i1 %217, label %b54, label %b55

b52:
  store i16 0, ptr %14, !tbaa !2
  br label %b53

b53:
  %218 = load i16, ptr %14, !tbaa !2
  %219 = load ptr, ptr %21, !tbaa !2
  store i16 3, ptr %13, !tbaa !2
  %220 = load i16, ptr %13, !tbaa !2
  %221 = call addrspace(1) i16 @i16.hash(i16 %220)
  %222 = or i16 %221, 1
  store i16 %222, ptr %12, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  store i8 0, ptr %10, !tbaa !2
  %223 = getelementptr i8, ptr %219, i16 -4
  %224 = load i16, ptr %223
  %225 = icmp ne i16 %224, 0
  %226 = sext i1 %225 to i8
  %227 = icmp ne i8 %226, 0
  br i1 %227, label %b56, label %b57

b54:
  %228 = mul i16 %212, 6
  %229 = getelementptr i8, ptr %62, i16 %228
  %230 = getelementptr i8, ptr %229, i16 4
  %231 = load i16, ptr %230
  store i16 %231, ptr %14, !tbaa !2
  br label %b53

b55:
  call addrspace(1) void @N$EBND()
  unreachable

b56:
  %232 = getelementptr i8, ptr %219, i16 -4
  %233 = load i16, ptr %232
  %234 = sub i16 %233, 1
  store i16 %234, ptr %9, !tbaa !2
  %235 = load i16, ptr %12, !tbaa !2
  %236 = load i16, ptr %9, !tbaa !2
  %237 = and i16 %235, %236
  store i16 %237, ptr %11, !tbaa !2
  br label %b59

b57:
  br label %b58

b58:
  %238 = load i8, ptr %10, !tbaa !2
  %239 = icmp ne i8 %238, 0
  br i1 %239, label %b73, label %b74

b59:
  %240 = load i16, ptr %11, !tbaa !2
  %241 = getelementptr i8, ptr %219, i16 -4
  %242 = load i16, ptr %241
  %243 = icmp ult i16 %240, %242
  %244 = sext i1 %243 to i8
  %245 = icmp ne i8 %244, 0
  br i1 %245, label %b62, label %b63

b60:
  %246 = load i16, ptr %11, !tbaa !2
  %247 = getelementptr i8, ptr %219, i16 -4
  %248 = load i16, ptr %247
  %249 = icmp ult i16 %246, %248
  %250 = sext i1 %249 to i8
  %251 = icmp ne i8 %250, 0
  br i1 %251, label %b64, label %b65

b61:
  br label %b58

b62:
  %252 = mul i16 %240, 6
  %253 = getelementptr i8, ptr %219, i16 %252
  %254 = load i16, ptr %253
  %255 = icmp ne i16 %254, 0
  %256 = sext i1 %255 to i8
  %257 = icmp ne i8 %256, 0
  br i1 %257, label %b60, label %b61

b63:
  call addrspace(1) void @N$EBND()
  unreachable

b64:
  %258 = mul i16 %246, 6
  %259 = getelementptr i8, ptr %219, i16 %258
  %260 = load i16, ptr %259
  %261 = load i16, ptr %12, !tbaa !2
  %262 = icmp eq i16 %260, %261
  %263 = sext i1 %262 to i8
  store i8 %263, ptr %8, !tbaa !2
  %264 = icmp ne i8 %263, 0
  br i1 %264, label %b66, label %b67

b65:
  call addrspace(1) void @N$EBND()
  unreachable

b66:
  %265 = load i16, ptr %11, !tbaa !2
  %266 = getelementptr i8, ptr %219, i16 -4
  %267 = load i16, ptr %266
  %268 = icmp ult i16 %265, %267
  %269 = sext i1 %268 to i8
  %270 = icmp ne i8 %269, 0
  br i1 %270, label %b68, label %b69

b67:
  %271 = load i8, ptr %8, !tbaa !2
  %272 = icmp ne i8 %271, 0
  br i1 %272, label %b70, label %b71

b68:
  %273 = mul i16 %265, 6
  %274 = getelementptr i8, ptr %219, i16 %273
  %275 = getelementptr i8, ptr %274, i16 2
  %276 = load i16, ptr %275
  %277 = load i16, ptr %13, !tbaa !2
  %278 = call addrspace(1) i8 @i16.eq(i16 %276, i16 %277)
  store i8 %278, ptr %8, !tbaa !2
  br label %b67

b69:
  call addrspace(1) void @N$EBND()
  unreachable

b70:
  store i8 -1, ptr %10, !tbaa !2
  br label %b61

b71:
  br label %b72

b72:
  %279 = load i16, ptr %11, !tbaa !2
  %280 = add i16 %279, 1
  %281 = load i16, ptr %9, !tbaa !2
  %282 = and i16 %280, %281
  store i16 %282, ptr %11, !tbaa !2
  br label %b59

b73:
  %283 = load i16, ptr %11, !tbaa !2
  %284 = getelementptr i8, ptr %219, i16 -4
  %285 = load i16, ptr %284
  %286 = icmp ult i16 %283, %285
  %287 = sext i1 %286 to i8
  %288 = icmp ne i8 %287, 0
  br i1 %288, label %b76, label %b77

b74:
  store i16 0, ptr %7, !tbaa !2
  br label %b75

b75:
  %289 = load i16, ptr %7, !tbaa !2
  %290 = add i16 %218, %289
  %291 = load ptr, ptr %21, !tbaa !2
  store i16 9, ptr %6, !tbaa !2
  %292 = load i16, ptr %6, !tbaa !2
  %293 = call addrspace(1) i16 @i16.hash(i16 %292)
  %294 = or i16 %293, 1
  store i16 %294, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i8 0, ptr %3, !tbaa !2
  %295 = getelementptr i8, ptr %291, i16 -4
  %296 = load i16, ptr %295
  %297 = icmp ne i16 %296, 0
  %298 = sext i1 %297 to i8
  %299 = icmp ne i8 %298, 0
  br i1 %299, label %b78, label %b79

b76:
  %300 = mul i16 %283, 6
  %301 = getelementptr i8, ptr %219, i16 %300
  %302 = getelementptr i8, ptr %301, i16 4
  %303 = load i16, ptr %302
  store i16 %303, ptr %7, !tbaa !2
  br label %b75

b77:
  call addrspace(1) void @N$EBND()
  unreachable

b78:
  %304 = getelementptr i8, ptr %291, i16 -4
  %305 = load i16, ptr %304
  %306 = sub i16 %305, 1
  store i16 %306, ptr %2, !tbaa !2
  %307 = load i16, ptr %5, !tbaa !2
  %308 = load i16, ptr %2, !tbaa !2
  %309 = and i16 %307, %308
  store i16 %309, ptr %4, !tbaa !2
  br label %b81

b79:
  br label %b80

b80:
  %310 = load i8, ptr %3, !tbaa !2
  %311 = icmp ne i8 %310, 0
  br i1 %311, label %b95, label %b96

b81:
  %312 = load i16, ptr %4, !tbaa !2
  %313 = getelementptr i8, ptr %291, i16 -4
  %314 = load i16, ptr %313
  %315 = icmp ult i16 %312, %314
  %316 = sext i1 %315 to i8
  %317 = icmp ne i8 %316, 0
  br i1 %317, label %b84, label %b85

b82:
  %318 = load i16, ptr %4, !tbaa !2
  %319 = getelementptr i8, ptr %291, i16 -4
  %320 = load i16, ptr %319
  %321 = icmp ult i16 %318, %320
  %322 = sext i1 %321 to i8
  %323 = icmp ne i8 %322, 0
  br i1 %323, label %b86, label %b87

b83:
  br label %b80

b84:
  %324 = mul i16 %312, 6
  %325 = getelementptr i8, ptr %291, i16 %324
  %326 = load i16, ptr %325
  %327 = icmp ne i16 %326, 0
  %328 = sext i1 %327 to i8
  %329 = icmp ne i8 %328, 0
  br i1 %329, label %b82, label %b83

b85:
  call addrspace(1) void @N$EBND()
  unreachable

b86:
  %330 = mul i16 %318, 6
  %331 = getelementptr i8, ptr %291, i16 %330
  %332 = load i16, ptr %331
  %333 = load i16, ptr %5, !tbaa !2
  %334 = icmp eq i16 %332, %333
  %335 = sext i1 %334 to i8
  store i8 %335, ptr %1, !tbaa !2
  %336 = icmp ne i8 %335, 0
  br i1 %336, label %b88, label %b89

b87:
  call addrspace(1) void @N$EBND()
  unreachable

b88:
  %337 = load i16, ptr %4, !tbaa !2
  %338 = getelementptr i8, ptr %291, i16 -4
  %339 = load i16, ptr %338
  %340 = icmp ult i16 %337, %339
  %341 = sext i1 %340 to i8
  %342 = icmp ne i8 %341, 0
  br i1 %342, label %b90, label %b91

b89:
  %343 = load i8, ptr %1, !tbaa !2
  %344 = icmp ne i8 %343, 0
  br i1 %344, label %b92, label %b93

b90:
  %345 = mul i16 %337, 6
  %346 = getelementptr i8, ptr %291, i16 %345
  %347 = getelementptr i8, ptr %346, i16 2
  %348 = load i16, ptr %347
  %349 = load i16, ptr %6, !tbaa !2
  %350 = call addrspace(1) i8 @i16.eq(i16 %348, i16 %349)
  store i8 %350, ptr %1, !tbaa !2
  br label %b89

b91:
  call addrspace(1) void @N$EBND()
  unreachable

b92:
  store i8 -1, ptr %3, !tbaa !2
  br label %b83

b93:
  br label %b94

b94:
  %351 = load i16, ptr %4, !tbaa !2
  %352 = add i16 %351, 1
  %353 = load i16, ptr %2, !tbaa !2
  %354 = and i16 %352, %353
  store i16 %354, ptr %4, !tbaa !2
  br label %b81

b95:
  %355 = load i16, ptr %4, !tbaa !2
  %356 = getelementptr i8, ptr %291, i16 -4
  %357 = load i16, ptr %356
  %358 = icmp ult i16 %355, %357
  %359 = sext i1 %358 to i8
  %360 = icmp ne i8 %359, 0
  br i1 %360, label %b98, label %b99

b96:
  store i16 5, ptr %0, !tbaa !2
  br label %b97

b97:
  %361 = load i16, ptr %0, !tbaa !2
  %362 = add i16 %290, %361
  %363 = load ptr, ptr %21, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %363)
  %364 = load ptr, ptr %29, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %364)
  ret i16 %362

b98:
  %365 = mul i16 %355, 6
  %366 = getelementptr i8, ptr %291, i16 %365
  %367 = getelementptr i8, ptr %366, i16 4
  %368 = load i16, ptr %367
  store i16 %368, ptr %0, !tbaa !2
  br label %b97

b99:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i16 @count() addrspace(1) {
b1:
  %0 = alloca ptr
  %1 = alloca i8
  %2 = alloca i16
  %3 = alloca i8
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca ptr
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca [8 x i8]
  store ptr null, ptr %0
  store i8 0, ptr %1
  store i16 0, ptr %2
  store i8 0, ptr %3
  store i16 0, ptr %4
  store i16 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store ptr null, ptr %8
  store i16 0, ptr %9
  store i16 0, ptr %10
  call void @llvm.memset.p0.i16(ptr %11, i8 0, i16 8, i1 false)
  store i16 4, ptr %9, !tbaa !2
  store i16 4, ptr %10, !tbaa !2
  %12 = sub i16 0, 0
  %13 = getelementptr inbounds i16, ptr %11, i16 %12
  store i16 1, ptr %13, !tbaa !2
  %14 = sub i16 1, 0
  %15 = getelementptr inbounds i16, ptr %11, i16 %14
  store i16 2, ptr %15, !tbaa !2
  %16 = sub i16 2, 0
  %17 = getelementptr inbounds i16, ptr %11, i16 %16
  store i16 1, ptr %17, !tbaa !2
  %18 = sub i16 3, 0
  %19 = getelementptr inbounds i16, ptr %11, i16 %18
  store i16 3, ptr %19, !tbaa !2
  %20 = getelementptr i8, ptr @$str1, i16 6
  store ptr %20, ptr %8, !tbaa !2
  store i16 0, ptr %7, !tbaa !2
  br label %b2

b2:
  %21 = load i16, ptr %7, !tbaa !2
  %22 = icmp ult i16 %21, 4
  %23 = sext i1 %22 to i8
  %24 = icmp ne i8 %23, 0
  br i1 %24, label %b3, label %b5

b3:
  %25 = load ptr, ptr %8, !tbaa !2
  %26 = call addrspace(1) ptr @N$DRES(ptr %25, i16 6)
  store ptr %26, ptr %8, !tbaa !2
  %27 = sub i16 %21, 0
  %28 = getelementptr inbounds i16, ptr %11, i16 %27
  %29 = load i16, ptr %28, !tbaa !2
  store i16 %29, ptr %6, !tbaa !2
  %30 = load i16, ptr %6, !tbaa !2
  %31 = call addrspace(1) i16 @i16.hash(i16 %30)
  %32 = or i16 %31, 1
  store i16 %32, ptr %5, !tbaa !2
  store i16 0, ptr %4, !tbaa !2
  store i8 0, ptr %3, !tbaa !2
  %33 = getelementptr i8, ptr %26, i16 -4
  %34 = load i16, ptr %33
  %35 = icmp ne i16 %34, 0
  %36 = sext i1 %35 to i8
  %37 = icmp ne i8 %36, 0
  br i1 %37, label %b6, label %b7

b4:
  %38 = load i16, ptr %7, !tbaa !2
  %39 = add i16 %38, 1
  store i16 %39, ptr %7, !tbaa !2
  br label %b2

b5:
  %40 = load ptr, ptr %8, !tbaa !2
  store ptr null, ptr %8, !tbaa !2
  store ptr %40, ptr %0, !tbaa !2
  %41 = load ptr, ptr %0, !tbaa !2
  %42 = getelementptr i8, ptr %41, i16 -2
  %43 = load i16, ptr %42
  %44 = load ptr, ptr %0, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %44)
  %45 = load ptr, ptr %8, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %45)
  ret i16 %43

b6:
  %46 = getelementptr i8, ptr %26, i16 -4
  %47 = load i16, ptr %46
  %48 = sub i16 %47, 1
  store i16 %48, ptr %2, !tbaa !2
  %49 = load i16, ptr %5, !tbaa !2
  %50 = load i16, ptr %2, !tbaa !2
  %51 = and i16 %49, %50
  store i16 %51, ptr %4, !tbaa !2
  br label %b9

b7:
  br label %b8

b8:
  %52 = load i8, ptr %3, !tbaa !2
  %53 = xor i8 %52, -1
  %54 = icmp ne i8 %53, 0
  br i1 %54, label %b23, label %b24

b9:
  %55 = load i16, ptr %4, !tbaa !2
  %56 = getelementptr i8, ptr %26, i16 -4
  %57 = load i16, ptr %56
  %58 = icmp ult i16 %55, %57
  %59 = sext i1 %58 to i8
  %60 = icmp ne i8 %59, 0
  br i1 %60, label %b12, label %b13

b10:
  %61 = load i16, ptr %4, !tbaa !2
  %62 = getelementptr i8, ptr %26, i16 -4
  %63 = load i16, ptr %62
  %64 = icmp ult i16 %61, %63
  %65 = sext i1 %64 to i8
  %66 = icmp ne i8 %65, 0
  br i1 %66, label %b14, label %b15

b11:
  br label %b8

b12:
  %67 = mul i16 %55, 6
  %68 = getelementptr i8, ptr %26, i16 %67
  %69 = load i16, ptr %68
  %70 = icmp ne i16 %69, 0
  %71 = sext i1 %70 to i8
  %72 = icmp ne i8 %71, 0
  br i1 %72, label %b10, label %b11

b13:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %73 = mul i16 %61, 6
  %74 = getelementptr i8, ptr %26, i16 %73
  %75 = load i16, ptr %74
  %76 = load i16, ptr %5, !tbaa !2
  %77 = icmp eq i16 %75, %76
  %78 = sext i1 %77 to i8
  store i8 %78, ptr %1, !tbaa !2
  %79 = icmp ne i8 %78, 0
  br i1 %79, label %b16, label %b17

b15:
  call addrspace(1) void @N$EBND()
  unreachable

b16:
  %80 = load i16, ptr %4, !tbaa !2
  %81 = getelementptr i8, ptr %26, i16 -4
  %82 = load i16, ptr %81
  %83 = icmp ult i16 %80, %82
  %84 = sext i1 %83 to i8
  %85 = icmp ne i8 %84, 0
  br i1 %85, label %b18, label %b19

b17:
  %86 = load i8, ptr %1, !tbaa !2
  %87 = icmp ne i8 %86, 0
  br i1 %87, label %b20, label %b21

b18:
  %88 = mul i16 %80, 6
  %89 = getelementptr i8, ptr %26, i16 %88
  %90 = getelementptr i8, ptr %89, i16 2
  %91 = load i16, ptr %90
  %92 = load i16, ptr %6, !tbaa !2
  %93 = call addrspace(1) i8 @i16.eq(i16 %91, i16 %92)
  store i8 %93, ptr %1, !tbaa !2
  br label %b17

b19:
  call addrspace(1) void @N$EBND()
  unreachable

b20:
  store i8 -1, ptr %3, !tbaa !2
  br label %b11

b21:
  br label %b22

b22:
  %94 = load i16, ptr %4, !tbaa !2
  %95 = add i16 %94, 1
  %96 = load i16, ptr %2, !tbaa !2
  %97 = and i16 %95, %96
  store i16 %97, ptr %4, !tbaa !2
  br label %b9

b23:
  %98 = load i16, ptr %4, !tbaa !2
  %99 = getelementptr i8, ptr %26, i16 -4
  %100 = load i16, ptr %99
  %101 = icmp ult i16 %98, %100
  %102 = sext i1 %101 to i8
  %103 = icmp ne i8 %102, 0
  br i1 %103, label %b26, label %b27

b24:
  br label %b25

b25:
  %104 = load i8, ptr %3, !tbaa !2
  %105 = icmp ne i8 %104, 0
  br i1 %105, label %b31, label %b30

b26:
  %106 = mul i16 %98, 6
  %107 = getelementptr i8, ptr %26, i16 %106
  %108 = load i16, ptr %5, !tbaa !2
  store i16 %108, ptr %107
  %109 = load i16, ptr %4, !tbaa !2
  %110 = getelementptr i8, ptr %26, i16 -4
  %111 = load i16, ptr %110
  %112 = icmp ult i16 %109, %111
  %113 = sext i1 %112 to i8
  %114 = icmp ne i8 %113, 0
  br i1 %114, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %115 = mul i16 %109, 6
  %116 = getelementptr i8, ptr %26, i16 %115
  %117 = load i16, ptr %6, !tbaa !2
  %118 = getelementptr i8, ptr %116, i16 2
  store i16 %117, ptr %118
  br label %b25

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  %119 = getelementptr i8, ptr %26, i16 -2
  %120 = load i16, ptr %119
  %121 = add i16 %120, 1
  %122 = getelementptr i8, ptr %26, i16 -2
  store i16 %121, ptr %122
  br label %b31

b31:
  %123 = load i16, ptr %4, !tbaa !2
  %124 = getelementptr i8, ptr %26, i16 -4
  %125 = load i16, ptr %124
  %126 = icmp ult i16 %123, %125
  %127 = sext i1 %126 to i8
  %128 = icmp ne i8 %127, 0
  br i1 %128, label %b32, label %b33

b32:
  %129 = mul i16 %123, 6
  %130 = getelementptr i8, ptr %26, i16 %129
  %131 = sub i16 %21, 0
  %132 = getelementptr inbounds i16, ptr %11, i16 %131
  %133 = load i16, ptr %132, !tbaa !2
  %134 = mul i16 %133, 10
  %135 = getelementptr i8, ptr %130, i16 4
  store i16 %134, ptr %135
  br label %b4

b33:
  call addrspace(1) void @N$EBND()
  unreachable
}

define internal i8 @i16.eq(i16 %0, i16 %1) addrspace(1) {
b1:
  %2 = icmp eq i16 %0, %1
  %3 = sext i1 %2 to i8
  ret i8 %3
}

define internal i16 @i16.hash(i16 %0) addrspace(1) {
b1:
  ret i16 %0
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$DRES(ptr, i16) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
