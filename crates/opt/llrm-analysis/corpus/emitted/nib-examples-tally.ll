target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str2 = internal constant [10 x i8] c"\08\00\03\00\03\00the\00"
@$str3 = internal constant [10 x i8] c"\08\00\03\00\03\00cat\00"
@$str4 = internal constant [10 x i8] c"\08\00\03\00\03\00saw\00"
@$str5 = internal constant [10 x i8] c"\08\00\03\00\03\00dog\00"
@$str6 = internal constant [10 x i8] c"\08\00\03\00\03\00and\00"
@$str7 = internal constant [10 x i8] c"\08\00\03\00\03\00ran\00"
@$str8 = internal constant [10 x i8] c"\08\00\03\00\03\00owl\00"
@$str9 = internal constant [20 x i8] c"\08\00\0D\00\0D\00 words, the x\00"
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00, owl x\00"
@$str11 = internal constant [14 x i8] c"\08\00\07\00\07\00hearts \00"
@$str12 = internal constant [18 x i8] c"\08\00\0B\00\0B\00, distinct \00"
@$str13 = internal constant [15 x i8] c"\08\00\08\00\08\00, queen \00"
@$str14 = internal constant [17 x i8] c"\08\00\0A\00\0A\00 squares, \00"
@$str15 = internal constant [11 x i8] c"\08\00\04\00\04\00 -> \00"

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i16
  %4 = alloca i8
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca ptr
  %9 = alloca i16
  %10 = alloca i8
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca ptr
  %17 = alloca i8
  %18 = alloca i16
  %19 = alloca i8
  %20 = alloca i16
  %21 = alloca i16
  %22 = alloca [2 x i8]
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca i16
  %26 = alloca i8
  %27 = alloca i16
  %28 = alloca i16
  %29 = alloca i8
  %30 = alloca i16
  %31 = alloca i8
  %32 = alloca i16
  %33 = alloca i16
  %34 = alloca [2 x i8]
  %35 = alloca i16
  %36 = alloca i8
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca i8
  %40 = alloca i16
  %41 = alloca ptr
  %42 = alloca ptr
  %43 = alloca ptr
  %44 = alloca i16
  %45 = alloca i8
  %46 = alloca i16
  %47 = alloca i16
  %48 = alloca i8
  %49 = alloca i16
  %50 = alloca i8
  %51 = alloca i16
  %52 = alloca i16
  %53 = alloca i8
  %54 = alloca ptr
  %55 = alloca ptr
  %56 = alloca i16
  %57 = alloca i16
  %58 = alloca [8 x i8]
  %59 = alloca [8 x i8]
  %60 = alloca i16
  %61 = alloca i8
  %62 = alloca i16
  %63 = alloca i16
  %64 = alloca [8 x i8]
  %65 = alloca ptr
  %66 = alloca i16
  %67 = alloca i16
  %68 = alloca i16
  %69 = alloca [8 x i8]
  %70 = alloca [8 x i8]
  %71 = alloca i16
  %72 = alloca i8
  %73 = alloca i16
  %74 = alloca i16
  %75 = alloca [8 x i8]
  %76 = alloca ptr
  %77 = alloca i16
  %78 = alloca [8 x i8]
  %79 = alloca i16
  %80 = alloca i8
  %81 = alloca i16
  %82 = alloca i16
  %83 = alloca [8 x i8]
  %84 = alloca [8 x i8]
  %85 = alloca [8 x i8]
  %86 = alloca i16
  %87 = alloca i8
  %88 = alloca i16
  %89 = alloca i16
  %90 = alloca [8 x i8]
  %91 = alloca ptr
  %92 = alloca i16
  %93 = alloca ptr
  %94 = alloca ptr
  %95 = alloca ptr
  %96 = getelementptr i8, ptr @$str1, i16 6
  %97 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %96, i16 9, i16 2)
  %98 = getelementptr inbounds i8, ptr %97, i16 0
  %99 = getelementptr i8, ptr @$str2, i16 6
  store ptr %99, ptr %98
  %100 = getelementptr inbounds i8, ptr %97, i16 2
  %101 = getelementptr i8, ptr @$str3, i16 6
  store ptr %101, ptr %100
  %102 = getelementptr inbounds i8, ptr %97, i16 4
  %103 = getelementptr i8, ptr @$str4, i16 6
  store ptr %103, ptr %102
  %104 = getelementptr inbounds i8, ptr %97, i16 6
  %105 = getelementptr i8, ptr @$str2, i16 6
  store ptr %105, ptr %104
  %106 = getelementptr inbounds i8, ptr %97, i16 8
  %107 = getelementptr i8, ptr @$str5, i16 6
  store ptr %107, ptr %106
  %108 = getelementptr inbounds i8, ptr %97, i16 10
  %109 = getelementptr i8, ptr @$str6, i16 6
  store ptr %109, ptr %108
  %110 = getelementptr inbounds i8, ptr %97, i16 12
  %111 = getelementptr i8, ptr @$str2, i16 6
  store ptr %111, ptr %110
  %112 = getelementptr inbounds i8, ptr %97, i16 14
  %113 = getelementptr i8, ptr @$str3, i16 6
  store ptr %113, ptr %112
  %114 = getelementptr inbounds i8, ptr %97, i16 16
  %115 = getelementptr i8, ptr @$str7, i16 6
  store ptr %115, ptr %114
  store ptr %97, ptr %95, !tbaa !2
  %116 = getelementptr i8, ptr @$str1, i16 6
  store ptr %116, ptr %94, !tbaa !2
  %117 = load ptr, ptr %94, !tbaa !2
  store ptr null, ptr %94, !tbaa !2
  store ptr %117, ptr %93, !tbaa !2
  %118 = load ptr, ptr %95, !tbaa !2
  %119 = getelementptr i8, ptr %118, i16 -4
  %120 = load i16, ptr %119
  store i16 0, ptr %92, !tbaa !2
  br label %b2

b2:
  %121 = load i16, ptr %92, !tbaa !2
  %122 = icmp ult i16 %121, %120
  br i1 %122, label %b3, label %b5

b3:
  %123 = mul i16 %121, 2
  %124 = getelementptr inbounds i8, ptr %118, i16 %123
  %125 = load ptr, ptr %93, !tbaa !2
  %126 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %125, i16 6)
  store ptr %126, ptr %93, !tbaa !2
  %127 = load ptr, ptr %124
  %128 = call cc 1005 addrspace(1) ptr @N$BCLN(ptr %127, i16 1)
  store ptr %128, ptr %91, !tbaa !2
  %129 = load ptr, ptr %91, !tbaa !2
  %130 = getelementptr i8, ptr %129, i16 -4
  %131 = load i16, ptr %130
  %132 = addrspacecast ptr %129 to ptr addrspace(1)
  store i16 %131, ptr %90, !tbaa !2
  %133 = getelementptr inbounds i8, ptr %90, i16 2
  store i16 %131, ptr %133, !tbaa !2
  %134 = getelementptr inbounds i8, ptr %90, i16 4
  store ptr addrspace(1) %132, ptr %134, !tbaa !2
  %135 = addrspacecast ptr %90 to ptr addrspace(1)
  %136 = call cc 1005 addrspace(1) i16 @string.hash(ptr addrspace(1) %135)
  %137 = or i16 %136, 1
  store i16 %137, ptr %89, !tbaa !2
  store i16 0, ptr %88, !tbaa !2
  store i8 0, ptr %87, !tbaa !2
  %138 = getelementptr i8, ptr %126, i16 -4
  %139 = load i16, ptr %138
  %140 = icmp ne i16 %139, 0
  br i1 %140, label %b6, label %b7

b4:
  %141 = load i16, ptr %92, !tbaa !2
  %142 = add i16 %141, 1
  store i16 %142, ptr %92, !tbaa !2
  br label %b2

b5:
  %143 = load ptr, ptr %93, !tbaa !2
  %144 = getelementptr i8, ptr @$str2, i16 6
  store ptr %144, ptr %76, !tbaa !2
  %145 = load ptr, ptr %76, !tbaa !2
  %146 = getelementptr i8, ptr %145, i16 -4
  %147 = load i16, ptr %146
  %148 = addrspacecast ptr %145 to ptr addrspace(1)
  store i16 %147, ptr %75, !tbaa !2
  %149 = getelementptr inbounds i8, ptr %75, i16 2
  store i16 %147, ptr %149, !tbaa !2
  %150 = getelementptr inbounds i8, ptr %75, i16 4
  store ptr addrspace(1) %148, ptr %150, !tbaa !2
  %151 = addrspacecast ptr %75 to ptr addrspace(1)
  %152 = call cc 1005 addrspace(1) i16 @string.hash(ptr addrspace(1) %151)
  %153 = or i16 %152, 1
  store i16 %153, ptr %74, !tbaa !2
  store i16 0, ptr %73, !tbaa !2
  store i8 0, ptr %72, !tbaa !2
  %154 = getelementptr i8, ptr %143, i16 -4
  %155 = load i16, ptr %154
  %156 = icmp ne i16 %155, 0
  br i1 %156, label %b54, label %b55

b6:
  %157 = getelementptr i8, ptr %126, i16 -4
  %158 = load i16, ptr %157
  %159 = sub i16 %158, 1
  store i16 %159, ptr %86, !tbaa !2
  %160 = load i16, ptr %89, !tbaa !2
  %161 = load i16, ptr %86, !tbaa !2
  %162 = and i16 %160, %161
  store i16 %162, ptr %88, !tbaa !2
  br label %b9

b7:
  br label %b8

b8:
  %163 = load i8, ptr %87, !tbaa !2, !range !5
  %164 = icmp ne i8 %163, 0
  br i1 %164, label %b23, label %b22

b9:
  %165 = load i16, ptr %88, !tbaa !2
  %166 = getelementptr i8, ptr %126, i16 -4
  %167 = load i16, ptr %166
  %168 = icmp ult i16 %165, %167
  br i1 %168, label %b12, label %b13

b10:
  %169 = load i16, ptr %88, !tbaa !2
  %170 = getelementptr i8, ptr %126, i16 -4
  %171 = load i16, ptr %170
  %172 = icmp ult i16 %169, %171
  br i1 %172, label %b18, label %b19

b11:
  br label %b8

b12:
  %173 = mul i16 %165, 6
  %174 = getelementptr inbounds i8, ptr %126, i16 %173
  %175 = load i16, ptr %174
  %176 = icmp ne i16 %175, 0
  br i1 %176, label %b10, label %b11

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b14:
  store i8 1, ptr %87, !tbaa !2
  br label %b11

b15:
  br label %b16

b16:
  %177 = load i16, ptr %88, !tbaa !2
  %178 = add i16 %177, 1
  %179 = load i16, ptr %86, !tbaa !2
  %180 = and i16 %178, %179
  store i16 %180, ptr %88, !tbaa !2
  br label %b9

b17:
  %181 = load i16, ptr %88, !tbaa !2
  %182 = getelementptr i8, ptr %126, i16 -4
  %183 = load i16, ptr %182
  %184 = icmp ult i16 %181, %183
  br i1 %184, label %b20, label %b21

b18:
  %185 = mul i16 %169, 6
  %186 = getelementptr inbounds i8, ptr %126, i16 %185
  %187 = load i16, ptr %186
  %188 = load i16, ptr %89, !tbaa !2
  %189 = icmp eq i16 %187, %188
  br i1 %189, label %b17, label %b15

b19:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b20:
  %190 = mul i16 %181, 6
  %191 = getelementptr inbounds i8, ptr %126, i16 %190
  %192 = getelementptr i8, ptr %191, i16 2
  %193 = load ptr, ptr %192
  %194 = getelementptr i8, ptr %193, i16 -4
  %195 = load i16, ptr %194
  %196 = addrspacecast ptr %193 to ptr addrspace(1)
  store i16 %195, ptr %85, !tbaa !2
  %197 = getelementptr inbounds i8, ptr %85, i16 2
  store i16 %195, ptr %197, !tbaa !2
  %198 = getelementptr inbounds i8, ptr %85, i16 4
  store ptr addrspace(1) %196, ptr %198, !tbaa !2
  %199 = addrspacecast ptr %85 to ptr addrspace(1)
  %200 = load ptr, ptr %91, !tbaa !2
  %201 = getelementptr i8, ptr %200, i16 -4
  %202 = load i16, ptr %201
  %203 = addrspacecast ptr %200 to ptr addrspace(1)
  store i16 %202, ptr %84, !tbaa !2
  %204 = getelementptr inbounds i8, ptr %84, i16 2
  store i16 %202, ptr %204, !tbaa !2
  %205 = getelementptr inbounds i8, ptr %84, i16 4
  store ptr addrspace(1) %203, ptr %205, !tbaa !2
  %206 = addrspacecast ptr %84 to ptr addrspace(1)
  %207 = call cc 1005 addrspace(1) i8 @string.eq(ptr addrspace(1) %199, ptr addrspace(1) %206)
  %208 = icmp ne i8 %207, 0
  br i1 %208, label %b14, label %b15

b21:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b22:
  %209 = load i16, ptr %88, !tbaa !2
  %210 = getelementptr i8, ptr %126, i16 -4
  %211 = load i16, ptr %210
  %212 = icmp ult i16 %209, %211
  br i1 %212, label %b25, label %b26

b23:
  br label %b24

b24:
  %213 = load i8, ptr %87, !tbaa !2, !range !6
  %214 = icmp ne i8 %213, 0
  br i1 %214, label %b30, label %b29

b25:
  %215 = mul i16 %209, 6
  %216 = getelementptr inbounds i8, ptr %126, i16 %215
  %217 = load i16, ptr %89, !tbaa !2
  store i16 %217, ptr %216
  %218 = load i16, ptr %88, !tbaa !2
  %219 = getelementptr i8, ptr %126, i16 -4
  %220 = load i16, ptr %219
  %221 = icmp ult i16 %218, %220
  br i1 %221, label %b27, label %b28

b26:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b27:
  %222 = mul i16 %218, 6
  %223 = getelementptr inbounds i8, ptr %126, i16 %222
  %224 = load ptr, ptr %91, !tbaa !2
  store ptr null, ptr %91, !tbaa !2
  %225 = getelementptr i8, ptr %223, i16 2
  %226 = load ptr, ptr %225
  call cc 1005 addrspace(1) void @N$BDRP(ptr %226)
  %227 = getelementptr i8, ptr %223, i16 2
  store ptr %224, ptr %227
  br label %b24

b28:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b29:
  %228 = getelementptr i8, ptr %126, i16 -2
  %229 = load i16, ptr %228
  %230 = add i16 %229, 1
  %231 = getelementptr i8, ptr %126, i16 -2
  store i16 %230, ptr %231
  br label %b30

b30:
  %232 = load i16, ptr %88, !tbaa !2
  %233 = getelementptr i8, ptr %126, i16 -4
  %234 = load i16, ptr %233
  %235 = icmp ult i16 %232, %234
  br i1 %235, label %b31, label %b32

b31:
  %236 = mul i16 %232, 6
  %237 = getelementptr inbounds i8, ptr %126, i16 %236
  %238 = load ptr, ptr %93, !tbaa !2
  %239 = load ptr, ptr %124
  %240 = getelementptr i8, ptr %239, i16 -4
  %241 = load i16, ptr %240
  %242 = addrspacecast ptr %239 to ptr addrspace(1)
  store i16 %241, ptr %83, !tbaa !2
  %243 = getelementptr inbounds i8, ptr %83, i16 2
  store i16 %241, ptr %243, !tbaa !2
  %244 = getelementptr inbounds i8, ptr %83, i16 4
  store ptr addrspace(1) %242, ptr %244, !tbaa !2
  %245 = addrspacecast ptr %83 to ptr addrspace(1)
  %246 = call cc 1005 addrspace(1) i16 @string.hash(ptr addrspace(1) %245)
  %247 = or i16 %246, 1
  store i16 %247, ptr %82, !tbaa !2
  store i16 0, ptr %81, !tbaa !2
  store i8 0, ptr %80, !tbaa !2
  %248 = getelementptr i8, ptr %238, i16 -4
  %249 = load i16, ptr %248
  %250 = icmp ne i16 %249, 0
  br i1 %250, label %b33, label %b34

b32:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b33:
  %251 = getelementptr i8, ptr %238, i16 -4
  %252 = load i16, ptr %251
  %253 = sub i16 %252, 1
  store i16 %253, ptr %79, !tbaa !2
  %254 = load i16, ptr %82, !tbaa !2
  %255 = load i16, ptr %79, !tbaa !2
  %256 = and i16 %254, %255
  store i16 %256, ptr %81, !tbaa !2
  br label %b36

b34:
  br label %b35

b35:
  %257 = load i8, ptr %80, !tbaa !2, !range !7
  %258 = icmp ne i8 %257, 0
  br i1 %258, label %b49, label %b50

b36:
  %259 = load i16, ptr %81, !tbaa !2
  %260 = getelementptr i8, ptr %238, i16 -4
  %261 = load i16, ptr %260
  %262 = icmp ult i16 %259, %261
  br i1 %262, label %b39, label %b40

b37:
  %263 = load i16, ptr %81, !tbaa !2
  %264 = getelementptr i8, ptr %238, i16 -4
  %265 = load i16, ptr %264
  %266 = icmp ult i16 %263, %265
  br i1 %266, label %b45, label %b46

b38:
  br label %b35

b39:
  %267 = mul i16 %259, 6
  %268 = getelementptr inbounds i8, ptr %238, i16 %267
  %269 = load i16, ptr %268
  %270 = icmp ne i16 %269, 0
  br i1 %270, label %b37, label %b38

b40:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b41:
  store i8 1, ptr %80, !tbaa !2
  br label %b38

b42:
  br label %b43

b43:
  %271 = load i16, ptr %81, !tbaa !2
  %272 = add i16 %271, 1
  %273 = load i16, ptr %79, !tbaa !2
  %274 = and i16 %272, %273
  store i16 %274, ptr %81, !tbaa !2
  br label %b36

b44:
  %275 = load i16, ptr %81, !tbaa !2
  %276 = getelementptr i8, ptr %238, i16 -4
  %277 = load i16, ptr %276
  %278 = icmp ult i16 %275, %277
  br i1 %278, label %b47, label %b48

b45:
  %279 = mul i16 %263, 6
  %280 = getelementptr inbounds i8, ptr %238, i16 %279
  %281 = load i16, ptr %280
  %282 = load i16, ptr %82, !tbaa !2
  %283 = icmp eq i16 %281, %282
  br i1 %283, label %b44, label %b42

b46:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b47:
  %284 = mul i16 %275, 6
  %285 = getelementptr inbounds i8, ptr %238, i16 %284
  %286 = getelementptr i8, ptr %285, i16 2
  %287 = load ptr, ptr %286
  %288 = getelementptr i8, ptr %287, i16 -4
  %289 = load i16, ptr %288
  %290 = addrspacecast ptr %287 to ptr addrspace(1)
  store i16 %289, ptr %78, !tbaa !2
  %291 = getelementptr inbounds i8, ptr %78, i16 2
  store i16 %289, ptr %291, !tbaa !2
  %292 = getelementptr inbounds i8, ptr %78, i16 4
  store ptr addrspace(1) %290, ptr %292, !tbaa !2
  %293 = addrspacecast ptr %78 to ptr addrspace(1)
  %294 = call cc 1005 addrspace(1) i8 @string.eq(ptr addrspace(1) %293, ptr addrspace(1) %245)
  %295 = icmp ne i8 %294, 0
  br i1 %295, label %b41, label %b42

b48:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b49:
  %296 = load i16, ptr %81, !tbaa !2
  %297 = getelementptr i8, ptr %238, i16 -4
  %298 = load i16, ptr %297
  %299 = icmp ult i16 %296, %298
  br i1 %299, label %b52, label %b53

b50:
  store i16 0, ptr %77, !tbaa !2
  br label %b51

b51:
  %300 = load i16, ptr %77, !tbaa !2
  %301 = add i16 %300, 1
  %302 = getelementptr i8, ptr %237, i16 4
  store i16 %301, ptr %302
  %303 = load ptr, ptr %91, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %303)
  br label %b4

b52:
  %304 = mul i16 %296, 6
  %305 = getelementptr inbounds i8, ptr %238, i16 %304
  %306 = getelementptr i8, ptr %305, i16 4
  %307 = load i16, ptr %306
  store i16 %307, ptr %77, !tbaa !2
  br label %b51

b53:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b54:
  %308 = getelementptr i8, ptr %143, i16 -4
  %309 = load i16, ptr %308
  %310 = sub i16 %309, 1
  store i16 %310, ptr %71, !tbaa !2
  %311 = load i16, ptr %74, !tbaa !2
  %312 = load i16, ptr %71, !tbaa !2
  %313 = and i16 %311, %312
  store i16 %313, ptr %73, !tbaa !2
  br label %b57

b55:
  br label %b56

b56:
  %314 = load i8, ptr %72, !tbaa !2, !range !8
  %315 = icmp ne i8 %314, 0
  br i1 %315, label %b70, label %b71

b57:
  %316 = load i16, ptr %73, !tbaa !2
  %317 = getelementptr i8, ptr %143, i16 -4
  %318 = load i16, ptr %317
  %319 = icmp ult i16 %316, %318
  br i1 %319, label %b60, label %b61

b58:
  %320 = load i16, ptr %73, !tbaa !2
  %321 = getelementptr i8, ptr %143, i16 -4
  %322 = load i16, ptr %321
  %323 = icmp ult i16 %320, %322
  br i1 %323, label %b66, label %b67

b59:
  br label %b56

b60:
  %324 = mul i16 %316, 6
  %325 = getelementptr inbounds i8, ptr %143, i16 %324
  %326 = load i16, ptr %325
  %327 = icmp ne i16 %326, 0
  br i1 %327, label %b58, label %b59

b61:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b62:
  store i8 1, ptr %72, !tbaa !2
  br label %b59

b63:
  br label %b64

b64:
  %328 = load i16, ptr %73, !tbaa !2
  %329 = add i16 %328, 1
  %330 = load i16, ptr %71, !tbaa !2
  %331 = and i16 %329, %330
  store i16 %331, ptr %73, !tbaa !2
  br label %b57

b65:
  %332 = load i16, ptr %73, !tbaa !2
  %333 = getelementptr i8, ptr %143, i16 -4
  %334 = load i16, ptr %333
  %335 = icmp ult i16 %332, %334
  br i1 %335, label %b68, label %b69

b66:
  %336 = mul i16 %320, 6
  %337 = getelementptr inbounds i8, ptr %143, i16 %336
  %338 = load i16, ptr %337
  %339 = load i16, ptr %74, !tbaa !2
  %340 = icmp eq i16 %338, %339
  br i1 %340, label %b65, label %b63

b67:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b68:
  %341 = mul i16 %332, 6
  %342 = getelementptr inbounds i8, ptr %143, i16 %341
  %343 = getelementptr i8, ptr %342, i16 2
  %344 = load ptr, ptr %343
  %345 = getelementptr i8, ptr %344, i16 -4
  %346 = load i16, ptr %345
  %347 = addrspacecast ptr %344 to ptr addrspace(1)
  store i16 %346, ptr %70, !tbaa !2
  %348 = getelementptr inbounds i8, ptr %70, i16 2
  store i16 %346, ptr %348, !tbaa !2
  %349 = getelementptr inbounds i8, ptr %70, i16 4
  store ptr addrspace(1) %347, ptr %349, !tbaa !2
  %350 = addrspacecast ptr %70 to ptr addrspace(1)
  %351 = load ptr, ptr %76, !tbaa !2
  %352 = getelementptr i8, ptr %351, i16 -4
  %353 = load i16, ptr %352
  %354 = addrspacecast ptr %351 to ptr addrspace(1)
  store i16 %353, ptr %69, !tbaa !2
  %355 = getelementptr inbounds i8, ptr %69, i16 2
  store i16 %353, ptr %355, !tbaa !2
  %356 = getelementptr inbounds i8, ptr %69, i16 4
  store ptr addrspace(1) %354, ptr %356, !tbaa !2
  %357 = addrspacecast ptr %69 to ptr addrspace(1)
  %358 = call cc 1005 addrspace(1) i8 @string.eq(ptr addrspace(1) %350, ptr addrspace(1) %357)
  %359 = icmp ne i8 %358, 0
  br i1 %359, label %b62, label %b63

b69:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b70:
  %360 = load i16, ptr %73, !tbaa !2
  %361 = getelementptr i8, ptr %143, i16 -4
  %362 = load i16, ptr %361
  %363 = icmp ult i16 %360, %362
  br i1 %363, label %b72, label %b73

b71:
  call cc 1005 addrspace(1) void @N$EKEY()
  unreachable

b72:
  %364 = mul i16 %360, 6
  %365 = getelementptr inbounds i8, ptr %143, i16 %364
  %366 = getelementptr i8, ptr %365, i16 4
  %367 = load i16, ptr %366
  store i16 %367, ptr %68, !tbaa !2
  %368 = load ptr, ptr %93, !tbaa !2
  %369 = getelementptr i8, ptr %368, i16 -2
  %370 = load i16, ptr %369
  store i16 %370, ptr %67, !tbaa !2
  %371 = load i16, ptr %68, !tbaa !2
  store i16 %371, ptr %66, !tbaa !2
  %372 = load ptr, ptr %93, !tbaa !2
  %373 = getelementptr i8, ptr @$str8, i16 6
  store ptr %373, ptr %65, !tbaa !2
  %374 = load ptr, ptr %65, !tbaa !2
  %375 = getelementptr i8, ptr %374, i16 -4
  %376 = load i16, ptr %375
  %377 = addrspacecast ptr %374 to ptr addrspace(1)
  store i16 %376, ptr %64, !tbaa !2
  %378 = getelementptr inbounds i8, ptr %64, i16 2
  store i16 %376, ptr %378, !tbaa !2
  %379 = getelementptr inbounds i8, ptr %64, i16 4
  store ptr addrspace(1) %377, ptr %379, !tbaa !2
  %380 = addrspacecast ptr %64 to ptr addrspace(1)
  %381 = call cc 1005 addrspace(1) i16 @string.hash(ptr addrspace(1) %380)
  %382 = or i16 %381, 1
  store i16 %382, ptr %63, !tbaa !2
  store i16 0, ptr %62, !tbaa !2
  store i8 0, ptr %61, !tbaa !2
  %383 = getelementptr i8, ptr %372, i16 -4
  %384 = load i16, ptr %383
  %385 = icmp ne i16 %384, 0
  br i1 %385, label %b74, label %b75

b73:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b74:
  %386 = getelementptr i8, ptr %372, i16 -4
  %387 = load i16, ptr %386
  %388 = sub i16 %387, 1
  store i16 %388, ptr %60, !tbaa !2
  %389 = load i16, ptr %63, !tbaa !2
  %390 = load i16, ptr %60, !tbaa !2
  %391 = and i16 %389, %390
  store i16 %391, ptr %62, !tbaa !2
  br label %b77

b75:
  br label %b76

b76:
  %392 = load i8, ptr %61, !tbaa !2, !range !9
  %393 = icmp ne i8 %392, 0
  br i1 %393, label %b90, label %b91

b77:
  %394 = load i16, ptr %62, !tbaa !2
  %395 = getelementptr i8, ptr %372, i16 -4
  %396 = load i16, ptr %395
  %397 = icmp ult i16 %394, %396
  br i1 %397, label %b80, label %b81

b78:
  %398 = load i16, ptr %62, !tbaa !2
  %399 = getelementptr i8, ptr %372, i16 -4
  %400 = load i16, ptr %399
  %401 = icmp ult i16 %398, %400
  br i1 %401, label %b86, label %b87

b79:
  br label %b76

b80:
  %402 = mul i16 %394, 6
  %403 = getelementptr inbounds i8, ptr %372, i16 %402
  %404 = load i16, ptr %403
  %405 = icmp ne i16 %404, 0
  br i1 %405, label %b78, label %b79

b81:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b82:
  store i8 1, ptr %61, !tbaa !2
  br label %b79

b83:
  br label %b84

b84:
  %406 = load i16, ptr %62, !tbaa !2
  %407 = add i16 %406, 1
  %408 = load i16, ptr %60, !tbaa !2
  %409 = and i16 %407, %408
  store i16 %409, ptr %62, !tbaa !2
  br label %b77

b85:
  %410 = load i16, ptr %62, !tbaa !2
  %411 = getelementptr i8, ptr %372, i16 -4
  %412 = load i16, ptr %411
  %413 = icmp ult i16 %410, %412
  br i1 %413, label %b88, label %b89

b86:
  %414 = mul i16 %398, 6
  %415 = getelementptr inbounds i8, ptr %372, i16 %414
  %416 = load i16, ptr %415
  %417 = load i16, ptr %63, !tbaa !2
  %418 = icmp eq i16 %416, %417
  br i1 %418, label %b85, label %b83

b87:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b88:
  %419 = mul i16 %410, 6
  %420 = getelementptr inbounds i8, ptr %372, i16 %419
  %421 = getelementptr i8, ptr %420, i16 2
  %422 = load ptr, ptr %421
  %423 = getelementptr i8, ptr %422, i16 -4
  %424 = load i16, ptr %423
  %425 = addrspacecast ptr %422 to ptr addrspace(1)
  store i16 %424, ptr %59, !tbaa !2
  %426 = getelementptr inbounds i8, ptr %59, i16 2
  store i16 %424, ptr %426, !tbaa !2
  %427 = getelementptr inbounds i8, ptr %59, i16 4
  store ptr addrspace(1) %425, ptr %427, !tbaa !2
  %428 = addrspacecast ptr %59 to ptr addrspace(1)
  %429 = load ptr, ptr %65, !tbaa !2
  %430 = getelementptr i8, ptr %429, i16 -4
  %431 = load i16, ptr %430
  %432 = addrspacecast ptr %429 to ptr addrspace(1)
  store i16 %431, ptr %58, !tbaa !2
  %433 = getelementptr inbounds i8, ptr %58, i16 2
  store i16 %431, ptr %433, !tbaa !2
  %434 = getelementptr inbounds i8, ptr %58, i16 4
  store ptr addrspace(1) %432, ptr %434, !tbaa !2
  %435 = addrspacecast ptr %58 to ptr addrspace(1)
  %436 = call cc 1005 addrspace(1) i8 @string.eq(ptr addrspace(1) %428, ptr addrspace(1) %435)
  %437 = icmp ne i8 %436, 0
  br i1 %437, label %b82, label %b83

b89:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b90:
  %438 = load i16, ptr %62, !tbaa !2
  %439 = getelementptr i8, ptr %372, i16 -4
  %440 = load i16, ptr %439
  %441 = icmp ult i16 %438, %440
  br i1 %441, label %b93, label %b94

b91:
  store i16 0, ptr %57, !tbaa !2
  br label %b92

b92:
  %442 = load i16, ptr %57, !tbaa !2
  store i16 %442, ptr %56, !tbaa !2
  %443 = load i16, ptr %67, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %443)
  %444 = getelementptr i8, ptr @$str9, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %444)
  %445 = load i16, ptr %66, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %445)
  %446 = getelementptr i8, ptr @$str10, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %446)
  %447 = load i16, ptr %56, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %447)
  call cc 1005 addrspace(1) void @N$PN()
  %448 = getelementptr i8, ptr @$str1, i16 6
  %449 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %448, i16 3, i16 2)
  %450 = getelementptr inbounds i8, ptr %449, i16 0
  store i8 12, ptr %450
  %451 = getelementptr i8, ptr %450, i16 1
  store i8 0, ptr %451
  %452 = getelementptr inbounds i8, ptr %449, i16 2
  store i8 3, ptr %452
  %453 = getelementptr i8, ptr %452, i16 1
  store i8 1, ptr %453
  %454 = getelementptr inbounds i8, ptr %449, i16 4
  store i8 12, ptr %454
  %455 = getelementptr i8, ptr %454, i16 1
  store i8 0, ptr %455
  store ptr %449, ptr %55, !tbaa !2
  %456 = getelementptr i8, ptr @$str1, i16 6
  store ptr %456, ptr %54, !tbaa !2
  %457 = load ptr, ptr %54, !tbaa !2
  %458 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %457, i16 6)
  store ptr %458, ptr %54, !tbaa !2
  store i8 0, ptr %53, !tbaa !2
  %459 = load i8, ptr %53, !tbaa !2
  %460 = call cc 1005 addrspace(1) i16 @Suit.hash(i8 zeroext %459)
  %461 = or i16 %460, 1
  store i16 %461, ptr %52, !tbaa !2
  store i16 0, ptr %51, !tbaa !2
  store i8 0, ptr %50, !tbaa !2
  %462 = getelementptr i8, ptr %458, i16 -4
  %463 = load i16, ptr %462
  %464 = icmp ne i16 %463, 0
  br i1 %464, label %b95, label %b96

b93:
  %465 = mul i16 %438, 6
  %466 = getelementptr inbounds i8, ptr %372, i16 %465
  %467 = getelementptr i8, ptr %466, i16 4
  %468 = load i16, ptr %467
  store i16 %468, ptr %57, !tbaa !2
  br label %b92

b94:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b95:
  %469 = getelementptr i8, ptr %458, i16 -4
  %470 = load i16, ptr %469
  %471 = sub i16 %470, 1
  store i16 %471, ptr %49, !tbaa !2
  %472 = load i16, ptr %52, !tbaa !2
  %473 = load i16, ptr %49, !tbaa !2
  %474 = and i16 %472, %473
  store i16 %474, ptr %51, !tbaa !2
  br label %b98

b96:
  br label %b97

b97:
  %475 = load i8, ptr %50, !tbaa !2, !range !10
  %476 = icmp ne i8 %475, 0
  br i1 %476, label %b112, label %b111

b98:
  %477 = load i16, ptr %51, !tbaa !2
  %478 = getelementptr i8, ptr %458, i16 -4
  %479 = load i16, ptr %478
  %480 = icmp ult i16 %477, %479
  br i1 %480, label %b101, label %b102

b99:
  %481 = load i16, ptr %51, !tbaa !2
  %482 = getelementptr i8, ptr %458, i16 -4
  %483 = load i16, ptr %482
  %484 = icmp ult i16 %481, %483
  br i1 %484, label %b107, label %b108

b100:
  br label %b97

b101:
  %485 = mul i16 %477, 6
  %486 = getelementptr inbounds i8, ptr %458, i16 %485
  %487 = load i16, ptr %486
  %488 = icmp ne i16 %487, 0
  br i1 %488, label %b99, label %b100

b102:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b103:
  store i8 1, ptr %50, !tbaa !2
  br label %b100

b104:
  br label %b105

b105:
  %489 = load i16, ptr %51, !tbaa !2
  %490 = add i16 %489, 1
  %491 = load i16, ptr %49, !tbaa !2
  %492 = and i16 %490, %491
  store i16 %492, ptr %51, !tbaa !2
  br label %b98

b106:
  %493 = load i16, ptr %51, !tbaa !2
  %494 = getelementptr i8, ptr %458, i16 -4
  %495 = load i16, ptr %494
  %496 = icmp ult i16 %493, %495
  br i1 %496, label %b109, label %b110

b107:
  %497 = mul i16 %481, 6
  %498 = getelementptr inbounds i8, ptr %458, i16 %497
  %499 = load i16, ptr %498
  %500 = load i16, ptr %52, !tbaa !2
  %501 = icmp eq i16 %499, %500
  br i1 %501, label %b106, label %b104

b108:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b109:
  %502 = mul i16 %493, 6
  %503 = getelementptr inbounds i8, ptr %458, i16 %502
  %504 = getelementptr i8, ptr %503, i16 2
  %505 = load i8, ptr %504
  %506 = load i8, ptr %53, !tbaa !2
  %507 = call cc 1005 addrspace(1) i8 @Suit.eq(i8 zeroext %505, i8 zeroext %506)
  %508 = icmp ne i8 %507, 0
  br i1 %508, label %b103, label %b104

b110:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b111:
  %509 = load i16, ptr %51, !tbaa !2
  %510 = getelementptr i8, ptr %458, i16 -4
  %511 = load i16, ptr %510
  %512 = icmp ult i16 %509, %511
  br i1 %512, label %b114, label %b115

b112:
  br label %b113

b113:
  %513 = load i8, ptr %50, !tbaa !2, !range !11
  %514 = icmp ne i8 %513, 0
  br i1 %514, label %b119, label %b118

b114:
  %515 = mul i16 %509, 6
  %516 = getelementptr inbounds i8, ptr %458, i16 %515
  %517 = load i16, ptr %52, !tbaa !2
  store i16 %517, ptr %516
  %518 = load i16, ptr %51, !tbaa !2
  %519 = getelementptr i8, ptr %458, i16 -4
  %520 = load i16, ptr %519
  %521 = icmp ult i16 %518, %520
  br i1 %521, label %b116, label %b117

b115:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b116:
  %522 = mul i16 %518, 6
  %523 = getelementptr inbounds i8, ptr %458, i16 %522
  %524 = load i8, ptr %53, !tbaa !2
  %525 = getelementptr i8, ptr %523, i16 2
  store i8 %524, ptr %525
  br label %b113

b117:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b118:
  %526 = getelementptr i8, ptr %458, i16 -2
  %527 = load i16, ptr %526
  %528 = add i16 %527, 1
  %529 = getelementptr i8, ptr %458, i16 -2
  store i16 %528, ptr %529
  br label %b119

b119:
  %530 = load i16, ptr %51, !tbaa !2
  %531 = getelementptr i8, ptr %458, i16 -4
  %532 = load i16, ptr %531
  %533 = icmp ult i16 %530, %532
  br i1 %533, label %b120, label %b121

b120:
  %534 = mul i16 %530, 6
  %535 = getelementptr inbounds i8, ptr %458, i16 %534
  %536 = getelementptr i8, ptr %535, i16 4
  store i16 0, ptr %536
  %537 = load ptr, ptr %54, !tbaa !2
  %538 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %537, i16 6)
  store ptr %538, ptr %54, !tbaa !2
  store i8 1, ptr %48, !tbaa !2
  %539 = load i8, ptr %48, !tbaa !2
  %540 = call cc 1005 addrspace(1) i16 @Suit.hash(i8 zeroext %539)
  %541 = or i16 %540, 1
  store i16 %541, ptr %47, !tbaa !2
  store i16 0, ptr %46, !tbaa !2
  store i8 0, ptr %45, !tbaa !2
  %542 = getelementptr i8, ptr %538, i16 -4
  %543 = load i16, ptr %542
  %544 = icmp ne i16 %543, 0
  br i1 %544, label %b122, label %b123

b121:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b122:
  %545 = getelementptr i8, ptr %538, i16 -4
  %546 = load i16, ptr %545
  %547 = sub i16 %546, 1
  store i16 %547, ptr %44, !tbaa !2
  %548 = load i16, ptr %47, !tbaa !2
  %549 = load i16, ptr %44, !tbaa !2
  %550 = and i16 %548, %549
  store i16 %550, ptr %46, !tbaa !2
  br label %b125

b123:
  br label %b124

b124:
  %551 = load i8, ptr %45, !tbaa !2, !range !12
  %552 = icmp ne i8 %551, 0
  br i1 %552, label %b139, label %b138

b125:
  %553 = load i16, ptr %46, !tbaa !2
  %554 = getelementptr i8, ptr %538, i16 -4
  %555 = load i16, ptr %554
  %556 = icmp ult i16 %553, %555
  br i1 %556, label %b128, label %b129

b126:
  %557 = load i16, ptr %46, !tbaa !2
  %558 = getelementptr i8, ptr %538, i16 -4
  %559 = load i16, ptr %558
  %560 = icmp ult i16 %557, %559
  br i1 %560, label %b134, label %b135

b127:
  br label %b124

b128:
  %561 = mul i16 %553, 6
  %562 = getelementptr inbounds i8, ptr %538, i16 %561
  %563 = load i16, ptr %562
  %564 = icmp ne i16 %563, 0
  br i1 %564, label %b126, label %b127

b129:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b130:
  store i8 1, ptr %45, !tbaa !2
  br label %b127

b131:
  br label %b132

b132:
  %565 = load i16, ptr %46, !tbaa !2
  %566 = add i16 %565, 1
  %567 = load i16, ptr %44, !tbaa !2
  %568 = and i16 %566, %567
  store i16 %568, ptr %46, !tbaa !2
  br label %b125

b133:
  %569 = load i16, ptr %46, !tbaa !2
  %570 = getelementptr i8, ptr %538, i16 -4
  %571 = load i16, ptr %570
  %572 = icmp ult i16 %569, %571
  br i1 %572, label %b136, label %b137

b134:
  %573 = mul i16 %557, 6
  %574 = getelementptr inbounds i8, ptr %538, i16 %573
  %575 = load i16, ptr %574
  %576 = load i16, ptr %47, !tbaa !2
  %577 = icmp eq i16 %575, %576
  br i1 %577, label %b133, label %b131

b135:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b136:
  %578 = mul i16 %569, 6
  %579 = getelementptr inbounds i8, ptr %538, i16 %578
  %580 = getelementptr i8, ptr %579, i16 2
  %581 = load i8, ptr %580
  %582 = load i8, ptr %48, !tbaa !2
  %583 = call cc 1005 addrspace(1) i8 @Suit.eq(i8 zeroext %581, i8 zeroext %582)
  %584 = icmp ne i8 %583, 0
  br i1 %584, label %b130, label %b131

b137:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b138:
  %585 = load i16, ptr %46, !tbaa !2
  %586 = getelementptr i8, ptr %538, i16 -4
  %587 = load i16, ptr %586
  %588 = icmp ult i16 %585, %587
  br i1 %588, label %b141, label %b142

b139:
  br label %b140

b140:
  %589 = load i8, ptr %45, !tbaa !2, !range !13
  %590 = icmp ne i8 %589, 0
  br i1 %590, label %b146, label %b145

b141:
  %591 = mul i16 %585, 6
  %592 = getelementptr inbounds i8, ptr %538, i16 %591
  %593 = load i16, ptr %47, !tbaa !2
  store i16 %593, ptr %592
  %594 = load i16, ptr %46, !tbaa !2
  %595 = getelementptr i8, ptr %538, i16 -4
  %596 = load i16, ptr %595
  %597 = icmp ult i16 %594, %596
  br i1 %597, label %b143, label %b144

b142:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b143:
  %598 = mul i16 %594, 6
  %599 = getelementptr inbounds i8, ptr %538, i16 %598
  %600 = load i8, ptr %48, !tbaa !2
  %601 = getelementptr i8, ptr %599, i16 2
  store i8 %600, ptr %601
  br label %b140

b144:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b145:
  %602 = getelementptr i8, ptr %538, i16 -2
  %603 = load i16, ptr %602
  %604 = add i16 %603, 1
  %605 = getelementptr i8, ptr %538, i16 -2
  store i16 %604, ptr %605
  br label %b146

b146:
  %606 = load i16, ptr %46, !tbaa !2
  %607 = getelementptr i8, ptr %538, i16 -4
  %608 = load i16, ptr %607
  %609 = icmp ult i16 %606, %608
  br i1 %609, label %b147, label %b148

b147:
  %610 = mul i16 %606, 6
  %611 = getelementptr inbounds i8, ptr %538, i16 %610
  %612 = getelementptr i8, ptr %611, i16 4
  store i16 0, ptr %612
  %613 = load ptr, ptr %54, !tbaa !2
  store ptr null, ptr %54, !tbaa !2
  store ptr %613, ptr %43, !tbaa !2
  %614 = getelementptr i8, ptr @$str1, i16 6
  store ptr %614, ptr %42, !tbaa !2
  %615 = load ptr, ptr %42, !tbaa !2
  store ptr null, ptr %42, !tbaa !2
  store ptr %615, ptr %41, !tbaa !2
  %616 = load ptr, ptr %55, !tbaa !2
  %617 = getelementptr i8, ptr %616, i16 -4
  %618 = load i16, ptr %617
  store i16 0, ptr %40, !tbaa !2
  br label %b149

b148:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b149:
  %619 = load i16, ptr %40, !tbaa !2
  %620 = icmp ult i16 %619, %618
  br i1 %620, label %b150, label %b152

b150:
  %621 = mul i16 %619, 2
  %622 = getelementptr inbounds i8, ptr %616, i16 %621
  %623 = load ptr, ptr %43, !tbaa !2
  %624 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %623, i16 6)
  store ptr %624, ptr %43, !tbaa !2
  %625 = getelementptr i8, ptr %622, i16 1
  %626 = load i8, ptr %625
  store i8 %626, ptr %39, !tbaa !2
  %627 = load i8, ptr %39, !tbaa !2
  %628 = call cc 1005 addrspace(1) i16 @Suit.hash(i8 zeroext %627)
  %629 = or i16 %628, 1
  store i16 %629, ptr %38, !tbaa !2
  store i16 0, ptr %37, !tbaa !2
  store i8 0, ptr %36, !tbaa !2
  %630 = getelementptr i8, ptr %624, i16 -4
  %631 = load i16, ptr %630
  %632 = icmp ne i16 %631, 0
  br i1 %632, label %b153, label %b154

b151:
  %633 = load i16, ptr %40, !tbaa !2
  %634 = add i16 %633, 1
  store i16 %634, ptr %40, !tbaa !2
  br label %b149

b152:
  %635 = load ptr, ptr %43, !tbaa !2
  store i8 0, ptr %29, !tbaa !2
  %636 = load i8, ptr %29, !tbaa !2
  %637 = call cc 1005 addrspace(1) i16 @Suit.hash(i8 zeroext %636)
  %638 = or i16 %637, 1
  store i16 %638, ptr %28, !tbaa !2
  store i16 0, ptr %27, !tbaa !2
  store i8 0, ptr %26, !tbaa !2
  %639 = getelementptr i8, ptr %635, i16 -4
  %640 = load i16, ptr %639
  %641 = icmp ne i16 %640, 0
  br i1 %641, label %b207, label %b208

b153:
  %642 = getelementptr i8, ptr %624, i16 -4
  %643 = load i16, ptr %642
  %644 = sub i16 %643, 1
  store i16 %644, ptr %35, !tbaa !2
  %645 = load i16, ptr %38, !tbaa !2
  %646 = load i16, ptr %35, !tbaa !2
  %647 = and i16 %645, %646
  store i16 %647, ptr %37, !tbaa !2
  br label %b156

b154:
  br label %b155

b155:
  %648 = load i8, ptr %36, !tbaa !2, !range !14
  %649 = icmp ne i8 %648, 0
  br i1 %649, label %b170, label %b169

b156:
  %650 = load i16, ptr %37, !tbaa !2
  %651 = getelementptr i8, ptr %624, i16 -4
  %652 = load i16, ptr %651
  %653 = icmp ult i16 %650, %652
  br i1 %653, label %b159, label %b160

b157:
  %654 = load i16, ptr %37, !tbaa !2
  %655 = getelementptr i8, ptr %624, i16 -4
  %656 = load i16, ptr %655
  %657 = icmp ult i16 %654, %656
  br i1 %657, label %b165, label %b166

b158:
  br label %b155

b159:
  %658 = mul i16 %650, 6
  %659 = getelementptr inbounds i8, ptr %624, i16 %658
  %660 = load i16, ptr %659
  %661 = icmp ne i16 %660, 0
  br i1 %661, label %b157, label %b158

b160:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b161:
  store i8 1, ptr %36, !tbaa !2
  br label %b158

b162:
  br label %b163

b163:
  %662 = load i16, ptr %37, !tbaa !2
  %663 = add i16 %662, 1
  %664 = load i16, ptr %35, !tbaa !2
  %665 = and i16 %663, %664
  store i16 %665, ptr %37, !tbaa !2
  br label %b156

b164:
  %666 = load i16, ptr %37, !tbaa !2
  %667 = getelementptr i8, ptr %624, i16 -4
  %668 = load i16, ptr %667
  %669 = icmp ult i16 %666, %668
  br i1 %669, label %b167, label %b168

b165:
  %670 = mul i16 %654, 6
  %671 = getelementptr inbounds i8, ptr %624, i16 %670
  %672 = load i16, ptr %671
  %673 = load i16, ptr %38, !tbaa !2
  %674 = icmp eq i16 %672, %673
  br i1 %674, label %b164, label %b162

b166:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b167:
  %675 = mul i16 %666, 6
  %676 = getelementptr inbounds i8, ptr %624, i16 %675
  %677 = getelementptr i8, ptr %676, i16 2
  %678 = load i8, ptr %677
  %679 = load i8, ptr %39, !tbaa !2
  %680 = call cc 1005 addrspace(1) i8 @Suit.eq(i8 zeroext %678, i8 zeroext %679)
  %681 = icmp ne i8 %680, 0
  br i1 %681, label %b161, label %b162

b168:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b169:
  %682 = load i16, ptr %37, !tbaa !2
  %683 = getelementptr i8, ptr %624, i16 -4
  %684 = load i16, ptr %683
  %685 = icmp ult i16 %682, %684
  br i1 %685, label %b172, label %b173

b170:
  br label %b171

b171:
  %686 = load i8, ptr %36, !tbaa !2, !range !15
  %687 = icmp ne i8 %686, 0
  br i1 %687, label %b177, label %b176

b172:
  %688 = mul i16 %682, 6
  %689 = getelementptr inbounds i8, ptr %624, i16 %688
  %690 = load i16, ptr %38, !tbaa !2
  store i16 %690, ptr %689
  %691 = load i16, ptr %37, !tbaa !2
  %692 = getelementptr i8, ptr %624, i16 -4
  %693 = load i16, ptr %692
  %694 = icmp ult i16 %691, %693
  br i1 %694, label %b174, label %b175

b173:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b174:
  %695 = mul i16 %691, 6
  %696 = getelementptr inbounds i8, ptr %624, i16 %695
  %697 = load i8, ptr %39, !tbaa !2
  %698 = getelementptr i8, ptr %696, i16 2
  store i8 %697, ptr %698
  br label %b171

b175:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b176:
  %699 = getelementptr i8, ptr %624, i16 -2
  %700 = load i16, ptr %699
  %701 = add i16 %700, 1
  %702 = getelementptr i8, ptr %624, i16 -2
  store i16 %701, ptr %702
  br label %b177

b177:
  %703 = load i16, ptr %37, !tbaa !2
  %704 = getelementptr i8, ptr %624, i16 -4
  %705 = load i16, ptr %704
  %706 = icmp ult i16 %703, %705
  br i1 %706, label %b178, label %b179

b178:
  %707 = mul i16 %703, 6
  %708 = getelementptr inbounds i8, ptr %624, i16 %707
  %709 = getelementptr i8, ptr %708, i16 4
  %710 = load i16, ptr %709
  %711 = add i16 %710, 1
  %712 = getelementptr i8, ptr %708, i16 4
  store i16 %711, ptr %712
  %713 = load ptr, ptr %41, !tbaa !2
  %714 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %713, i16 6)
  store ptr %714, ptr %41, !tbaa !2
  %715 = load i8, ptr %622
  %716 = getelementptr i8, ptr %622, i16 1
  %717 = load i8, ptr %716
  store i8 %715, ptr %34, !tbaa !2
  %718 = getelementptr inbounds i8, ptr %34, i16 1
  store i8 %717, ptr %718, !tbaa !2
  %719 = addrspacecast ptr %34 to ptr addrspace(1)
  %720 = call cc 1005 addrspace(1) i16 @Card.hash(ptr addrspace(1) %719)
  %721 = or i16 %720, 1
  store i16 %721, ptr %33, !tbaa !2
  store i16 0, ptr %32, !tbaa !2
  store i8 0, ptr %31, !tbaa !2
  %722 = getelementptr i8, ptr %714, i16 -4
  %723 = load i16, ptr %722
  %724 = icmp ne i16 %723, 0
  br i1 %724, label %b180, label %b181

b179:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b180:
  %725 = getelementptr i8, ptr %714, i16 -4
  %726 = load i16, ptr %725
  %727 = sub i16 %726, 1
  store i16 %727, ptr %30, !tbaa !2
  %728 = load i16, ptr %33, !tbaa !2
  %729 = load i16, ptr %30, !tbaa !2
  %730 = and i16 %728, %729
  store i16 %730, ptr %32, !tbaa !2
  br label %b183

b181:
  br label %b182

b182:
  %731 = load i8, ptr %31, !tbaa !2, !range !16
  %732 = icmp ne i8 %731, 0
  br i1 %732, label %b197, label %b196

b183:
  %733 = load i16, ptr %32, !tbaa !2
  %734 = getelementptr i8, ptr %714, i16 -4
  %735 = load i16, ptr %734
  %736 = icmp ult i16 %733, %735
  br i1 %736, label %b186, label %b187

b184:
  %737 = load i16, ptr %32, !tbaa !2
  %738 = getelementptr i8, ptr %714, i16 -4
  %739 = load i16, ptr %738
  %740 = icmp ult i16 %737, %739
  br i1 %740, label %b192, label %b193

b185:
  br label %b182

b186:
  %741 = mul i16 %733, 6
  %742 = getelementptr inbounds i8, ptr %714, i16 %741
  %743 = load i16, ptr %742
  %744 = icmp ne i16 %743, 0
  br i1 %744, label %b184, label %b185

b187:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b188:
  store i8 1, ptr %31, !tbaa !2
  br label %b185

b189:
  br label %b190

b190:
  %745 = load i16, ptr %32, !tbaa !2
  %746 = add i16 %745, 1
  %747 = load i16, ptr %30, !tbaa !2
  %748 = and i16 %746, %747
  store i16 %748, ptr %32, !tbaa !2
  br label %b183

b191:
  %749 = load i16, ptr %32, !tbaa !2
  %750 = getelementptr i8, ptr %714, i16 -4
  %751 = load i16, ptr %750
  %752 = icmp ult i16 %749, %751
  br i1 %752, label %b194, label %b195

b192:
  %753 = mul i16 %737, 6
  %754 = getelementptr inbounds i8, ptr %714, i16 %753
  %755 = load i16, ptr %754
  %756 = load i16, ptr %33, !tbaa !2
  %757 = icmp eq i16 %755, %756
  br i1 %757, label %b191, label %b189

b193:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b194:
  %758 = mul i16 %749, 6
  %759 = getelementptr inbounds i8, ptr %714, i16 %758
  %760 = getelementptr i8, ptr %759, i16 2
  %761 = addrspacecast ptr %760 to ptr addrspace(1)
  %762 = addrspacecast ptr %34 to ptr addrspace(1)
  %763 = call cc 1005 addrspace(1) i8 @Card.eq(ptr addrspace(1) %761, ptr addrspace(1) %762)
  %764 = icmp ne i8 %763, 0
  br i1 %764, label %b188, label %b189

b195:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b196:
  %765 = load i16, ptr %32, !tbaa !2
  %766 = getelementptr i8, ptr %714, i16 -4
  %767 = load i16, ptr %766
  %768 = icmp ult i16 %765, %767
  br i1 %768, label %b199, label %b200

b197:
  br label %b198

b198:
  %769 = load i8, ptr %31, !tbaa !2, !range !17
  %770 = icmp ne i8 %769, 0
  br i1 %770, label %b204, label %b203

b199:
  %771 = mul i16 %765, 6
  %772 = getelementptr inbounds i8, ptr %714, i16 %771
  %773 = load i16, ptr %33, !tbaa !2
  store i16 %773, ptr %772
  %774 = load i16, ptr %32, !tbaa !2
  %775 = getelementptr i8, ptr %714, i16 -4
  %776 = load i16, ptr %775
  %777 = icmp ult i16 %774, %776
  br i1 %777, label %b201, label %b202

b200:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b201:
  %778 = mul i16 %774, 6
  %779 = getelementptr inbounds i8, ptr %714, i16 %778
  %780 = load i8, ptr %34, !tbaa !2
  %781 = getelementptr inbounds i8, ptr %34, i16 1
  %782 = load i8, ptr %781, !tbaa !2
  %783 = getelementptr i8, ptr %779, i16 2
  store i8 %780, ptr %783
  %784 = getelementptr i8, ptr %779, i16 3
  store i8 %782, ptr %784
  br label %b198

b202:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b203:
  %785 = getelementptr i8, ptr %714, i16 -2
  %786 = load i16, ptr %785
  %787 = add i16 %786, 1
  %788 = getelementptr i8, ptr %714, i16 -2
  store i16 %787, ptr %788
  br label %b204

b204:
  %789 = load i16, ptr %32, !tbaa !2
  %790 = getelementptr i8, ptr %714, i16 -4
  %791 = load i16, ptr %790
  %792 = icmp ult i16 %789, %791
  br i1 %792, label %b205, label %b206

b205:
  %793 = mul i16 %789, 6
  %794 = getelementptr inbounds i8, ptr %714, i16 %793
  %795 = getelementptr i8, ptr %794, i16 4
  store i8 1, ptr %795
  br label %b151

b206:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b207:
  %796 = getelementptr i8, ptr %635, i16 -4
  %797 = load i16, ptr %796
  %798 = sub i16 %797, 1
  store i16 %798, ptr %25, !tbaa !2
  %799 = load i16, ptr %28, !tbaa !2
  %800 = load i16, ptr %25, !tbaa !2
  %801 = and i16 %799, %800
  store i16 %801, ptr %27, !tbaa !2
  br label %b210

b208:
  br label %b209

b209:
  %802 = load i8, ptr %26, !tbaa !2, !range !18
  %803 = icmp ne i8 %802, 0
  br i1 %803, label %b223, label %b224

b210:
  %804 = load i16, ptr %27, !tbaa !2
  %805 = getelementptr i8, ptr %635, i16 -4
  %806 = load i16, ptr %805
  %807 = icmp ult i16 %804, %806
  br i1 %807, label %b213, label %b214

b211:
  %808 = load i16, ptr %27, !tbaa !2
  %809 = getelementptr i8, ptr %635, i16 -4
  %810 = load i16, ptr %809
  %811 = icmp ult i16 %808, %810
  br i1 %811, label %b219, label %b220

b212:
  br label %b209

b213:
  %812 = mul i16 %804, 6
  %813 = getelementptr inbounds i8, ptr %635, i16 %812
  %814 = load i16, ptr %813
  %815 = icmp ne i16 %814, 0
  br i1 %815, label %b211, label %b212

b214:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b215:
  store i8 1, ptr %26, !tbaa !2
  br label %b212

b216:
  br label %b217

b217:
  %816 = load i16, ptr %27, !tbaa !2
  %817 = add i16 %816, 1
  %818 = load i16, ptr %25, !tbaa !2
  %819 = and i16 %817, %818
  store i16 %819, ptr %27, !tbaa !2
  br label %b210

b218:
  %820 = load i16, ptr %27, !tbaa !2
  %821 = getelementptr i8, ptr %635, i16 -4
  %822 = load i16, ptr %821
  %823 = icmp ult i16 %820, %822
  br i1 %823, label %b221, label %b222

b219:
  %824 = mul i16 %808, 6
  %825 = getelementptr inbounds i8, ptr %635, i16 %824
  %826 = load i16, ptr %825
  %827 = load i16, ptr %28, !tbaa !2
  %828 = icmp eq i16 %826, %827
  br i1 %828, label %b218, label %b216

b220:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b221:
  %829 = mul i16 %820, 6
  %830 = getelementptr inbounds i8, ptr %635, i16 %829
  %831 = getelementptr i8, ptr %830, i16 2
  %832 = load i8, ptr %831
  %833 = load i8, ptr %29, !tbaa !2
  %834 = call cc 1005 addrspace(1) i8 @Suit.eq(i8 zeroext %832, i8 zeroext %833)
  %835 = icmp ne i8 %834, 0
  br i1 %835, label %b215, label %b216

b222:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b223:
  %836 = load i16, ptr %27, !tbaa !2
  %837 = getelementptr i8, ptr %635, i16 -4
  %838 = load i16, ptr %837
  %839 = icmp ult i16 %836, %838
  br i1 %839, label %b225, label %b226

b224:
  call cc 1005 addrspace(1) void @N$EKEY()
  unreachable

b225:
  %840 = mul i16 %836, 6
  %841 = getelementptr inbounds i8, ptr %635, i16 %840
  %842 = getelementptr i8, ptr %841, i16 4
  %843 = load i16, ptr %842
  store i16 %843, ptr %24, !tbaa !2
  %844 = load ptr, ptr %41, !tbaa !2
  %845 = getelementptr i8, ptr %844, i16 -2
  %846 = load i16, ptr %845
  store i16 %846, ptr %23, !tbaa !2
  %847 = load ptr, ptr %41, !tbaa !2
  store i8 12, ptr %22, !tbaa !2
  %848 = getelementptr inbounds i8, ptr %22, i16 1
  store i8 0, ptr %848, !tbaa !2
  %849 = addrspacecast ptr %22 to ptr addrspace(1)
  %850 = call cc 1005 addrspace(1) i16 @Card.hash(ptr addrspace(1) %849)
  %851 = or i16 %850, 1
  store i16 %851, ptr %21, !tbaa !2
  store i16 0, ptr %20, !tbaa !2
  store i8 0, ptr %19, !tbaa !2
  %852 = getelementptr i8, ptr %847, i16 -4
  %853 = load i16, ptr %852
  %854 = icmp ne i16 %853, 0
  br i1 %854, label %b227, label %b228

b226:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b227:
  %855 = getelementptr i8, ptr %847, i16 -4
  %856 = load i16, ptr %855
  %857 = sub i16 %856, 1
  store i16 %857, ptr %18, !tbaa !2
  %858 = load i16, ptr %21, !tbaa !2
  %859 = load i16, ptr %18, !tbaa !2
  %860 = and i16 %858, %859
  store i16 %860, ptr %20, !tbaa !2
  br label %b230

b228:
  br label %b229

b229:
  %861 = load i8, ptr %19, !tbaa !2, !range !19
  store i8 %861, ptr %17, !tbaa !2
  %862 = getelementptr i8, ptr @$str11, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %862)
  %863 = load i16, ptr %24, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %863)
  %864 = getelementptr i8, ptr @$str12, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %864)
  %865 = load i16, ptr %23, !tbaa !2
  call cc 1005 addrspace(1) void @N$PU2(i16 %865)
  %866 = getelementptr i8, ptr @$str13, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %866)
  %867 = load i8, ptr %17, !tbaa !2, !range !20
  call cc 1005 addrspace(1) void @N$PB(i8 %867)
  call cc 1005 addrspace(1) void @N$PN()
  %868 = getelementptr i8, ptr @$str1, i16 6
  store ptr %868, ptr %16, !tbaa !2
  store i16 0, ptr %15, !tbaa !2
  store i16 40, ptr %14, !tbaa !2
  br label %b243

b230:
  %869 = load i16, ptr %20, !tbaa !2
  %870 = getelementptr i8, ptr %847, i16 -4
  %871 = load i16, ptr %870
  %872 = icmp ult i16 %869, %871
  br i1 %872, label %b233, label %b234

b231:
  %873 = load i16, ptr %20, !tbaa !2
  %874 = getelementptr i8, ptr %847, i16 -4
  %875 = load i16, ptr %874
  %876 = icmp ult i16 %873, %875
  br i1 %876, label %b239, label %b240

b232:
  br label %b229

b233:
  %877 = mul i16 %869, 6
  %878 = getelementptr inbounds i8, ptr %847, i16 %877
  %879 = load i16, ptr %878
  %880 = icmp ne i16 %879, 0
  br i1 %880, label %b231, label %b232

b234:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b235:
  store i8 1, ptr %19, !tbaa !2
  br label %b232

b236:
  br label %b237

b237:
  %881 = load i16, ptr %20, !tbaa !2
  %882 = add i16 %881, 1
  %883 = load i16, ptr %18, !tbaa !2
  %884 = and i16 %882, %883
  store i16 %884, ptr %20, !tbaa !2
  br label %b230

b238:
  %885 = load i16, ptr %20, !tbaa !2
  %886 = getelementptr i8, ptr %847, i16 -4
  %887 = load i16, ptr %886
  %888 = icmp ult i16 %885, %887
  br i1 %888, label %b241, label %b242

b239:
  %889 = mul i16 %873, 6
  %890 = getelementptr inbounds i8, ptr %847, i16 %889
  %891 = load i16, ptr %890
  %892 = load i16, ptr %21, !tbaa !2
  %893 = icmp eq i16 %891, %892
  br i1 %893, label %b238, label %b236

b240:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b241:
  %894 = mul i16 %885, 6
  %895 = getelementptr inbounds i8, ptr %847, i16 %894
  %896 = getelementptr i8, ptr %895, i16 2
  %897 = addrspacecast ptr %896 to ptr addrspace(1)
  %898 = addrspacecast ptr %22 to ptr addrspace(1)
  %899 = call cc 1005 addrspace(1) i8 @Card.eq(ptr addrspace(1) %897, ptr addrspace(1) %898)
  %900 = icmp ne i8 %899, 0
  br i1 %900, label %b235, label %b236

b242:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b243:
  %901 = load i16, ptr %15, !tbaa !2
  %902 = load i16, ptr %14, !tbaa !2
  %903 = icmp slt i16 %901, %902
  br i1 %903, label %b244, label %b246

b244:
  %904 = load ptr, ptr %16, !tbaa !2
  %905 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %904, i16 6)
  store ptr %905, ptr %16, !tbaa !2
  %906 = load i16, ptr %15, !tbaa !2
  store i16 %906, ptr %13, !tbaa !2
  %907 = load i16, ptr %13, !tbaa !2
  %908 = call cc 1005 addrspace(1) i16 @i16.hash(i16 %907)
  %909 = or i16 %908, 1
  store i16 %909, ptr %12, !tbaa !2
  store i16 0, ptr %11, !tbaa !2
  store i8 0, ptr %10, !tbaa !2
  %910 = getelementptr i8, ptr %905, i16 -4
  %911 = load i16, ptr %910
  %912 = icmp ne i16 %911, 0
  br i1 %912, label %b247, label %b248

b245:
  %913 = load i16, ptr %15, !tbaa !2
  %914 = add nsw i16 %913, 1
  store i16 %914, ptr %15, !tbaa !2
  br label %b243

b246:
  %915 = load ptr, ptr %16, !tbaa !2
  store ptr null, ptr %16, !tbaa !2
  store ptr %915, ptr %8, !tbaa !2
  store i16 37, ptr %7, !tbaa !2
  %916 = load ptr, ptr %8, !tbaa !2
  %917 = getelementptr i8, ptr %916, i16 -2
  %918 = load i16, ptr %917
  call cc 1005 addrspace(1) void @N$PU2(i16 %918)
  %919 = getelementptr i8, ptr @$str14, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %919)
  %920 = load i16, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$PI2(i16 %920)
  %921 = getelementptr i8, ptr @$str15, i16 6
  call cc 1005 addrspace(1) void @N$PS(ptr %921)
  %922 = load ptr, ptr %8, !tbaa !2
  %923 = addrspacecast ptr %7 to ptr addrspace(1)
  %924 = load i16, ptr addrspace(1) %923, !tbaa !2
  %925 = call cc 1005 addrspace(1) i16 @i16.hash(i16 %924)
  %926 = or i16 %925, 1
  store i16 %926, ptr %6, !tbaa !2
  store i16 0, ptr %5, !tbaa !2
  store i8 0, ptr %4, !tbaa !2
  %927 = getelementptr i8, ptr %922, i16 -4
  %928 = load i16, ptr %927
  %929 = icmp ne i16 %928, 0
  br i1 %929, label %b274, label %b275

b247:
  %930 = getelementptr i8, ptr %905, i16 -4
  %931 = load i16, ptr %930
  %932 = sub i16 %931, 1
  store i16 %932, ptr %9, !tbaa !2
  %933 = load i16, ptr %12, !tbaa !2
  %934 = load i16, ptr %9, !tbaa !2
  %935 = and i16 %933, %934
  store i16 %935, ptr %11, !tbaa !2
  br label %b250

b248:
  br label %b249

b249:
  %936 = load i8, ptr %10, !tbaa !2, !range !21
  %937 = icmp ne i8 %936, 0
  br i1 %937, label %b264, label %b263

b250:
  %938 = load i16, ptr %11, !tbaa !2
  %939 = getelementptr i8, ptr %905, i16 -4
  %940 = load i16, ptr %939
  %941 = icmp ult i16 %938, %940
  br i1 %941, label %b253, label %b254

b251:
  %942 = load i16, ptr %11, !tbaa !2
  %943 = getelementptr i8, ptr %905, i16 -4
  %944 = load i16, ptr %943
  %945 = icmp ult i16 %942, %944
  br i1 %945, label %b259, label %b260

b252:
  br label %b249

b253:
  %946 = mul i16 %938, 6
  %947 = getelementptr inbounds i8, ptr %905, i16 %946
  %948 = load i16, ptr %947
  %949 = icmp ne i16 %948, 0
  br i1 %949, label %b251, label %b252

b254:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b255:
  store i8 1, ptr %10, !tbaa !2
  br label %b252

b256:
  br label %b257

b257:
  %950 = load i16, ptr %11, !tbaa !2
  %951 = add i16 %950, 1
  %952 = load i16, ptr %9, !tbaa !2
  %953 = and i16 %951, %952
  store i16 %953, ptr %11, !tbaa !2
  br label %b250

b258:
  %954 = load i16, ptr %11, !tbaa !2
  %955 = getelementptr i8, ptr %905, i16 -4
  %956 = load i16, ptr %955
  %957 = icmp ult i16 %954, %956
  br i1 %957, label %b261, label %b262

b259:
  %958 = mul i16 %942, 6
  %959 = getelementptr inbounds i8, ptr %905, i16 %958
  %960 = load i16, ptr %959
  %961 = load i16, ptr %12, !tbaa !2
  %962 = icmp eq i16 %960, %961
  br i1 %962, label %b258, label %b256

b260:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b261:
  %963 = mul i16 %954, 6
  %964 = getelementptr inbounds i8, ptr %905, i16 %963
  %965 = getelementptr i8, ptr %964, i16 2
  %966 = load i16, ptr %965
  %967 = load i16, ptr %13, !tbaa !2
  %968 = call cc 1005 addrspace(1) i8 @i16.eq(i16 %966, i16 %967)
  %969 = icmp ne i8 %968, 0
  br i1 %969, label %b255, label %b256

b262:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b263:
  %970 = load i16, ptr %11, !tbaa !2
  %971 = getelementptr i8, ptr %905, i16 -4
  %972 = load i16, ptr %971
  %973 = icmp ult i16 %970, %972
  br i1 %973, label %b266, label %b267

b264:
  br label %b265

b265:
  %974 = load i8, ptr %10, !tbaa !2, !range !22
  %975 = icmp ne i8 %974, 0
  br i1 %975, label %b271, label %b270

b266:
  %976 = mul i16 %970, 6
  %977 = getelementptr inbounds i8, ptr %905, i16 %976
  %978 = load i16, ptr %12, !tbaa !2
  store i16 %978, ptr %977
  %979 = load i16, ptr %11, !tbaa !2
  %980 = getelementptr i8, ptr %905, i16 -4
  %981 = load i16, ptr %980
  %982 = icmp ult i16 %979, %981
  br i1 %982, label %b268, label %b269

b267:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b268:
  %983 = mul i16 %979, 6
  %984 = getelementptr inbounds i8, ptr %905, i16 %983
  %985 = load i16, ptr %13, !tbaa !2
  %986 = getelementptr i8, ptr %984, i16 2
  store i16 %985, ptr %986
  br label %b265

b269:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b270:
  %987 = getelementptr i8, ptr %905, i16 -2
  %988 = load i16, ptr %987
  %989 = add i16 %988, 1
  %990 = getelementptr i8, ptr %905, i16 -2
  store i16 %989, ptr %990
  br label %b271

b271:
  %991 = load i16, ptr %11, !tbaa !2
  %992 = getelementptr i8, ptr %905, i16 -4
  %993 = load i16, ptr %992
  %994 = icmp ult i16 %991, %993
  br i1 %994, label %b272, label %b273

b272:
  %995 = mul i16 %991, 6
  %996 = getelementptr inbounds i8, ptr %905, i16 %995
  %997 = load i16, ptr %15, !tbaa !2
  %998 = load i16, ptr %15, !tbaa !2
  %999 = mul i16 %997, %998
  %1000 = getelementptr i8, ptr %996, i16 4
  store i16 %999, ptr %1000
  br label %b245

b273:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b274:
  %1001 = getelementptr i8, ptr %922, i16 -4
  %1002 = load i16, ptr %1001
  %1003 = sub i16 %1002, 1
  store i16 %1003, ptr %3, !tbaa !2
  %1004 = load i16, ptr %6, !tbaa !2
  %1005 = load i16, ptr %3, !tbaa !2
  %1006 = and i16 %1004, %1005
  store i16 %1006, ptr %5, !tbaa !2
  br label %b277

b275:
  br label %b276

b276:
  %1007 = load i8, ptr %4, !tbaa !2, !range !23
  %1008 = icmp ne i8 %1007, 0
  br i1 %1008, label %b290, label %b291

b277:
  %1009 = load i16, ptr %5, !tbaa !2
  %1010 = getelementptr i8, ptr %922, i16 -4
  %1011 = load i16, ptr %1010
  %1012 = icmp ult i16 %1009, %1011
  br i1 %1012, label %b280, label %b281

b278:
  %1013 = load i16, ptr %5, !tbaa !2
  %1014 = getelementptr i8, ptr %922, i16 -4
  %1015 = load i16, ptr %1014
  %1016 = icmp ult i16 %1013, %1015
  br i1 %1016, label %b286, label %b287

b279:
  br label %b276

b280:
  %1017 = mul i16 %1009, 6
  %1018 = getelementptr inbounds i8, ptr %922, i16 %1017
  %1019 = load i16, ptr %1018
  %1020 = icmp ne i16 %1019, 0
  br i1 %1020, label %b278, label %b279

b281:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b282:
  store i8 1, ptr %4, !tbaa !2
  br label %b279

b283:
  br label %b284

b284:
  %1021 = load i16, ptr %5, !tbaa !2
  %1022 = add i16 %1021, 1
  %1023 = load i16, ptr %3, !tbaa !2
  %1024 = and i16 %1022, %1023
  store i16 %1024, ptr %5, !tbaa !2
  br label %b277

b285:
  %1025 = load i16, ptr %5, !tbaa !2
  %1026 = getelementptr i8, ptr %922, i16 -4
  %1027 = load i16, ptr %1026
  %1028 = icmp ult i16 %1025, %1027
  br i1 %1028, label %b288, label %b289

b286:
  %1029 = mul i16 %1013, 6
  %1030 = getelementptr inbounds i8, ptr %922, i16 %1029
  %1031 = load i16, ptr %1030
  %1032 = load i16, ptr %6, !tbaa !2
  %1033 = icmp eq i16 %1031, %1032
  br i1 %1033, label %b285, label %b283

b287:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b288:
  %1034 = mul i16 %1025, 6
  %1035 = getelementptr inbounds i8, ptr %922, i16 %1034
  %1036 = getelementptr i8, ptr %1035, i16 2
  %1037 = load i16, ptr %1036
  %1038 = load i16, ptr addrspace(1) %923, !tbaa !2
  %1039 = call cc 1005 addrspace(1) i8 @i16.eq(i16 %1037, i16 %1038)
  %1040 = icmp ne i8 %1039, 0
  br i1 %1040, label %b282, label %b283

b289:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b290:
  %1041 = load i16, ptr %5, !tbaa !2
  %1042 = getelementptr i8, ptr %922, i16 -4
  %1043 = load i16, ptr %1042
  %1044 = icmp ult i16 %1041, %1043
  br i1 %1044, label %b292, label %b293

b291:
  call cc 1005 addrspace(1) void @N$EKEY()
  unreachable

b292:
  %1045 = mul i16 %1041, 6
  %1046 = getelementptr inbounds i8, ptr %922, i16 %1045
  %1047 = getelementptr i8, ptr %1046, i16 4
  %1048 = load i16, ptr %1047
  call cc 1005 addrspace(1) void @N$PI2(i16 %1048)
  call cc 1005 addrspace(1) void @N$PN()
  %1049 = load ptr, ptr %8, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1049)
  %1050 = load ptr, ptr %16, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1050)
  %1051 = load ptr, ptr %41, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1051)
  %1052 = load ptr, ptr %42, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1052)
  %1053 = load ptr, ptr %43, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1053)
  %1054 = load ptr, ptr %54, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1054)
  %1055 = load ptr, ptr %55, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1055)
  %1056 = load ptr, ptr %65, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1056)
  %1057 = load ptr, ptr %76, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1057)
  %1058 = load ptr, ptr %93, !tbaa !2
  %1059 = icmp ne ptr %1058, null
  br i1 %1059, label %b295, label %b294

b293:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b294:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1058)
  %1060 = load ptr, ptr %94, !tbaa !2
  %1061 = icmp ne ptr %1060, null
  br i1 %1061, label %b300, label %b299

b295:
  %1062 = getelementptr i8, ptr %1058, i16 -4
  %1063 = load i16, ptr %1062
  store i16 0, ptr %2, !tbaa !2
  br label %b296

b296:
  %1064 = load i16, ptr %2, !tbaa !2
  %1065 = icmp ult i16 %1064, %1063
  br i1 %1065, label %b298, label %b297

b297:
  br label %b294

b298:
  %1066 = mul i16 %1064, 6
  %1067 = getelementptr inbounds i8, ptr %1058, i16 %1066
  %1068 = getelementptr i8, ptr %1067, i16 2
  %1069 = load ptr, ptr %1068
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1069)
  %1070 = add i16 %1064, 1
  store i16 %1070, ptr %2, !tbaa !2
  br label %b296

b299:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1060)
  %1071 = load ptr, ptr %95, !tbaa !2
  %1072 = icmp ne ptr %1071, null
  br i1 %1072, label %b305, label %b304

b300:
  %1073 = getelementptr i8, ptr %1060, i16 -4
  %1074 = load i16, ptr %1073
  store i16 0, ptr %1, !tbaa !2
  br label %b301

b301:
  %1075 = load i16, ptr %1, !tbaa !2
  %1076 = icmp ult i16 %1075, %1074
  br i1 %1076, label %b303, label %b302

b302:
  br label %b299

b303:
  %1077 = mul i16 %1075, 6
  %1078 = getelementptr inbounds i8, ptr %1060, i16 %1077
  %1079 = getelementptr i8, ptr %1078, i16 2
  %1080 = load ptr, ptr %1079
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1080)
  %1081 = add i16 %1075, 1
  store i16 %1081, ptr %1, !tbaa !2
  br label %b301

b304:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1071)
  ret i16 0

b305:
  %1082 = getelementptr i8, ptr %1071, i16 -4
  %1083 = load i16, ptr %1082
  store i16 0, ptr %0, !tbaa !2
  br label %b306

b306:
  %1084 = load i16, ptr %0, !tbaa !2
  %1085 = icmp ult i16 %1084, %1083
  br i1 %1085, label %b308, label %b307

b307:
  br label %b304

b308:
  %1086 = mul i16 %1084, 2
  %1087 = getelementptr inbounds i8, ptr %1071, i16 %1086
  %1088 = load ptr, ptr %1087
  call cc 1005 addrspace(1) void @N$BDRP(ptr %1088)
  %1089 = add i16 %1084, 1
  store i16 %1089, ptr %0, !tbaa !2
  br label %b306
}

define internal cc 1005 i8 @i16.eq(i16 %0, i16 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = icmp eq i16 %0, %1
  %3 = zext i1 %2 to i8
  ret i8 %3
}

define internal cc 1005 i16 @i16.hash(i16 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  ret i16 %0
}

define internal cc 1005 i8 @Card.eq(ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %0, ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = alloca i8
  %3 = load i8, ptr addrspace(1) %0
  %4 = load i8, ptr addrspace(1) %1
  %5 = call cc 1005 addrspace(1) i8 @u8.eq(i8 zeroext %3, i8 zeroext %4)
  store i8 %5, ptr %2, !tbaa !2
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b2, label %b3

b2:
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 1
  %8 = load i8, ptr addrspace(1) %7
  %9 = getelementptr i8, ptr addrspace(1) %1, i16 1
  %10 = load i8, ptr addrspace(1) %9
  %11 = call cc 1005 addrspace(1) i8 @Suit.eq(i8 zeroext %8, i8 zeroext %10)
  store i8 %11, ptr %2, !tbaa !2
  br label %b3

b3:
  %12 = load i8, ptr %2, !tbaa !2, !range !24
  ret i8 %12
}

define internal cc 1005 i8 @u8.eq(i8 %0, i8 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = zext i8 %0 to i16
  %3 = zext i8 %1 to i16
  %4 = icmp eq i16 %2, %3
  %5 = zext i1 %4 to i8
  ret i8 %5
}

define internal cc 1005 i16 @Card.hash(ptr addrspace(1) nonnull dereferenceable(2) readonly noalias nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  store i16 0, ptr %1, !tbaa !2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = mul i16 %2, 31
  %4 = load i8, ptr addrspace(1) %0
  %5 = call cc 1005 addrspace(1) i16 @u8.hash(i8 zeroext %4)
  %6 = add i16 %3, %5
  store i16 %6, ptr %1, !tbaa !2
  %7 = load i16, ptr %1, !tbaa !2
  %8 = mul i16 %7, 31
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 1
  %10 = load i8, ptr addrspace(1) %9
  %11 = call cc 1005 addrspace(1) i16 @Suit.hash(i8 zeroext %10)
  %12 = add i16 %8, %11
  store i16 %12, ptr %1, !tbaa !2
  %13 = load i16, ptr %1, !tbaa !2
  ret i16 %13
}

define internal cc 1005 i16 @u8.hash(i8 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = zext i8 %0 to i16
  ret i16 %1
}

define internal cc 1005 i8 @Suit.eq(i8 %0, i8 %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = icmp eq i8 %0, %1
  %3 = zext i1 %2 to i8
  ret i8 %3
}

define internal cc 1005 i16 @Suit.hash(i8 %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = zext i8 %0 to i16
  ret i16 %1
}

define internal cc 1005 i8 @string.eq(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0, ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %1) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %2 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %0, ptr addrspace(1) %1)
  %3 = icmp eq i8 %2, 0
  %4 = zext i1 %3 to i8
  ret i8 %4
}

define internal cc 1005 i16 @string.hash(ptr addrspace(1) noalias readonly dereferenceable(8) nocapture %0) addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 5381, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  br i1 %5, label %b3, label %b5

b3:
  %6 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %7 = load ptr addrspace(1), ptr addrspace(1) %6
  %8 = getelementptr inbounds i8, ptr addrspace(1) %7, i16 %4
  %9 = load i16, ptr %2, !tbaa !2
  %10 = mul i16 %9, 33
  %11 = load i8, ptr addrspace(1) %8
  %12 = zext i8 %11 to i16
  %13 = xor i16 %10, %12
  store i16 %13, ptr %2, !tbaa !2
  br label %b4

b4:
  %14 = load i16, ptr %1, !tbaa !2
  %15 = add i16 %14, 1
  store i16 %15, ptr %1, !tbaa !2
  br label %b2

b5:
  %16 = load i16, ptr %2, !tbaa !2
  ret i16 %16
}

declare cc 1005 ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare cc 1005 ptr @N$DRES(ptr, i16) addrspace(1)

declare cc 1005 ptr @N$BCLN(ptr, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

declare cc 1005 void @N$EKEY() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PB(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1) memory(read)

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
!17 = !{i8 0, i8 2}
!18 = !{i8 0, i8 2}
!19 = !{i8 0, i8 2}
!20 = !{i8 0, i8 2}
!21 = !{i8 0, i8 2}
!22 = !{i8 0, i8 2}
!23 = !{i8 0, i8 2}
!24 = !{i8 0, i8 2}
