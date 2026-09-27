target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-i32:16-i64:16"

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

define internal i16 @main() addrspace(1) {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i16
  %3 = alloca i8
  %4 = alloca i16
  %5 = alloca i8
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i16
  %9 = alloca ptr
  %10 = alloca i8
  %11 = alloca i16
  %12 = alloca i8
  %13 = alloca i16
  %14 = alloca i16
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca ptr
  %19 = alloca i8
  %20 = alloca i8
  %21 = alloca i16
  %22 = alloca i8
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca [2 x i8]
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca i8
  %29 = alloca i16
  %30 = alloca i8
  %31 = alloca i16
  %32 = alloca i16
  %33 = alloca i8
  %34 = alloca i8
  %35 = alloca i16
  %36 = alloca i8
  %37 = alloca i16
  %38 = alloca i16
  %39 = alloca [2 x i8]
  %40 = alloca i8
  %41 = alloca i16
  %42 = alloca i8
  %43 = alloca i16
  %44 = alloca i16
  %45 = alloca i8
  %46 = alloca i16
  %47 = alloca ptr
  %48 = alloca ptr
  %49 = alloca ptr
  %50 = alloca i8
  %51 = alloca i16
  %52 = alloca i8
  %53 = alloca i16
  %54 = alloca i16
  %55 = alloca i8
  %56 = alloca i8
  %57 = alloca i16
  %58 = alloca i8
  %59 = alloca i16
  %60 = alloca i16
  %61 = alloca i8
  %62 = alloca ptr
  %63 = alloca ptr
  %64 = alloca i16
  %65 = alloca i16
  %66 = alloca [8 x i8]
  %67 = alloca [8 x i8]
  %68 = alloca i8
  %69 = alloca i16
  %70 = alloca i8
  %71 = alloca i16
  %72 = alloca i16
  %73 = alloca [8 x i8]
  %74 = alloca ptr
  %75 = alloca i16
  %76 = alloca i16
  %77 = alloca i16
  %78 = alloca [8 x i8]
  %79 = alloca [8 x i8]
  %80 = alloca i8
  %81 = alloca i16
  %82 = alloca i8
  %83 = alloca i16
  %84 = alloca i16
  %85 = alloca [8 x i8]
  %86 = alloca ptr
  %87 = alloca i16
  %88 = alloca [8 x i8]
  %89 = alloca i8
  %90 = alloca i16
  %91 = alloca i8
  %92 = alloca i16
  %93 = alloca i16
  %94 = alloca [8 x i8]
  %95 = alloca [8 x i8]
  %96 = alloca [8 x i8]
  %97 = alloca i8
  %98 = alloca i16
  %99 = alloca i8
  %100 = alloca i16
  %101 = alloca i16
  %102 = alloca [8 x i8]
  %103 = alloca ptr
  %104 = alloca i16
  %105 = alloca ptr
  %106 = alloca ptr
  %107 = alloca ptr
  store i16 0, ptr %0
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i8 0, ptr %3
  store i16 0, ptr %4
  store i8 0, ptr %5
  store i16 0, ptr %6
  store i16 0, ptr %7
  store i16 0, ptr %8
  store ptr null, ptr %9
  store i8 0, ptr %10
  store i16 0, ptr %11
  store i8 0, ptr %12
  store i16 0, ptr %13
  store i16 0, ptr %14
  store i16 0, ptr %15
  store i16 0, ptr %16
  store i16 0, ptr %17
  store ptr null, ptr %18
  store i8 0, ptr %19
  store i8 0, ptr %20
  store i16 0, ptr %21
  store i8 0, ptr %22
  store i16 0, ptr %23
  store i16 0, ptr %24
  call void @llvm.memset.p0.i16(ptr %25, i8 0, i16 2, i1 false)
  store i16 0, ptr %26
  store i16 0, ptr %27
  store i8 0, ptr %28
  store i16 0, ptr %29
  store i8 0, ptr %30
  store i16 0, ptr %31
  store i16 0, ptr %32
  store i8 0, ptr %33
  store i8 0, ptr %34
  store i16 0, ptr %35
  store i8 0, ptr %36
  store i16 0, ptr %37
  store i16 0, ptr %38
  call void @llvm.memset.p0.i16(ptr %39, i8 0, i16 2, i1 false)
  store i8 0, ptr %40
  store i16 0, ptr %41
  store i8 0, ptr %42
  store i16 0, ptr %43
  store i16 0, ptr %44
  store i8 0, ptr %45
  store i16 0, ptr %46
  store ptr null, ptr %47
  store ptr null, ptr %48
  store ptr null, ptr %49
  store i8 0, ptr %50
  store i16 0, ptr %51
  store i8 0, ptr %52
  store i16 0, ptr %53
  store i16 0, ptr %54
  store i8 0, ptr %55
  store i8 0, ptr %56
  store i16 0, ptr %57
  store i8 0, ptr %58
  store i16 0, ptr %59
  store i16 0, ptr %60
  store i8 0, ptr %61
  store ptr null, ptr %62
  store ptr null, ptr %63
  store i16 0, ptr %64
  store i16 0, ptr %65
  call void @llvm.memset.p0.i16(ptr %66, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %67, i8 0, i16 8, i1 false)
  store i8 0, ptr %68
  store i16 0, ptr %69
  store i8 0, ptr %70
  store i16 0, ptr %71
  store i16 0, ptr %72
  call void @llvm.memset.p0.i16(ptr %73, i8 0, i16 8, i1 false)
  store ptr null, ptr %74
  store i16 0, ptr %75
  store i16 0, ptr %76
  store i16 0, ptr %77
  call void @llvm.memset.p0.i16(ptr %78, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %79, i8 0, i16 8, i1 false)
  store i8 0, ptr %80
  store i16 0, ptr %81
  store i8 0, ptr %82
  store i16 0, ptr %83
  store i16 0, ptr %84
  call void @llvm.memset.p0.i16(ptr %85, i8 0, i16 8, i1 false)
  store ptr null, ptr %86
  store i16 0, ptr %87
  call void @llvm.memset.p0.i16(ptr %88, i8 0, i16 8, i1 false)
  store i8 0, ptr %89
  store i16 0, ptr %90
  store i8 0, ptr %91
  store i16 0, ptr %92
  store i16 0, ptr %93
  call void @llvm.memset.p0.i16(ptr %94, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %95, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %96, i8 0, i16 8, i1 false)
  store i8 0, ptr %97
  store i16 0, ptr %98
  store i8 0, ptr %99
  store i16 0, ptr %100
  store i16 0, ptr %101
  call void @llvm.memset.p0.i16(ptr %102, i8 0, i16 8, i1 false)
  store ptr null, ptr %103
  store i16 0, ptr %104
  store ptr null, ptr %105
  store ptr null, ptr %106
  store ptr null, ptr %107
  %108 = getelementptr i8, ptr @$str1, i16 6
  %109 = call addrspace(1) ptr @N$BGRW(ptr %108, i16 9, i16 2)
  %110 = getelementptr i8, ptr %109, i16 0
  %111 = getelementptr i8, ptr @$str2, i16 6
  store ptr %111, ptr %110
  %112 = getelementptr i8, ptr %109, i16 2
  %113 = getelementptr i8, ptr @$str3, i16 6
  store ptr %113, ptr %112
  %114 = getelementptr i8, ptr %109, i16 4
  %115 = getelementptr i8, ptr @$str4, i16 6
  store ptr %115, ptr %114
  %116 = getelementptr i8, ptr %109, i16 6
  %117 = getelementptr i8, ptr @$str2, i16 6
  store ptr %117, ptr %116
  %118 = getelementptr i8, ptr %109, i16 8
  %119 = getelementptr i8, ptr @$str5, i16 6
  store ptr %119, ptr %118
  %120 = getelementptr i8, ptr %109, i16 10
  %121 = getelementptr i8, ptr @$str6, i16 6
  store ptr %121, ptr %120
  %122 = getelementptr i8, ptr %109, i16 12
  %123 = getelementptr i8, ptr @$str2, i16 6
  store ptr %123, ptr %122
  %124 = getelementptr i8, ptr %109, i16 14
  %125 = getelementptr i8, ptr @$str3, i16 6
  store ptr %125, ptr %124
  %126 = getelementptr i8, ptr %109, i16 16
  %127 = getelementptr i8, ptr @$str7, i16 6
  store ptr %127, ptr %126
  store ptr %109, ptr %107, !tbaa !2
  %128 = getelementptr i8, ptr @$str1, i16 6
  store ptr %128, ptr %106, !tbaa !2
  %129 = load ptr, ptr %106, !tbaa !2
  store ptr null, ptr %106, !tbaa !2
  store ptr %129, ptr %105, !tbaa !2
  %130 = load ptr, ptr %107, !tbaa !2
  %131 = getelementptr i8, ptr %130, i16 -4
  %132 = load i16, ptr %131
  store i16 0, ptr %104, !tbaa !2
  br label %b2

b2:
  %133 = load i16, ptr %104, !tbaa !2
  %134 = icmp ult i16 %133, %132
  %135 = sext i1 %134 to i8
  %136 = icmp ne i8 %135, 0
  br i1 %136, label %b3, label %b5

b3:
  %137 = mul i16 %133, 2
  %138 = getelementptr i8, ptr %130, i16 %137
  %139 = load ptr, ptr %105, !tbaa !2
  %140 = call addrspace(1) ptr @N$DRES(ptr %139, i16 6)
  store ptr %140, ptr %105, !tbaa !2
  %141 = load ptr, ptr %138
  %142 = call addrspace(1) ptr @N$BCLN(ptr %141, i16 1)
  store ptr %142, ptr %103, !tbaa !2
  %143 = load ptr, ptr %103, !tbaa !2
  %144 = getelementptr i8, ptr %143, i16 -4
  %145 = load i16, ptr %144
  %146 = addrspacecast ptr %143 to ptr addrspace(1)
  store i16 %145, ptr %102, !tbaa !2
  %147 = getelementptr inbounds i8, ptr %102, i16 2
  store i16 %145, ptr %147, !tbaa !2
  %148 = getelementptr inbounds i8, ptr %102, i16 4
  store ptr addrspace(1) %146, ptr %148, !tbaa !2
  %149 = addrspacecast ptr %102 to ptr addrspace(1)
  %150 = call addrspace(1) i16 @string.hash(ptr addrspace(1) %149)
  %151 = or i16 %150, 1
  store i16 %151, ptr %101, !tbaa !2
  store i16 0, ptr %100, !tbaa !2
  store i8 0, ptr %99, !tbaa !2
  %152 = getelementptr i8, ptr %140, i16 -4
  %153 = load i16, ptr %152
  %154 = icmp ne i16 %153, 0
  %155 = sext i1 %154 to i8
  %156 = icmp ne i8 %155, 0
  br i1 %156, label %b6, label %b7

b4:
  %157 = load i16, ptr %104, !tbaa !2
  %158 = add i16 %157, 1
  store i16 %158, ptr %104, !tbaa !2
  br label %b2

b5:
  %159 = load ptr, ptr %105, !tbaa !2
  %160 = getelementptr i8, ptr @$str2, i16 6
  store ptr %160, ptr %86, !tbaa !2
  %161 = load ptr, ptr %86, !tbaa !2
  %162 = getelementptr i8, ptr %161, i16 -4
  %163 = load i16, ptr %162
  %164 = addrspacecast ptr %161 to ptr addrspace(1)
  store i16 %163, ptr %85, !tbaa !2
  %165 = getelementptr inbounds i8, ptr %85, i16 2
  store i16 %163, ptr %165, !tbaa !2
  %166 = getelementptr inbounds i8, ptr %85, i16 4
  store ptr addrspace(1) %164, ptr %166, !tbaa !2
  %167 = addrspacecast ptr %85 to ptr addrspace(1)
  %168 = call addrspace(1) i16 @string.hash(ptr addrspace(1) %167)
  %169 = or i16 %168, 1
  store i16 %169, ptr %84, !tbaa !2
  store i16 0, ptr %83, !tbaa !2
  store i8 0, ptr %82, !tbaa !2
  %170 = getelementptr i8, ptr %159, i16 -4
  %171 = load i16, ptr %170
  %172 = icmp ne i16 %171, 0
  %173 = sext i1 %172 to i8
  %174 = icmp ne i8 %173, 0
  br i1 %174, label %b56, label %b57

b6:
  %175 = getelementptr i8, ptr %140, i16 -4
  %176 = load i16, ptr %175
  %177 = sub i16 %176, 1
  store i16 %177, ptr %98, !tbaa !2
  %178 = load i16, ptr %101, !tbaa !2
  %179 = load i16, ptr %98, !tbaa !2
  %180 = and i16 %178, %179
  store i16 %180, ptr %100, !tbaa !2
  br label %b9

b7:
  br label %b8

b8:
  %181 = load i8, ptr %99, !tbaa !2
  %182 = xor i8 %181, -1
  %183 = icmp ne i8 %182, 0
  br i1 %183, label %b23, label %b24

b9:
  %184 = load i16, ptr %100, !tbaa !2
  %185 = getelementptr i8, ptr %140, i16 -4
  %186 = load i16, ptr %185
  %187 = icmp ult i16 %184, %186
  %188 = sext i1 %187 to i8
  %189 = icmp ne i8 %188, 0
  br i1 %189, label %b12, label %b13

b10:
  %190 = load i16, ptr %100, !tbaa !2
  %191 = getelementptr i8, ptr %140, i16 -4
  %192 = load i16, ptr %191
  %193 = icmp ult i16 %190, %192
  %194 = sext i1 %193 to i8
  %195 = icmp ne i8 %194, 0
  br i1 %195, label %b14, label %b15

b11:
  br label %b8

b12:
  %196 = mul i16 %184, 6
  %197 = getelementptr i8, ptr %140, i16 %196
  %198 = load i16, ptr %197
  %199 = icmp ne i16 %198, 0
  %200 = sext i1 %199 to i8
  %201 = icmp ne i8 %200, 0
  br i1 %201, label %b10, label %b11

b13:
  call addrspace(1) void @N$EBND()
  unreachable

b14:
  %202 = mul i16 %190, 6
  %203 = getelementptr i8, ptr %140, i16 %202
  %204 = load i16, ptr %203
  %205 = load i16, ptr %101, !tbaa !2
  %206 = icmp eq i16 %204, %205
  %207 = sext i1 %206 to i8
  store i8 %207, ptr %97, !tbaa !2
  %208 = icmp ne i8 %207, 0
  br i1 %208, label %b16, label %b17

b15:
  call addrspace(1) void @N$EBND()
  unreachable

b16:
  %209 = load i16, ptr %100, !tbaa !2
  %210 = getelementptr i8, ptr %140, i16 -4
  %211 = load i16, ptr %210
  %212 = icmp ult i16 %209, %211
  %213 = sext i1 %212 to i8
  %214 = icmp ne i8 %213, 0
  br i1 %214, label %b18, label %b19

b17:
  %215 = load i8, ptr %97, !tbaa !2
  %216 = icmp ne i8 %215, 0
  br i1 %216, label %b20, label %b21

b18:
  %217 = mul i16 %209, 6
  %218 = getelementptr i8, ptr %140, i16 %217
  %219 = getelementptr i8, ptr %218, i16 2
  %220 = load ptr, ptr %219
  %221 = getelementptr i8, ptr %220, i16 -4
  %222 = load i16, ptr %221
  %223 = addrspacecast ptr %220 to ptr addrspace(1)
  store i16 %222, ptr %96, !tbaa !2
  %224 = getelementptr inbounds i8, ptr %96, i16 2
  store i16 %222, ptr %224, !tbaa !2
  %225 = getelementptr inbounds i8, ptr %96, i16 4
  store ptr addrspace(1) %223, ptr %225, !tbaa !2
  %226 = addrspacecast ptr %96 to ptr addrspace(1)
  %227 = load ptr, ptr %103, !tbaa !2
  %228 = getelementptr i8, ptr %227, i16 -4
  %229 = load i16, ptr %228
  %230 = addrspacecast ptr %227 to ptr addrspace(1)
  store i16 %229, ptr %95, !tbaa !2
  %231 = getelementptr inbounds i8, ptr %95, i16 2
  store i16 %229, ptr %231, !tbaa !2
  %232 = getelementptr inbounds i8, ptr %95, i16 4
  store ptr addrspace(1) %230, ptr %232, !tbaa !2
  %233 = addrspacecast ptr %95 to ptr addrspace(1)
  %234 = call addrspace(1) i8 @string.eq(ptr addrspace(1) %226, ptr addrspace(1) %233)
  store i8 %234, ptr %97, !tbaa !2
  br label %b17

b19:
  call addrspace(1) void @N$EBND()
  unreachable

b20:
  store i8 -1, ptr %99, !tbaa !2
  br label %b11

b21:
  br label %b22

b22:
  %235 = load i16, ptr %100, !tbaa !2
  %236 = add i16 %235, 1
  %237 = load i16, ptr %98, !tbaa !2
  %238 = and i16 %236, %237
  store i16 %238, ptr %100, !tbaa !2
  br label %b9

b23:
  %239 = load i16, ptr %100, !tbaa !2
  %240 = getelementptr i8, ptr %140, i16 -4
  %241 = load i16, ptr %240
  %242 = icmp ult i16 %239, %241
  %243 = sext i1 %242 to i8
  %244 = icmp ne i8 %243, 0
  br i1 %244, label %b26, label %b27

b24:
  br label %b25

b25:
  %245 = load i8, ptr %99, !tbaa !2
  %246 = icmp ne i8 %245, 0
  br i1 %246, label %b31, label %b30

b26:
  %247 = mul i16 %239, 6
  %248 = getelementptr i8, ptr %140, i16 %247
  %249 = load i16, ptr %101, !tbaa !2
  store i16 %249, ptr %248
  %250 = load i16, ptr %100, !tbaa !2
  %251 = getelementptr i8, ptr %140, i16 -4
  %252 = load i16, ptr %251
  %253 = icmp ult i16 %250, %252
  %254 = sext i1 %253 to i8
  %255 = icmp ne i8 %254, 0
  br i1 %255, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %256 = mul i16 %250, 6
  %257 = getelementptr i8, ptr %140, i16 %256
  %258 = load ptr, ptr %103, !tbaa !2
  store ptr null, ptr %103, !tbaa !2
  %259 = getelementptr i8, ptr %257, i16 2
  %260 = load ptr, ptr %259
  call addrspace(1) void @N$BDRP(ptr %260)
  %261 = getelementptr i8, ptr %257, i16 2
  store ptr %258, ptr %261
  br label %b25

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  %262 = getelementptr i8, ptr %140, i16 -2
  %263 = load i16, ptr %262
  %264 = add i16 %263, 1
  %265 = getelementptr i8, ptr %140, i16 -2
  store i16 %264, ptr %265
  br label %b31

b31:
  %266 = load i16, ptr %100, !tbaa !2
  %267 = getelementptr i8, ptr %140, i16 -4
  %268 = load i16, ptr %267
  %269 = icmp ult i16 %266, %268
  %270 = sext i1 %269 to i8
  %271 = icmp ne i8 %270, 0
  br i1 %271, label %b32, label %b33

b32:
  %272 = mul i16 %266, 6
  %273 = getelementptr i8, ptr %140, i16 %272
  %274 = load ptr, ptr %105, !tbaa !2
  %275 = load ptr, ptr %138
  %276 = getelementptr i8, ptr %275, i16 -4
  %277 = load i16, ptr %276
  %278 = addrspacecast ptr %275 to ptr addrspace(1)
  store i16 %277, ptr %94, !tbaa !2
  %279 = getelementptr inbounds i8, ptr %94, i16 2
  store i16 %277, ptr %279, !tbaa !2
  %280 = getelementptr inbounds i8, ptr %94, i16 4
  store ptr addrspace(1) %278, ptr %280, !tbaa !2
  %281 = addrspacecast ptr %94 to ptr addrspace(1)
  %282 = call addrspace(1) i16 @string.hash(ptr addrspace(1) %281)
  %283 = or i16 %282, 1
  store i16 %283, ptr %93, !tbaa !2
  store i16 0, ptr %92, !tbaa !2
  store i8 0, ptr %91, !tbaa !2
  %284 = getelementptr i8, ptr %274, i16 -4
  %285 = load i16, ptr %284
  %286 = icmp ne i16 %285, 0
  %287 = sext i1 %286 to i8
  %288 = icmp ne i8 %287, 0
  br i1 %288, label %b34, label %b35

b33:
  call addrspace(1) void @N$EBND()
  unreachable

b34:
  %289 = getelementptr i8, ptr %274, i16 -4
  %290 = load i16, ptr %289
  %291 = sub i16 %290, 1
  store i16 %291, ptr %90, !tbaa !2
  %292 = load i16, ptr %93, !tbaa !2
  %293 = load i16, ptr %90, !tbaa !2
  %294 = and i16 %292, %293
  store i16 %294, ptr %92, !tbaa !2
  br label %b37

b35:
  br label %b36

b36:
  %295 = load i8, ptr %91, !tbaa !2
  %296 = icmp ne i8 %295, 0
  br i1 %296, label %b51, label %b52

b37:
  %297 = load i16, ptr %92, !tbaa !2
  %298 = getelementptr i8, ptr %274, i16 -4
  %299 = load i16, ptr %298
  %300 = icmp ult i16 %297, %299
  %301 = sext i1 %300 to i8
  %302 = icmp ne i8 %301, 0
  br i1 %302, label %b40, label %b41

b38:
  %303 = load i16, ptr %92, !tbaa !2
  %304 = getelementptr i8, ptr %274, i16 -4
  %305 = load i16, ptr %304
  %306 = icmp ult i16 %303, %305
  %307 = sext i1 %306 to i8
  %308 = icmp ne i8 %307, 0
  br i1 %308, label %b42, label %b43

b39:
  br label %b36

b40:
  %309 = mul i16 %297, 6
  %310 = getelementptr i8, ptr %274, i16 %309
  %311 = load i16, ptr %310
  %312 = icmp ne i16 %311, 0
  %313 = sext i1 %312 to i8
  %314 = icmp ne i8 %313, 0
  br i1 %314, label %b38, label %b39

b41:
  call addrspace(1) void @N$EBND()
  unreachable

b42:
  %315 = mul i16 %303, 6
  %316 = getelementptr i8, ptr %274, i16 %315
  %317 = load i16, ptr %316
  %318 = load i16, ptr %93, !tbaa !2
  %319 = icmp eq i16 %317, %318
  %320 = sext i1 %319 to i8
  store i8 %320, ptr %89, !tbaa !2
  %321 = icmp ne i8 %320, 0
  br i1 %321, label %b44, label %b45

b43:
  call addrspace(1) void @N$EBND()
  unreachable

b44:
  %322 = load i16, ptr %92, !tbaa !2
  %323 = getelementptr i8, ptr %274, i16 -4
  %324 = load i16, ptr %323
  %325 = icmp ult i16 %322, %324
  %326 = sext i1 %325 to i8
  %327 = icmp ne i8 %326, 0
  br i1 %327, label %b46, label %b47

b45:
  %328 = load i8, ptr %89, !tbaa !2
  %329 = icmp ne i8 %328, 0
  br i1 %329, label %b48, label %b49

b46:
  %330 = mul i16 %322, 6
  %331 = getelementptr i8, ptr %274, i16 %330
  %332 = getelementptr i8, ptr %331, i16 2
  %333 = load ptr, ptr %332
  %334 = getelementptr i8, ptr %333, i16 -4
  %335 = load i16, ptr %334
  %336 = addrspacecast ptr %333 to ptr addrspace(1)
  store i16 %335, ptr %88, !tbaa !2
  %337 = getelementptr inbounds i8, ptr %88, i16 2
  store i16 %335, ptr %337, !tbaa !2
  %338 = getelementptr inbounds i8, ptr %88, i16 4
  store ptr addrspace(1) %336, ptr %338, !tbaa !2
  %339 = addrspacecast ptr %88 to ptr addrspace(1)
  %340 = call addrspace(1) i8 @string.eq(ptr addrspace(1) %339, ptr addrspace(1) %281)
  store i8 %340, ptr %89, !tbaa !2
  br label %b45

b47:
  call addrspace(1) void @N$EBND()
  unreachable

b48:
  store i8 -1, ptr %91, !tbaa !2
  br label %b39

b49:
  br label %b50

b50:
  %341 = load i16, ptr %92, !tbaa !2
  %342 = add i16 %341, 1
  %343 = load i16, ptr %90, !tbaa !2
  %344 = and i16 %342, %343
  store i16 %344, ptr %92, !tbaa !2
  br label %b37

b51:
  %345 = load i16, ptr %92, !tbaa !2
  %346 = getelementptr i8, ptr %274, i16 -4
  %347 = load i16, ptr %346
  %348 = icmp ult i16 %345, %347
  %349 = sext i1 %348 to i8
  %350 = icmp ne i8 %349, 0
  br i1 %350, label %b54, label %b55

b52:
  store i16 0, ptr %87, !tbaa !2
  br label %b53

b53:
  %351 = load i16, ptr %87, !tbaa !2
  %352 = add i16 %351, 1
  %353 = getelementptr i8, ptr %273, i16 4
  store i16 %352, ptr %353
  %354 = load ptr, ptr %103, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %354)
  br label %b4

b54:
  %355 = mul i16 %345, 6
  %356 = getelementptr i8, ptr %274, i16 %355
  %357 = getelementptr i8, ptr %356, i16 4
  %358 = load i16, ptr %357
  store i16 %358, ptr %87, !tbaa !2
  br label %b53

b55:
  call addrspace(1) void @N$EBND()
  unreachable

b56:
  %359 = getelementptr i8, ptr %159, i16 -4
  %360 = load i16, ptr %359
  %361 = sub i16 %360, 1
  store i16 %361, ptr %81, !tbaa !2
  %362 = load i16, ptr %84, !tbaa !2
  %363 = load i16, ptr %81, !tbaa !2
  %364 = and i16 %362, %363
  store i16 %364, ptr %83, !tbaa !2
  br label %b59

b57:
  br label %b58

b58:
  %365 = load i8, ptr %82, !tbaa !2
  %366 = icmp ne i8 %365, 0
  br i1 %366, label %b73, label %b74

b59:
  %367 = load i16, ptr %83, !tbaa !2
  %368 = getelementptr i8, ptr %159, i16 -4
  %369 = load i16, ptr %368
  %370 = icmp ult i16 %367, %369
  %371 = sext i1 %370 to i8
  %372 = icmp ne i8 %371, 0
  br i1 %372, label %b62, label %b63

b60:
  %373 = load i16, ptr %83, !tbaa !2
  %374 = getelementptr i8, ptr %159, i16 -4
  %375 = load i16, ptr %374
  %376 = icmp ult i16 %373, %375
  %377 = sext i1 %376 to i8
  %378 = icmp ne i8 %377, 0
  br i1 %378, label %b64, label %b65

b61:
  br label %b58

b62:
  %379 = mul i16 %367, 6
  %380 = getelementptr i8, ptr %159, i16 %379
  %381 = load i16, ptr %380
  %382 = icmp ne i16 %381, 0
  %383 = sext i1 %382 to i8
  %384 = icmp ne i8 %383, 0
  br i1 %384, label %b60, label %b61

b63:
  call addrspace(1) void @N$EBND()
  unreachable

b64:
  %385 = mul i16 %373, 6
  %386 = getelementptr i8, ptr %159, i16 %385
  %387 = load i16, ptr %386
  %388 = load i16, ptr %84, !tbaa !2
  %389 = icmp eq i16 %387, %388
  %390 = sext i1 %389 to i8
  store i8 %390, ptr %80, !tbaa !2
  %391 = icmp ne i8 %390, 0
  br i1 %391, label %b66, label %b67

b65:
  call addrspace(1) void @N$EBND()
  unreachable

b66:
  %392 = load i16, ptr %83, !tbaa !2
  %393 = getelementptr i8, ptr %159, i16 -4
  %394 = load i16, ptr %393
  %395 = icmp ult i16 %392, %394
  %396 = sext i1 %395 to i8
  %397 = icmp ne i8 %396, 0
  br i1 %397, label %b68, label %b69

b67:
  %398 = load i8, ptr %80, !tbaa !2
  %399 = icmp ne i8 %398, 0
  br i1 %399, label %b70, label %b71

b68:
  %400 = mul i16 %392, 6
  %401 = getelementptr i8, ptr %159, i16 %400
  %402 = getelementptr i8, ptr %401, i16 2
  %403 = load ptr, ptr %402
  %404 = getelementptr i8, ptr %403, i16 -4
  %405 = load i16, ptr %404
  %406 = addrspacecast ptr %403 to ptr addrspace(1)
  store i16 %405, ptr %79, !tbaa !2
  %407 = getelementptr inbounds i8, ptr %79, i16 2
  store i16 %405, ptr %407, !tbaa !2
  %408 = getelementptr inbounds i8, ptr %79, i16 4
  store ptr addrspace(1) %406, ptr %408, !tbaa !2
  %409 = addrspacecast ptr %79 to ptr addrspace(1)
  %410 = load ptr, ptr %86, !tbaa !2
  %411 = getelementptr i8, ptr %410, i16 -4
  %412 = load i16, ptr %411
  %413 = addrspacecast ptr %410 to ptr addrspace(1)
  store i16 %412, ptr %78, !tbaa !2
  %414 = getelementptr inbounds i8, ptr %78, i16 2
  store i16 %412, ptr %414, !tbaa !2
  %415 = getelementptr inbounds i8, ptr %78, i16 4
  store ptr addrspace(1) %413, ptr %415, !tbaa !2
  %416 = addrspacecast ptr %78 to ptr addrspace(1)
  %417 = call addrspace(1) i8 @string.eq(ptr addrspace(1) %409, ptr addrspace(1) %416)
  store i8 %417, ptr %80, !tbaa !2
  br label %b67

b69:
  call addrspace(1) void @N$EBND()
  unreachable

b70:
  store i8 -1, ptr %82, !tbaa !2
  br label %b61

b71:
  br label %b72

b72:
  %418 = load i16, ptr %83, !tbaa !2
  %419 = add i16 %418, 1
  %420 = load i16, ptr %81, !tbaa !2
  %421 = and i16 %419, %420
  store i16 %421, ptr %83, !tbaa !2
  br label %b59

b73:
  %422 = load i16, ptr %83, !tbaa !2
  %423 = getelementptr i8, ptr %159, i16 -4
  %424 = load i16, ptr %423
  %425 = icmp ult i16 %422, %424
  %426 = sext i1 %425 to i8
  %427 = icmp ne i8 %426, 0
  br i1 %427, label %b75, label %b76

b74:
  call addrspace(1) void @N$EKEY()
  unreachable

b75:
  %428 = mul i16 %422, 6
  %429 = getelementptr i8, ptr %159, i16 %428
  %430 = getelementptr i8, ptr %429, i16 4
  %431 = load i16, ptr %430
  store i16 %431, ptr %77, !tbaa !2
  %432 = load ptr, ptr %105, !tbaa !2
  %433 = getelementptr i8, ptr %432, i16 -2
  %434 = load i16, ptr %433
  store i16 %434, ptr %76, !tbaa !2
  %435 = load i16, ptr %77, !tbaa !2
  store i16 %435, ptr %75, !tbaa !2
  %436 = load ptr, ptr %105, !tbaa !2
  %437 = getelementptr i8, ptr @$str8, i16 6
  store ptr %437, ptr %74, !tbaa !2
  %438 = load ptr, ptr %74, !tbaa !2
  %439 = getelementptr i8, ptr %438, i16 -4
  %440 = load i16, ptr %439
  %441 = addrspacecast ptr %438 to ptr addrspace(1)
  store i16 %440, ptr %73, !tbaa !2
  %442 = getelementptr inbounds i8, ptr %73, i16 2
  store i16 %440, ptr %442, !tbaa !2
  %443 = getelementptr inbounds i8, ptr %73, i16 4
  store ptr addrspace(1) %441, ptr %443, !tbaa !2
  %444 = addrspacecast ptr %73 to ptr addrspace(1)
  %445 = call addrspace(1) i16 @string.hash(ptr addrspace(1) %444)
  %446 = or i16 %445, 1
  store i16 %446, ptr %72, !tbaa !2
  store i16 0, ptr %71, !tbaa !2
  store i8 0, ptr %70, !tbaa !2
  %447 = getelementptr i8, ptr %436, i16 -4
  %448 = load i16, ptr %447
  %449 = icmp ne i16 %448, 0
  %450 = sext i1 %449 to i8
  %451 = icmp ne i8 %450, 0
  br i1 %451, label %b77, label %b78

b76:
  call addrspace(1) void @N$EBND()
  unreachable

b77:
  %452 = getelementptr i8, ptr %436, i16 -4
  %453 = load i16, ptr %452
  %454 = sub i16 %453, 1
  store i16 %454, ptr %69, !tbaa !2
  %455 = load i16, ptr %72, !tbaa !2
  %456 = load i16, ptr %69, !tbaa !2
  %457 = and i16 %455, %456
  store i16 %457, ptr %71, !tbaa !2
  br label %b80

b78:
  br label %b79

b79:
  %458 = load i8, ptr %70, !tbaa !2
  %459 = icmp ne i8 %458, 0
  br i1 %459, label %b94, label %b95

b80:
  %460 = load i16, ptr %71, !tbaa !2
  %461 = getelementptr i8, ptr %436, i16 -4
  %462 = load i16, ptr %461
  %463 = icmp ult i16 %460, %462
  %464 = sext i1 %463 to i8
  %465 = icmp ne i8 %464, 0
  br i1 %465, label %b83, label %b84

b81:
  %466 = load i16, ptr %71, !tbaa !2
  %467 = getelementptr i8, ptr %436, i16 -4
  %468 = load i16, ptr %467
  %469 = icmp ult i16 %466, %468
  %470 = sext i1 %469 to i8
  %471 = icmp ne i8 %470, 0
  br i1 %471, label %b85, label %b86

b82:
  br label %b79

b83:
  %472 = mul i16 %460, 6
  %473 = getelementptr i8, ptr %436, i16 %472
  %474 = load i16, ptr %473
  %475 = icmp ne i16 %474, 0
  %476 = sext i1 %475 to i8
  %477 = icmp ne i8 %476, 0
  br i1 %477, label %b81, label %b82

b84:
  call addrspace(1) void @N$EBND()
  unreachable

b85:
  %478 = mul i16 %466, 6
  %479 = getelementptr i8, ptr %436, i16 %478
  %480 = load i16, ptr %479
  %481 = load i16, ptr %72, !tbaa !2
  %482 = icmp eq i16 %480, %481
  %483 = sext i1 %482 to i8
  store i8 %483, ptr %68, !tbaa !2
  %484 = icmp ne i8 %483, 0
  br i1 %484, label %b87, label %b88

b86:
  call addrspace(1) void @N$EBND()
  unreachable

b87:
  %485 = load i16, ptr %71, !tbaa !2
  %486 = getelementptr i8, ptr %436, i16 -4
  %487 = load i16, ptr %486
  %488 = icmp ult i16 %485, %487
  %489 = sext i1 %488 to i8
  %490 = icmp ne i8 %489, 0
  br i1 %490, label %b89, label %b90

b88:
  %491 = load i8, ptr %68, !tbaa !2
  %492 = icmp ne i8 %491, 0
  br i1 %492, label %b91, label %b92

b89:
  %493 = mul i16 %485, 6
  %494 = getelementptr i8, ptr %436, i16 %493
  %495 = getelementptr i8, ptr %494, i16 2
  %496 = load ptr, ptr %495
  %497 = getelementptr i8, ptr %496, i16 -4
  %498 = load i16, ptr %497
  %499 = addrspacecast ptr %496 to ptr addrspace(1)
  store i16 %498, ptr %67, !tbaa !2
  %500 = getelementptr inbounds i8, ptr %67, i16 2
  store i16 %498, ptr %500, !tbaa !2
  %501 = getelementptr inbounds i8, ptr %67, i16 4
  store ptr addrspace(1) %499, ptr %501, !tbaa !2
  %502 = addrspacecast ptr %67 to ptr addrspace(1)
  %503 = load ptr, ptr %74, !tbaa !2
  %504 = getelementptr i8, ptr %503, i16 -4
  %505 = load i16, ptr %504
  %506 = addrspacecast ptr %503 to ptr addrspace(1)
  store i16 %505, ptr %66, !tbaa !2
  %507 = getelementptr inbounds i8, ptr %66, i16 2
  store i16 %505, ptr %507, !tbaa !2
  %508 = getelementptr inbounds i8, ptr %66, i16 4
  store ptr addrspace(1) %506, ptr %508, !tbaa !2
  %509 = addrspacecast ptr %66 to ptr addrspace(1)
  %510 = call addrspace(1) i8 @string.eq(ptr addrspace(1) %502, ptr addrspace(1) %509)
  store i8 %510, ptr %68, !tbaa !2
  br label %b88

b90:
  call addrspace(1) void @N$EBND()
  unreachable

b91:
  store i8 -1, ptr %70, !tbaa !2
  br label %b82

b92:
  br label %b93

b93:
  %511 = load i16, ptr %71, !tbaa !2
  %512 = add i16 %511, 1
  %513 = load i16, ptr %69, !tbaa !2
  %514 = and i16 %512, %513
  store i16 %514, ptr %71, !tbaa !2
  br label %b80

b94:
  %515 = load i16, ptr %71, !tbaa !2
  %516 = getelementptr i8, ptr %436, i16 -4
  %517 = load i16, ptr %516
  %518 = icmp ult i16 %515, %517
  %519 = sext i1 %518 to i8
  %520 = icmp ne i8 %519, 0
  br i1 %520, label %b97, label %b98

b95:
  store i16 0, ptr %65, !tbaa !2
  br label %b96

b96:
  %521 = load i16, ptr %65, !tbaa !2
  store i16 %521, ptr %64, !tbaa !2
  %522 = load i16, ptr %76, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %522)
  %523 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %523)
  %524 = load i16, ptr %75, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %524)
  %525 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %525)
  %526 = load i16, ptr %64, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %526)
  call addrspace(1) void @N$PN()
  %527 = getelementptr i8, ptr @$str1, i16 6
  %528 = call addrspace(1) ptr @N$BGRW(ptr %527, i16 3, i16 2)
  %529 = getelementptr i8, ptr %528, i16 0
  store i8 12, ptr %529
  %530 = getelementptr i8, ptr %529, i16 1
  store i8 0, ptr %530
  %531 = getelementptr i8, ptr %528, i16 2
  store i8 3, ptr %531
  %532 = getelementptr i8, ptr %531, i16 1
  store i8 1, ptr %532
  %533 = getelementptr i8, ptr %528, i16 4
  store i8 12, ptr %533
  %534 = getelementptr i8, ptr %533, i16 1
  store i8 0, ptr %534
  store ptr %528, ptr %63, !tbaa !2
  %535 = getelementptr i8, ptr @$str1, i16 6
  store ptr %535, ptr %62, !tbaa !2
  %536 = load ptr, ptr %62, !tbaa !2
  %537 = call addrspace(1) ptr @N$DRES(ptr %536, i16 6)
  store ptr %537, ptr %62, !tbaa !2
  store i8 0, ptr %61, !tbaa !2
  %538 = load i8, ptr %61, !tbaa !2
  %539 = call addrspace(1) i16 @Suit.hash(i8 %538)
  %540 = or i16 %539, 1
  store i16 %540, ptr %60, !tbaa !2
  store i16 0, ptr %59, !tbaa !2
  store i8 0, ptr %58, !tbaa !2
  %541 = getelementptr i8, ptr %537, i16 -4
  %542 = load i16, ptr %541
  %543 = icmp ne i16 %542, 0
  %544 = sext i1 %543 to i8
  %545 = icmp ne i8 %544, 0
  br i1 %545, label %b99, label %b100

b97:
  %546 = mul i16 %515, 6
  %547 = getelementptr i8, ptr %436, i16 %546
  %548 = getelementptr i8, ptr %547, i16 4
  %549 = load i16, ptr %548
  store i16 %549, ptr %65, !tbaa !2
  br label %b96

b98:
  call addrspace(1) void @N$EBND()
  unreachable

b99:
  %550 = getelementptr i8, ptr %537, i16 -4
  %551 = load i16, ptr %550
  %552 = sub i16 %551, 1
  store i16 %552, ptr %57, !tbaa !2
  %553 = load i16, ptr %60, !tbaa !2
  %554 = load i16, ptr %57, !tbaa !2
  %555 = and i16 %553, %554
  store i16 %555, ptr %59, !tbaa !2
  br label %b102

b100:
  br label %b101

b101:
  %556 = load i8, ptr %58, !tbaa !2
  %557 = xor i8 %556, -1
  %558 = icmp ne i8 %557, 0
  br i1 %558, label %b116, label %b117

b102:
  %559 = load i16, ptr %59, !tbaa !2
  %560 = getelementptr i8, ptr %537, i16 -4
  %561 = load i16, ptr %560
  %562 = icmp ult i16 %559, %561
  %563 = sext i1 %562 to i8
  %564 = icmp ne i8 %563, 0
  br i1 %564, label %b105, label %b106

b103:
  %565 = load i16, ptr %59, !tbaa !2
  %566 = getelementptr i8, ptr %537, i16 -4
  %567 = load i16, ptr %566
  %568 = icmp ult i16 %565, %567
  %569 = sext i1 %568 to i8
  %570 = icmp ne i8 %569, 0
  br i1 %570, label %b107, label %b108

b104:
  br label %b101

b105:
  %571 = mul i16 %559, 6
  %572 = getelementptr i8, ptr %537, i16 %571
  %573 = load i16, ptr %572
  %574 = icmp ne i16 %573, 0
  %575 = sext i1 %574 to i8
  %576 = icmp ne i8 %575, 0
  br i1 %576, label %b103, label %b104

b106:
  call addrspace(1) void @N$EBND()
  unreachable

b107:
  %577 = mul i16 %565, 6
  %578 = getelementptr i8, ptr %537, i16 %577
  %579 = load i16, ptr %578
  %580 = load i16, ptr %60, !tbaa !2
  %581 = icmp eq i16 %579, %580
  %582 = sext i1 %581 to i8
  store i8 %582, ptr %56, !tbaa !2
  %583 = icmp ne i8 %582, 0
  br i1 %583, label %b109, label %b110

b108:
  call addrspace(1) void @N$EBND()
  unreachable

b109:
  %584 = load i16, ptr %59, !tbaa !2
  %585 = getelementptr i8, ptr %537, i16 -4
  %586 = load i16, ptr %585
  %587 = icmp ult i16 %584, %586
  %588 = sext i1 %587 to i8
  %589 = icmp ne i8 %588, 0
  br i1 %589, label %b111, label %b112

b110:
  %590 = load i8, ptr %56, !tbaa !2
  %591 = icmp ne i8 %590, 0
  br i1 %591, label %b113, label %b114

b111:
  %592 = mul i16 %584, 6
  %593 = getelementptr i8, ptr %537, i16 %592
  %594 = getelementptr i8, ptr %593, i16 2
  %595 = load i8, ptr %594
  %596 = load i8, ptr %61, !tbaa !2
  %597 = call addrspace(1) i8 @Suit.eq(i8 %595, i8 %596)
  store i8 %597, ptr %56, !tbaa !2
  br label %b110

b112:
  call addrspace(1) void @N$EBND()
  unreachable

b113:
  store i8 -1, ptr %58, !tbaa !2
  br label %b104

b114:
  br label %b115

b115:
  %598 = load i16, ptr %59, !tbaa !2
  %599 = add i16 %598, 1
  %600 = load i16, ptr %57, !tbaa !2
  %601 = and i16 %599, %600
  store i16 %601, ptr %59, !tbaa !2
  br label %b102

b116:
  %602 = load i16, ptr %59, !tbaa !2
  %603 = getelementptr i8, ptr %537, i16 -4
  %604 = load i16, ptr %603
  %605 = icmp ult i16 %602, %604
  %606 = sext i1 %605 to i8
  %607 = icmp ne i8 %606, 0
  br i1 %607, label %b119, label %b120

b117:
  br label %b118

b118:
  %608 = load i8, ptr %58, !tbaa !2
  %609 = icmp ne i8 %608, 0
  br i1 %609, label %b124, label %b123

b119:
  %610 = mul i16 %602, 6
  %611 = getelementptr i8, ptr %537, i16 %610
  %612 = load i16, ptr %60, !tbaa !2
  store i16 %612, ptr %611
  %613 = load i16, ptr %59, !tbaa !2
  %614 = getelementptr i8, ptr %537, i16 -4
  %615 = load i16, ptr %614
  %616 = icmp ult i16 %613, %615
  %617 = sext i1 %616 to i8
  %618 = icmp ne i8 %617, 0
  br i1 %618, label %b121, label %b122

b120:
  call addrspace(1) void @N$EBND()
  unreachable

b121:
  %619 = mul i16 %613, 6
  %620 = getelementptr i8, ptr %537, i16 %619
  %621 = load i8, ptr %61, !tbaa !2
  %622 = getelementptr i8, ptr %620, i16 2
  store i8 %621, ptr %622
  br label %b118

b122:
  call addrspace(1) void @N$EBND()
  unreachable

b123:
  %623 = getelementptr i8, ptr %537, i16 -2
  %624 = load i16, ptr %623
  %625 = add i16 %624, 1
  %626 = getelementptr i8, ptr %537, i16 -2
  store i16 %625, ptr %626
  br label %b124

b124:
  %627 = load i16, ptr %59, !tbaa !2
  %628 = getelementptr i8, ptr %537, i16 -4
  %629 = load i16, ptr %628
  %630 = icmp ult i16 %627, %629
  %631 = sext i1 %630 to i8
  %632 = icmp ne i8 %631, 0
  br i1 %632, label %b125, label %b126

b125:
  %633 = mul i16 %627, 6
  %634 = getelementptr i8, ptr %537, i16 %633
  %635 = getelementptr i8, ptr %634, i16 4
  store i16 0, ptr %635
  %636 = load ptr, ptr %62, !tbaa !2
  %637 = call addrspace(1) ptr @N$DRES(ptr %636, i16 6)
  store ptr %637, ptr %62, !tbaa !2
  store i8 1, ptr %55, !tbaa !2
  %638 = load i8, ptr %55, !tbaa !2
  %639 = call addrspace(1) i16 @Suit.hash(i8 %638)
  %640 = or i16 %639, 1
  store i16 %640, ptr %54, !tbaa !2
  store i16 0, ptr %53, !tbaa !2
  store i8 0, ptr %52, !tbaa !2
  %641 = getelementptr i8, ptr %637, i16 -4
  %642 = load i16, ptr %641
  %643 = icmp ne i16 %642, 0
  %644 = sext i1 %643 to i8
  %645 = icmp ne i8 %644, 0
  br i1 %645, label %b127, label %b128

b126:
  call addrspace(1) void @N$EBND()
  unreachable

b127:
  %646 = getelementptr i8, ptr %637, i16 -4
  %647 = load i16, ptr %646
  %648 = sub i16 %647, 1
  store i16 %648, ptr %51, !tbaa !2
  %649 = load i16, ptr %54, !tbaa !2
  %650 = load i16, ptr %51, !tbaa !2
  %651 = and i16 %649, %650
  store i16 %651, ptr %53, !tbaa !2
  br label %b130

b128:
  br label %b129

b129:
  %652 = load i8, ptr %52, !tbaa !2
  %653 = xor i8 %652, -1
  %654 = icmp ne i8 %653, 0
  br i1 %654, label %b144, label %b145

b130:
  %655 = load i16, ptr %53, !tbaa !2
  %656 = getelementptr i8, ptr %637, i16 -4
  %657 = load i16, ptr %656
  %658 = icmp ult i16 %655, %657
  %659 = sext i1 %658 to i8
  %660 = icmp ne i8 %659, 0
  br i1 %660, label %b133, label %b134

b131:
  %661 = load i16, ptr %53, !tbaa !2
  %662 = getelementptr i8, ptr %637, i16 -4
  %663 = load i16, ptr %662
  %664 = icmp ult i16 %661, %663
  %665 = sext i1 %664 to i8
  %666 = icmp ne i8 %665, 0
  br i1 %666, label %b135, label %b136

b132:
  br label %b129

b133:
  %667 = mul i16 %655, 6
  %668 = getelementptr i8, ptr %637, i16 %667
  %669 = load i16, ptr %668
  %670 = icmp ne i16 %669, 0
  %671 = sext i1 %670 to i8
  %672 = icmp ne i8 %671, 0
  br i1 %672, label %b131, label %b132

b134:
  call addrspace(1) void @N$EBND()
  unreachable

b135:
  %673 = mul i16 %661, 6
  %674 = getelementptr i8, ptr %637, i16 %673
  %675 = load i16, ptr %674
  %676 = load i16, ptr %54, !tbaa !2
  %677 = icmp eq i16 %675, %676
  %678 = sext i1 %677 to i8
  store i8 %678, ptr %50, !tbaa !2
  %679 = icmp ne i8 %678, 0
  br i1 %679, label %b137, label %b138

b136:
  call addrspace(1) void @N$EBND()
  unreachable

b137:
  %680 = load i16, ptr %53, !tbaa !2
  %681 = getelementptr i8, ptr %637, i16 -4
  %682 = load i16, ptr %681
  %683 = icmp ult i16 %680, %682
  %684 = sext i1 %683 to i8
  %685 = icmp ne i8 %684, 0
  br i1 %685, label %b139, label %b140

b138:
  %686 = load i8, ptr %50, !tbaa !2
  %687 = icmp ne i8 %686, 0
  br i1 %687, label %b141, label %b142

b139:
  %688 = mul i16 %680, 6
  %689 = getelementptr i8, ptr %637, i16 %688
  %690 = getelementptr i8, ptr %689, i16 2
  %691 = load i8, ptr %690
  %692 = load i8, ptr %55, !tbaa !2
  %693 = call addrspace(1) i8 @Suit.eq(i8 %691, i8 %692)
  store i8 %693, ptr %50, !tbaa !2
  br label %b138

b140:
  call addrspace(1) void @N$EBND()
  unreachable

b141:
  store i8 -1, ptr %52, !tbaa !2
  br label %b132

b142:
  br label %b143

b143:
  %694 = load i16, ptr %53, !tbaa !2
  %695 = add i16 %694, 1
  %696 = load i16, ptr %51, !tbaa !2
  %697 = and i16 %695, %696
  store i16 %697, ptr %53, !tbaa !2
  br label %b130

b144:
  %698 = load i16, ptr %53, !tbaa !2
  %699 = getelementptr i8, ptr %637, i16 -4
  %700 = load i16, ptr %699
  %701 = icmp ult i16 %698, %700
  %702 = sext i1 %701 to i8
  %703 = icmp ne i8 %702, 0
  br i1 %703, label %b147, label %b148

b145:
  br label %b146

b146:
  %704 = load i8, ptr %52, !tbaa !2
  %705 = icmp ne i8 %704, 0
  br i1 %705, label %b152, label %b151

b147:
  %706 = mul i16 %698, 6
  %707 = getelementptr i8, ptr %637, i16 %706
  %708 = load i16, ptr %54, !tbaa !2
  store i16 %708, ptr %707
  %709 = load i16, ptr %53, !tbaa !2
  %710 = getelementptr i8, ptr %637, i16 -4
  %711 = load i16, ptr %710
  %712 = icmp ult i16 %709, %711
  %713 = sext i1 %712 to i8
  %714 = icmp ne i8 %713, 0
  br i1 %714, label %b149, label %b150

b148:
  call addrspace(1) void @N$EBND()
  unreachable

b149:
  %715 = mul i16 %709, 6
  %716 = getelementptr i8, ptr %637, i16 %715
  %717 = load i8, ptr %55, !tbaa !2
  %718 = getelementptr i8, ptr %716, i16 2
  store i8 %717, ptr %718
  br label %b146

b150:
  call addrspace(1) void @N$EBND()
  unreachable

b151:
  %719 = getelementptr i8, ptr %637, i16 -2
  %720 = load i16, ptr %719
  %721 = add i16 %720, 1
  %722 = getelementptr i8, ptr %637, i16 -2
  store i16 %721, ptr %722
  br label %b152

b152:
  %723 = load i16, ptr %53, !tbaa !2
  %724 = getelementptr i8, ptr %637, i16 -4
  %725 = load i16, ptr %724
  %726 = icmp ult i16 %723, %725
  %727 = sext i1 %726 to i8
  %728 = icmp ne i8 %727, 0
  br i1 %728, label %b153, label %b154

b153:
  %729 = mul i16 %723, 6
  %730 = getelementptr i8, ptr %637, i16 %729
  %731 = getelementptr i8, ptr %730, i16 4
  store i16 0, ptr %731
  %732 = load ptr, ptr %62, !tbaa !2
  store ptr null, ptr %62, !tbaa !2
  store ptr %732, ptr %49, !tbaa !2
  %733 = getelementptr i8, ptr @$str1, i16 6
  store ptr %733, ptr %48, !tbaa !2
  %734 = load ptr, ptr %48, !tbaa !2
  store ptr null, ptr %48, !tbaa !2
  store ptr %734, ptr %47, !tbaa !2
  %735 = load ptr, ptr %63, !tbaa !2
  %736 = getelementptr i8, ptr %735, i16 -4
  %737 = load i16, ptr %736
  store i16 0, ptr %46, !tbaa !2
  br label %b155

b154:
  call addrspace(1) void @N$EBND()
  unreachable

b155:
  %738 = load i16, ptr %46, !tbaa !2
  %739 = icmp ult i16 %738, %737
  %740 = sext i1 %739 to i8
  %741 = icmp ne i8 %740, 0
  br i1 %741, label %b156, label %b158

b156:
  %742 = mul i16 %738, 2
  %743 = getelementptr i8, ptr %735, i16 %742
  %744 = load ptr, ptr %49, !tbaa !2
  %745 = call addrspace(1) ptr @N$DRES(ptr %744, i16 6)
  store ptr %745, ptr %49, !tbaa !2
  %746 = getelementptr i8, ptr %743, i16 1
  %747 = load i8, ptr %746
  store i8 %747, ptr %45, !tbaa !2
  %748 = load i8, ptr %45, !tbaa !2
  %749 = call addrspace(1) i16 @Suit.hash(i8 %748)
  %750 = or i16 %749, 1
  store i16 %750, ptr %44, !tbaa !2
  store i16 0, ptr %43, !tbaa !2
  store i8 0, ptr %42, !tbaa !2
  %751 = getelementptr i8, ptr %745, i16 -4
  %752 = load i16, ptr %751
  %753 = icmp ne i16 %752, 0
  %754 = sext i1 %753 to i8
  %755 = icmp ne i8 %754, 0
  br i1 %755, label %b159, label %b160

b157:
  %756 = load i16, ptr %46, !tbaa !2
  %757 = add i16 %756, 1
  store i16 %757, ptr %46, !tbaa !2
  br label %b155

b158:
  %758 = load ptr, ptr %49, !tbaa !2
  store i8 0, ptr %33, !tbaa !2
  %759 = load i8, ptr %33, !tbaa !2
  %760 = call addrspace(1) i16 @Suit.hash(i8 %759)
  %761 = or i16 %760, 1
  store i16 %761, ptr %32, !tbaa !2
  store i16 0, ptr %31, !tbaa !2
  store i8 0, ptr %30, !tbaa !2
  %762 = getelementptr i8, ptr %758, i16 -4
  %763 = load i16, ptr %762
  %764 = icmp ne i16 %763, 0
  %765 = sext i1 %764 to i8
  %766 = icmp ne i8 %765, 0
  br i1 %766, label %b215, label %b216

b159:
  %767 = getelementptr i8, ptr %745, i16 -4
  %768 = load i16, ptr %767
  %769 = sub i16 %768, 1
  store i16 %769, ptr %41, !tbaa !2
  %770 = load i16, ptr %44, !tbaa !2
  %771 = load i16, ptr %41, !tbaa !2
  %772 = and i16 %770, %771
  store i16 %772, ptr %43, !tbaa !2
  br label %b162

b160:
  br label %b161

b161:
  %773 = load i8, ptr %42, !tbaa !2
  %774 = xor i8 %773, -1
  %775 = icmp ne i8 %774, 0
  br i1 %775, label %b176, label %b177

b162:
  %776 = load i16, ptr %43, !tbaa !2
  %777 = getelementptr i8, ptr %745, i16 -4
  %778 = load i16, ptr %777
  %779 = icmp ult i16 %776, %778
  %780 = sext i1 %779 to i8
  %781 = icmp ne i8 %780, 0
  br i1 %781, label %b165, label %b166

b163:
  %782 = load i16, ptr %43, !tbaa !2
  %783 = getelementptr i8, ptr %745, i16 -4
  %784 = load i16, ptr %783
  %785 = icmp ult i16 %782, %784
  %786 = sext i1 %785 to i8
  %787 = icmp ne i8 %786, 0
  br i1 %787, label %b167, label %b168

b164:
  br label %b161

b165:
  %788 = mul i16 %776, 6
  %789 = getelementptr i8, ptr %745, i16 %788
  %790 = load i16, ptr %789
  %791 = icmp ne i16 %790, 0
  %792 = sext i1 %791 to i8
  %793 = icmp ne i8 %792, 0
  br i1 %793, label %b163, label %b164

b166:
  call addrspace(1) void @N$EBND()
  unreachable

b167:
  %794 = mul i16 %782, 6
  %795 = getelementptr i8, ptr %745, i16 %794
  %796 = load i16, ptr %795
  %797 = load i16, ptr %44, !tbaa !2
  %798 = icmp eq i16 %796, %797
  %799 = sext i1 %798 to i8
  store i8 %799, ptr %40, !tbaa !2
  %800 = icmp ne i8 %799, 0
  br i1 %800, label %b169, label %b170

b168:
  call addrspace(1) void @N$EBND()
  unreachable

b169:
  %801 = load i16, ptr %43, !tbaa !2
  %802 = getelementptr i8, ptr %745, i16 -4
  %803 = load i16, ptr %802
  %804 = icmp ult i16 %801, %803
  %805 = sext i1 %804 to i8
  %806 = icmp ne i8 %805, 0
  br i1 %806, label %b171, label %b172

b170:
  %807 = load i8, ptr %40, !tbaa !2
  %808 = icmp ne i8 %807, 0
  br i1 %808, label %b173, label %b174

b171:
  %809 = mul i16 %801, 6
  %810 = getelementptr i8, ptr %745, i16 %809
  %811 = getelementptr i8, ptr %810, i16 2
  %812 = load i8, ptr %811
  %813 = load i8, ptr %45, !tbaa !2
  %814 = call addrspace(1) i8 @Suit.eq(i8 %812, i8 %813)
  store i8 %814, ptr %40, !tbaa !2
  br label %b170

b172:
  call addrspace(1) void @N$EBND()
  unreachable

b173:
  store i8 -1, ptr %42, !tbaa !2
  br label %b164

b174:
  br label %b175

b175:
  %815 = load i16, ptr %43, !tbaa !2
  %816 = add i16 %815, 1
  %817 = load i16, ptr %41, !tbaa !2
  %818 = and i16 %816, %817
  store i16 %818, ptr %43, !tbaa !2
  br label %b162

b176:
  %819 = load i16, ptr %43, !tbaa !2
  %820 = getelementptr i8, ptr %745, i16 -4
  %821 = load i16, ptr %820
  %822 = icmp ult i16 %819, %821
  %823 = sext i1 %822 to i8
  %824 = icmp ne i8 %823, 0
  br i1 %824, label %b179, label %b180

b177:
  br label %b178

b178:
  %825 = load i8, ptr %42, !tbaa !2
  %826 = icmp ne i8 %825, 0
  br i1 %826, label %b184, label %b183

b179:
  %827 = mul i16 %819, 6
  %828 = getelementptr i8, ptr %745, i16 %827
  %829 = load i16, ptr %44, !tbaa !2
  store i16 %829, ptr %828
  %830 = load i16, ptr %43, !tbaa !2
  %831 = getelementptr i8, ptr %745, i16 -4
  %832 = load i16, ptr %831
  %833 = icmp ult i16 %830, %832
  %834 = sext i1 %833 to i8
  %835 = icmp ne i8 %834, 0
  br i1 %835, label %b181, label %b182

b180:
  call addrspace(1) void @N$EBND()
  unreachable

b181:
  %836 = mul i16 %830, 6
  %837 = getelementptr i8, ptr %745, i16 %836
  %838 = load i8, ptr %45, !tbaa !2
  %839 = getelementptr i8, ptr %837, i16 2
  store i8 %838, ptr %839
  br label %b178

b182:
  call addrspace(1) void @N$EBND()
  unreachable

b183:
  %840 = getelementptr i8, ptr %745, i16 -2
  %841 = load i16, ptr %840
  %842 = add i16 %841, 1
  %843 = getelementptr i8, ptr %745, i16 -2
  store i16 %842, ptr %843
  br label %b184

b184:
  %844 = load i16, ptr %43, !tbaa !2
  %845 = getelementptr i8, ptr %745, i16 -4
  %846 = load i16, ptr %845
  %847 = icmp ult i16 %844, %846
  %848 = sext i1 %847 to i8
  %849 = icmp ne i8 %848, 0
  br i1 %849, label %b185, label %b186

b185:
  %850 = mul i16 %844, 6
  %851 = getelementptr i8, ptr %745, i16 %850
  %852 = getelementptr i8, ptr %851, i16 4
  %853 = load i16, ptr %852
  %854 = add i16 %853, 1
  %855 = getelementptr i8, ptr %851, i16 4
  store i16 %854, ptr %855
  %856 = load ptr, ptr %47, !tbaa !2
  %857 = call addrspace(1) ptr @N$DRES(ptr %856, i16 6)
  store ptr %857, ptr %47, !tbaa !2
  %858 = load i8, ptr %743
  %859 = getelementptr i8, ptr %743, i16 1
  %860 = load i8, ptr %859
  store i8 %858, ptr %39, !tbaa !2
  %861 = getelementptr inbounds i8, ptr %39, i16 1
  store i8 %860, ptr %861, !tbaa !2
  %862 = addrspacecast ptr %39 to ptr addrspace(1)
  %863 = call addrspace(1) i16 @Card.hash(ptr addrspace(1) %862)
  %864 = or i16 %863, 1
  store i16 %864, ptr %38, !tbaa !2
  store i16 0, ptr %37, !tbaa !2
  store i8 0, ptr %36, !tbaa !2
  %865 = getelementptr i8, ptr %857, i16 -4
  %866 = load i16, ptr %865
  %867 = icmp ne i16 %866, 0
  %868 = sext i1 %867 to i8
  %869 = icmp ne i8 %868, 0
  br i1 %869, label %b187, label %b188

b186:
  call addrspace(1) void @N$EBND()
  unreachable

b187:
  %870 = getelementptr i8, ptr %857, i16 -4
  %871 = load i16, ptr %870
  %872 = sub i16 %871, 1
  store i16 %872, ptr %35, !tbaa !2
  %873 = load i16, ptr %38, !tbaa !2
  %874 = load i16, ptr %35, !tbaa !2
  %875 = and i16 %873, %874
  store i16 %875, ptr %37, !tbaa !2
  br label %b190

b188:
  br label %b189

b189:
  %876 = load i8, ptr %36, !tbaa !2
  %877 = xor i8 %876, -1
  %878 = icmp ne i8 %877, 0
  br i1 %878, label %b204, label %b205

b190:
  %879 = load i16, ptr %37, !tbaa !2
  %880 = getelementptr i8, ptr %857, i16 -4
  %881 = load i16, ptr %880
  %882 = icmp ult i16 %879, %881
  %883 = sext i1 %882 to i8
  %884 = icmp ne i8 %883, 0
  br i1 %884, label %b193, label %b194

b191:
  %885 = load i16, ptr %37, !tbaa !2
  %886 = getelementptr i8, ptr %857, i16 -4
  %887 = load i16, ptr %886
  %888 = icmp ult i16 %885, %887
  %889 = sext i1 %888 to i8
  %890 = icmp ne i8 %889, 0
  br i1 %890, label %b195, label %b196

b192:
  br label %b189

b193:
  %891 = mul i16 %879, 6
  %892 = getelementptr i8, ptr %857, i16 %891
  %893 = load i16, ptr %892
  %894 = icmp ne i16 %893, 0
  %895 = sext i1 %894 to i8
  %896 = icmp ne i8 %895, 0
  br i1 %896, label %b191, label %b192

b194:
  call addrspace(1) void @N$EBND()
  unreachable

b195:
  %897 = mul i16 %885, 6
  %898 = getelementptr i8, ptr %857, i16 %897
  %899 = load i16, ptr %898
  %900 = load i16, ptr %38, !tbaa !2
  %901 = icmp eq i16 %899, %900
  %902 = sext i1 %901 to i8
  store i8 %902, ptr %34, !tbaa !2
  %903 = icmp ne i8 %902, 0
  br i1 %903, label %b197, label %b198

b196:
  call addrspace(1) void @N$EBND()
  unreachable

b197:
  %904 = load i16, ptr %37, !tbaa !2
  %905 = getelementptr i8, ptr %857, i16 -4
  %906 = load i16, ptr %905
  %907 = icmp ult i16 %904, %906
  %908 = sext i1 %907 to i8
  %909 = icmp ne i8 %908, 0
  br i1 %909, label %b199, label %b200

b198:
  %910 = load i8, ptr %34, !tbaa !2
  %911 = icmp ne i8 %910, 0
  br i1 %911, label %b201, label %b202

b199:
  %912 = mul i16 %904, 6
  %913 = getelementptr i8, ptr %857, i16 %912
  %914 = getelementptr i8, ptr %913, i16 2
  %915 = addrspacecast ptr %914 to ptr addrspace(1)
  %916 = addrspacecast ptr %39 to ptr addrspace(1)
  %917 = call addrspace(1) i8 @Card.eq(ptr addrspace(1) %915, ptr addrspace(1) %916)
  store i8 %917, ptr %34, !tbaa !2
  br label %b198

b200:
  call addrspace(1) void @N$EBND()
  unreachable

b201:
  store i8 -1, ptr %36, !tbaa !2
  br label %b192

b202:
  br label %b203

b203:
  %918 = load i16, ptr %37, !tbaa !2
  %919 = add i16 %918, 1
  %920 = load i16, ptr %35, !tbaa !2
  %921 = and i16 %919, %920
  store i16 %921, ptr %37, !tbaa !2
  br label %b190

b204:
  %922 = load i16, ptr %37, !tbaa !2
  %923 = getelementptr i8, ptr %857, i16 -4
  %924 = load i16, ptr %923
  %925 = icmp ult i16 %922, %924
  %926 = sext i1 %925 to i8
  %927 = icmp ne i8 %926, 0
  br i1 %927, label %b207, label %b208

b205:
  br label %b206

b206:
  %928 = load i8, ptr %36, !tbaa !2
  %929 = icmp ne i8 %928, 0
  br i1 %929, label %b212, label %b211

b207:
  %930 = mul i16 %922, 6
  %931 = getelementptr i8, ptr %857, i16 %930
  %932 = load i16, ptr %38, !tbaa !2
  store i16 %932, ptr %931
  %933 = load i16, ptr %37, !tbaa !2
  %934 = getelementptr i8, ptr %857, i16 -4
  %935 = load i16, ptr %934
  %936 = icmp ult i16 %933, %935
  %937 = sext i1 %936 to i8
  %938 = icmp ne i8 %937, 0
  br i1 %938, label %b209, label %b210

b208:
  call addrspace(1) void @N$EBND()
  unreachable

b209:
  %939 = mul i16 %933, 6
  %940 = getelementptr i8, ptr %857, i16 %939
  %941 = load i8, ptr %39, !tbaa !2
  %942 = getelementptr inbounds i8, ptr %39, i16 1
  %943 = load i8, ptr %942, !tbaa !2
  %944 = getelementptr i8, ptr %940, i16 2
  store i8 %941, ptr %944
  %945 = getelementptr i8, ptr %940, i16 3
  store i8 %943, ptr %945
  br label %b206

b210:
  call addrspace(1) void @N$EBND()
  unreachable

b211:
  %946 = getelementptr i8, ptr %857, i16 -2
  %947 = load i16, ptr %946
  %948 = add i16 %947, 1
  %949 = getelementptr i8, ptr %857, i16 -2
  store i16 %948, ptr %949
  br label %b212

b212:
  %950 = load i16, ptr %37, !tbaa !2
  %951 = getelementptr i8, ptr %857, i16 -4
  %952 = load i16, ptr %951
  %953 = icmp ult i16 %950, %952
  %954 = sext i1 %953 to i8
  %955 = icmp ne i8 %954, 0
  br i1 %955, label %b213, label %b214

b213:
  %956 = mul i16 %950, 6
  %957 = getelementptr i8, ptr %857, i16 %956
  %958 = getelementptr i8, ptr %957, i16 4
  store i8 -1, ptr %958
  br label %b157

b214:
  call addrspace(1) void @N$EBND()
  unreachable

b215:
  %959 = getelementptr i8, ptr %758, i16 -4
  %960 = load i16, ptr %959
  %961 = sub i16 %960, 1
  store i16 %961, ptr %29, !tbaa !2
  %962 = load i16, ptr %32, !tbaa !2
  %963 = load i16, ptr %29, !tbaa !2
  %964 = and i16 %962, %963
  store i16 %964, ptr %31, !tbaa !2
  br label %b218

b216:
  br label %b217

b217:
  %965 = load i8, ptr %30, !tbaa !2
  %966 = icmp ne i8 %965, 0
  br i1 %966, label %b232, label %b233

b218:
  %967 = load i16, ptr %31, !tbaa !2
  %968 = getelementptr i8, ptr %758, i16 -4
  %969 = load i16, ptr %968
  %970 = icmp ult i16 %967, %969
  %971 = sext i1 %970 to i8
  %972 = icmp ne i8 %971, 0
  br i1 %972, label %b221, label %b222

b219:
  %973 = load i16, ptr %31, !tbaa !2
  %974 = getelementptr i8, ptr %758, i16 -4
  %975 = load i16, ptr %974
  %976 = icmp ult i16 %973, %975
  %977 = sext i1 %976 to i8
  %978 = icmp ne i8 %977, 0
  br i1 %978, label %b223, label %b224

b220:
  br label %b217

b221:
  %979 = mul i16 %967, 6
  %980 = getelementptr i8, ptr %758, i16 %979
  %981 = load i16, ptr %980
  %982 = icmp ne i16 %981, 0
  %983 = sext i1 %982 to i8
  %984 = icmp ne i8 %983, 0
  br i1 %984, label %b219, label %b220

b222:
  call addrspace(1) void @N$EBND()
  unreachable

b223:
  %985 = mul i16 %973, 6
  %986 = getelementptr i8, ptr %758, i16 %985
  %987 = load i16, ptr %986
  %988 = load i16, ptr %32, !tbaa !2
  %989 = icmp eq i16 %987, %988
  %990 = sext i1 %989 to i8
  store i8 %990, ptr %28, !tbaa !2
  %991 = icmp ne i8 %990, 0
  br i1 %991, label %b225, label %b226

b224:
  call addrspace(1) void @N$EBND()
  unreachable

b225:
  %992 = load i16, ptr %31, !tbaa !2
  %993 = getelementptr i8, ptr %758, i16 -4
  %994 = load i16, ptr %993
  %995 = icmp ult i16 %992, %994
  %996 = sext i1 %995 to i8
  %997 = icmp ne i8 %996, 0
  br i1 %997, label %b227, label %b228

b226:
  %998 = load i8, ptr %28, !tbaa !2
  %999 = icmp ne i8 %998, 0
  br i1 %999, label %b229, label %b230

b227:
  %1000 = mul i16 %992, 6
  %1001 = getelementptr i8, ptr %758, i16 %1000
  %1002 = getelementptr i8, ptr %1001, i16 2
  %1003 = load i8, ptr %1002
  %1004 = load i8, ptr %33, !tbaa !2
  %1005 = call addrspace(1) i8 @Suit.eq(i8 %1003, i8 %1004)
  store i8 %1005, ptr %28, !tbaa !2
  br label %b226

b228:
  call addrspace(1) void @N$EBND()
  unreachable

b229:
  store i8 -1, ptr %30, !tbaa !2
  br label %b220

b230:
  br label %b231

b231:
  %1006 = load i16, ptr %31, !tbaa !2
  %1007 = add i16 %1006, 1
  %1008 = load i16, ptr %29, !tbaa !2
  %1009 = and i16 %1007, %1008
  store i16 %1009, ptr %31, !tbaa !2
  br label %b218

b232:
  %1010 = load i16, ptr %31, !tbaa !2
  %1011 = getelementptr i8, ptr %758, i16 -4
  %1012 = load i16, ptr %1011
  %1013 = icmp ult i16 %1010, %1012
  %1014 = sext i1 %1013 to i8
  %1015 = icmp ne i8 %1014, 0
  br i1 %1015, label %b234, label %b235

b233:
  call addrspace(1) void @N$EKEY()
  unreachable

b234:
  %1016 = mul i16 %1010, 6
  %1017 = getelementptr i8, ptr %758, i16 %1016
  %1018 = getelementptr i8, ptr %1017, i16 4
  %1019 = load i16, ptr %1018
  store i16 %1019, ptr %27, !tbaa !2
  %1020 = load ptr, ptr %47, !tbaa !2
  %1021 = getelementptr i8, ptr %1020, i16 -2
  %1022 = load i16, ptr %1021
  store i16 %1022, ptr %26, !tbaa !2
  %1023 = load ptr, ptr %47, !tbaa !2
  store i8 12, ptr %25, !tbaa !2
  %1024 = getelementptr inbounds i8, ptr %25, i16 1
  store i8 0, ptr %1024, !tbaa !2
  %1025 = addrspacecast ptr %25 to ptr addrspace(1)
  %1026 = call addrspace(1) i16 @Card.hash(ptr addrspace(1) %1025)
  %1027 = or i16 %1026, 1
  store i16 %1027, ptr %24, !tbaa !2
  store i16 0, ptr %23, !tbaa !2
  store i8 0, ptr %22, !tbaa !2
  %1028 = getelementptr i8, ptr %1023, i16 -4
  %1029 = load i16, ptr %1028
  %1030 = icmp ne i16 %1029, 0
  %1031 = sext i1 %1030 to i8
  %1032 = icmp ne i8 %1031, 0
  br i1 %1032, label %b236, label %b237

b235:
  call addrspace(1) void @N$EBND()
  unreachable

b236:
  %1033 = getelementptr i8, ptr %1023, i16 -4
  %1034 = load i16, ptr %1033
  %1035 = sub i16 %1034, 1
  store i16 %1035, ptr %21, !tbaa !2
  %1036 = load i16, ptr %24, !tbaa !2
  %1037 = load i16, ptr %21, !tbaa !2
  %1038 = and i16 %1036, %1037
  store i16 %1038, ptr %23, !tbaa !2
  br label %b239

b237:
  br label %b238

b238:
  %1039 = load i8, ptr %22, !tbaa !2
  store i8 %1039, ptr %19, !tbaa !2
  %1040 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %1040)
  %1041 = load i16, ptr %27, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %1041)
  %1042 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %1042)
  %1043 = load i16, ptr %26, !tbaa !2
  call addrspace(1) void @N$PU2(i16 %1043)
  %1044 = getelementptr i8, ptr @$str13, i16 6
  call addrspace(1) void @N$PS(ptr %1044)
  %1045 = load i8, ptr %19, !tbaa !2
  call addrspace(1) void @N$PB(i8 %1045)
  call addrspace(1) void @N$PN()
  %1046 = getelementptr i8, ptr @$str1, i16 6
  store ptr %1046, ptr %18, !tbaa !2
  store i16 0, ptr %17, !tbaa !2
  store i16 40, ptr %16, !tbaa !2
  br label %b253

b239:
  %1047 = load i16, ptr %23, !tbaa !2
  %1048 = getelementptr i8, ptr %1023, i16 -4
  %1049 = load i16, ptr %1048
  %1050 = icmp ult i16 %1047, %1049
  %1051 = sext i1 %1050 to i8
  %1052 = icmp ne i8 %1051, 0
  br i1 %1052, label %b242, label %b243

b240:
  %1053 = load i16, ptr %23, !tbaa !2
  %1054 = getelementptr i8, ptr %1023, i16 -4
  %1055 = load i16, ptr %1054
  %1056 = icmp ult i16 %1053, %1055
  %1057 = sext i1 %1056 to i8
  %1058 = icmp ne i8 %1057, 0
  br i1 %1058, label %b244, label %b245

b241:
  br label %b238

b242:
  %1059 = mul i16 %1047, 6
  %1060 = getelementptr i8, ptr %1023, i16 %1059
  %1061 = load i16, ptr %1060
  %1062 = icmp ne i16 %1061, 0
  %1063 = sext i1 %1062 to i8
  %1064 = icmp ne i8 %1063, 0
  br i1 %1064, label %b240, label %b241

b243:
  call addrspace(1) void @N$EBND()
  unreachable

b244:
  %1065 = mul i16 %1053, 6
  %1066 = getelementptr i8, ptr %1023, i16 %1065
  %1067 = load i16, ptr %1066
  %1068 = load i16, ptr %24, !tbaa !2
  %1069 = icmp eq i16 %1067, %1068
  %1070 = sext i1 %1069 to i8
  store i8 %1070, ptr %20, !tbaa !2
  %1071 = icmp ne i8 %1070, 0
  br i1 %1071, label %b246, label %b247

b245:
  call addrspace(1) void @N$EBND()
  unreachable

b246:
  %1072 = load i16, ptr %23, !tbaa !2
  %1073 = getelementptr i8, ptr %1023, i16 -4
  %1074 = load i16, ptr %1073
  %1075 = icmp ult i16 %1072, %1074
  %1076 = sext i1 %1075 to i8
  %1077 = icmp ne i8 %1076, 0
  br i1 %1077, label %b248, label %b249

b247:
  %1078 = load i8, ptr %20, !tbaa !2
  %1079 = icmp ne i8 %1078, 0
  br i1 %1079, label %b250, label %b251

b248:
  %1080 = mul i16 %1072, 6
  %1081 = getelementptr i8, ptr %1023, i16 %1080
  %1082 = getelementptr i8, ptr %1081, i16 2
  %1083 = addrspacecast ptr %1082 to ptr addrspace(1)
  %1084 = addrspacecast ptr %25 to ptr addrspace(1)
  %1085 = call addrspace(1) i8 @Card.eq(ptr addrspace(1) %1083, ptr addrspace(1) %1084)
  store i8 %1085, ptr %20, !tbaa !2
  br label %b247

b249:
  call addrspace(1) void @N$EBND()
  unreachable

b250:
  store i8 -1, ptr %22, !tbaa !2
  br label %b241

b251:
  br label %b252

b252:
  %1086 = load i16, ptr %23, !tbaa !2
  %1087 = add i16 %1086, 1
  %1088 = load i16, ptr %21, !tbaa !2
  %1089 = and i16 %1087, %1088
  store i16 %1089, ptr %23, !tbaa !2
  br label %b239

b253:
  %1090 = load i16, ptr %17, !tbaa !2
  %1091 = load i16, ptr %16, !tbaa !2
  %1092 = icmp slt i16 %1090, %1091
  %1093 = sext i1 %1092 to i8
  %1094 = icmp ne i8 %1093, 0
  br i1 %1094, label %b254, label %b256

b254:
  %1095 = load ptr, ptr %18, !tbaa !2
  %1096 = call addrspace(1) ptr @N$DRES(ptr %1095, i16 6)
  store ptr %1096, ptr %18, !tbaa !2
  %1097 = load i16, ptr %17, !tbaa !2
  store i16 %1097, ptr %15, !tbaa !2
  %1098 = load i16, ptr %15, !tbaa !2
  %1099 = call addrspace(1) i16 @i16.hash(i16 %1098)
  %1100 = or i16 %1099, 1
  store i16 %1100, ptr %14, !tbaa !2
  store i16 0, ptr %13, !tbaa !2
  store i8 0, ptr %12, !tbaa !2
  %1101 = getelementptr i8, ptr %1096, i16 -4
  %1102 = load i16, ptr %1101
  %1103 = icmp ne i16 %1102, 0
  %1104 = sext i1 %1103 to i8
  %1105 = icmp ne i8 %1104, 0
  br i1 %1105, label %b257, label %b258

b255:
  %1106 = load i16, ptr %17, !tbaa !2
  %1107 = add i16 %1106, 1
  store i16 %1107, ptr %17, !tbaa !2
  br label %b253

b256:
  %1108 = load ptr, ptr %18, !tbaa !2
  store ptr null, ptr %18, !tbaa !2
  store ptr %1108, ptr %9, !tbaa !2
  store i16 37, ptr %8, !tbaa !2
  %1109 = load ptr, ptr %9, !tbaa !2
  %1110 = getelementptr i8, ptr %1109, i16 -2
  %1111 = load i16, ptr %1110
  call addrspace(1) void @N$PU2(i16 %1111)
  %1112 = getelementptr i8, ptr @$str14, i16 6
  call addrspace(1) void @N$PS(ptr %1112)
  %1113 = load i16, ptr %8, !tbaa !2
  call addrspace(1) void @N$PI2(i16 %1113)
  %1114 = getelementptr i8, ptr @$str15, i16 6
  call addrspace(1) void @N$PS(ptr %1114)
  %1115 = load ptr, ptr %9, !tbaa !2
  %1116 = addrspacecast ptr %8 to ptr addrspace(1)
  %1117 = load i16, ptr addrspace(1) %1116, !tbaa !2
  %1118 = call addrspace(1) i16 @i16.hash(i16 %1117)
  %1119 = or i16 %1118, 1
  store i16 %1119, ptr %7, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  store i8 0, ptr %5, !tbaa !2
  %1120 = getelementptr i8, ptr %1115, i16 -4
  %1121 = load i16, ptr %1120
  %1122 = icmp ne i16 %1121, 0
  %1123 = sext i1 %1122 to i8
  %1124 = icmp ne i8 %1123, 0
  br i1 %1124, label %b285, label %b286

b257:
  %1125 = getelementptr i8, ptr %1096, i16 -4
  %1126 = load i16, ptr %1125
  %1127 = sub i16 %1126, 1
  store i16 %1127, ptr %11, !tbaa !2
  %1128 = load i16, ptr %14, !tbaa !2
  %1129 = load i16, ptr %11, !tbaa !2
  %1130 = and i16 %1128, %1129
  store i16 %1130, ptr %13, !tbaa !2
  br label %b260

b258:
  br label %b259

b259:
  %1131 = load i8, ptr %12, !tbaa !2
  %1132 = xor i8 %1131, -1
  %1133 = icmp ne i8 %1132, 0
  br i1 %1133, label %b274, label %b275

b260:
  %1134 = load i16, ptr %13, !tbaa !2
  %1135 = getelementptr i8, ptr %1096, i16 -4
  %1136 = load i16, ptr %1135
  %1137 = icmp ult i16 %1134, %1136
  %1138 = sext i1 %1137 to i8
  %1139 = icmp ne i8 %1138, 0
  br i1 %1139, label %b263, label %b264

b261:
  %1140 = load i16, ptr %13, !tbaa !2
  %1141 = getelementptr i8, ptr %1096, i16 -4
  %1142 = load i16, ptr %1141
  %1143 = icmp ult i16 %1140, %1142
  %1144 = sext i1 %1143 to i8
  %1145 = icmp ne i8 %1144, 0
  br i1 %1145, label %b265, label %b266

b262:
  br label %b259

b263:
  %1146 = mul i16 %1134, 6
  %1147 = getelementptr i8, ptr %1096, i16 %1146
  %1148 = load i16, ptr %1147
  %1149 = icmp ne i16 %1148, 0
  %1150 = sext i1 %1149 to i8
  %1151 = icmp ne i8 %1150, 0
  br i1 %1151, label %b261, label %b262

b264:
  call addrspace(1) void @N$EBND()
  unreachable

b265:
  %1152 = mul i16 %1140, 6
  %1153 = getelementptr i8, ptr %1096, i16 %1152
  %1154 = load i16, ptr %1153
  %1155 = load i16, ptr %14, !tbaa !2
  %1156 = icmp eq i16 %1154, %1155
  %1157 = sext i1 %1156 to i8
  store i8 %1157, ptr %10, !tbaa !2
  %1158 = icmp ne i8 %1157, 0
  br i1 %1158, label %b267, label %b268

b266:
  call addrspace(1) void @N$EBND()
  unreachable

b267:
  %1159 = load i16, ptr %13, !tbaa !2
  %1160 = getelementptr i8, ptr %1096, i16 -4
  %1161 = load i16, ptr %1160
  %1162 = icmp ult i16 %1159, %1161
  %1163 = sext i1 %1162 to i8
  %1164 = icmp ne i8 %1163, 0
  br i1 %1164, label %b269, label %b270

b268:
  %1165 = load i8, ptr %10, !tbaa !2
  %1166 = icmp ne i8 %1165, 0
  br i1 %1166, label %b271, label %b272

b269:
  %1167 = mul i16 %1159, 6
  %1168 = getelementptr i8, ptr %1096, i16 %1167
  %1169 = getelementptr i8, ptr %1168, i16 2
  %1170 = load i16, ptr %1169
  %1171 = load i16, ptr %15, !tbaa !2
  %1172 = call addrspace(1) i8 @i16.eq(i16 %1170, i16 %1171)
  store i8 %1172, ptr %10, !tbaa !2
  br label %b268

b270:
  call addrspace(1) void @N$EBND()
  unreachable

b271:
  store i8 -1, ptr %12, !tbaa !2
  br label %b262

b272:
  br label %b273

b273:
  %1173 = load i16, ptr %13, !tbaa !2
  %1174 = add i16 %1173, 1
  %1175 = load i16, ptr %11, !tbaa !2
  %1176 = and i16 %1174, %1175
  store i16 %1176, ptr %13, !tbaa !2
  br label %b260

b274:
  %1177 = load i16, ptr %13, !tbaa !2
  %1178 = getelementptr i8, ptr %1096, i16 -4
  %1179 = load i16, ptr %1178
  %1180 = icmp ult i16 %1177, %1179
  %1181 = sext i1 %1180 to i8
  %1182 = icmp ne i8 %1181, 0
  br i1 %1182, label %b277, label %b278

b275:
  br label %b276

b276:
  %1183 = load i8, ptr %12, !tbaa !2
  %1184 = icmp ne i8 %1183, 0
  br i1 %1184, label %b282, label %b281

b277:
  %1185 = mul i16 %1177, 6
  %1186 = getelementptr i8, ptr %1096, i16 %1185
  %1187 = load i16, ptr %14, !tbaa !2
  store i16 %1187, ptr %1186
  %1188 = load i16, ptr %13, !tbaa !2
  %1189 = getelementptr i8, ptr %1096, i16 -4
  %1190 = load i16, ptr %1189
  %1191 = icmp ult i16 %1188, %1190
  %1192 = sext i1 %1191 to i8
  %1193 = icmp ne i8 %1192, 0
  br i1 %1193, label %b279, label %b280

b278:
  call addrspace(1) void @N$EBND()
  unreachable

b279:
  %1194 = mul i16 %1188, 6
  %1195 = getelementptr i8, ptr %1096, i16 %1194
  %1196 = load i16, ptr %15, !tbaa !2
  %1197 = getelementptr i8, ptr %1195, i16 2
  store i16 %1196, ptr %1197
  br label %b276

b280:
  call addrspace(1) void @N$EBND()
  unreachable

b281:
  %1198 = getelementptr i8, ptr %1096, i16 -2
  %1199 = load i16, ptr %1198
  %1200 = add i16 %1199, 1
  %1201 = getelementptr i8, ptr %1096, i16 -2
  store i16 %1200, ptr %1201
  br label %b282

b282:
  %1202 = load i16, ptr %13, !tbaa !2
  %1203 = getelementptr i8, ptr %1096, i16 -4
  %1204 = load i16, ptr %1203
  %1205 = icmp ult i16 %1202, %1204
  %1206 = sext i1 %1205 to i8
  %1207 = icmp ne i8 %1206, 0
  br i1 %1207, label %b283, label %b284

b283:
  %1208 = mul i16 %1202, 6
  %1209 = getelementptr i8, ptr %1096, i16 %1208
  %1210 = load i16, ptr %17, !tbaa !2
  %1211 = load i16, ptr %17, !tbaa !2
  %1212 = mul i16 %1210, %1211
  %1213 = getelementptr i8, ptr %1209, i16 4
  store i16 %1212, ptr %1213
  br label %b255

b284:
  call addrspace(1) void @N$EBND()
  unreachable

b285:
  %1214 = getelementptr i8, ptr %1115, i16 -4
  %1215 = load i16, ptr %1214
  %1216 = sub i16 %1215, 1
  store i16 %1216, ptr %4, !tbaa !2
  %1217 = load i16, ptr %7, !tbaa !2
  %1218 = load i16, ptr %4, !tbaa !2
  %1219 = and i16 %1217, %1218
  store i16 %1219, ptr %6, !tbaa !2
  br label %b288

b286:
  br label %b287

b287:
  %1220 = load i8, ptr %5, !tbaa !2
  %1221 = icmp ne i8 %1220, 0
  br i1 %1221, label %b302, label %b303

b288:
  %1222 = load i16, ptr %6, !tbaa !2
  %1223 = getelementptr i8, ptr %1115, i16 -4
  %1224 = load i16, ptr %1223
  %1225 = icmp ult i16 %1222, %1224
  %1226 = sext i1 %1225 to i8
  %1227 = icmp ne i8 %1226, 0
  br i1 %1227, label %b291, label %b292

b289:
  %1228 = load i16, ptr %6, !tbaa !2
  %1229 = getelementptr i8, ptr %1115, i16 -4
  %1230 = load i16, ptr %1229
  %1231 = icmp ult i16 %1228, %1230
  %1232 = sext i1 %1231 to i8
  %1233 = icmp ne i8 %1232, 0
  br i1 %1233, label %b293, label %b294

b290:
  br label %b287

b291:
  %1234 = mul i16 %1222, 6
  %1235 = getelementptr i8, ptr %1115, i16 %1234
  %1236 = load i16, ptr %1235
  %1237 = icmp ne i16 %1236, 0
  %1238 = sext i1 %1237 to i8
  %1239 = icmp ne i8 %1238, 0
  br i1 %1239, label %b289, label %b290

b292:
  call addrspace(1) void @N$EBND()
  unreachable

b293:
  %1240 = mul i16 %1228, 6
  %1241 = getelementptr i8, ptr %1115, i16 %1240
  %1242 = load i16, ptr %1241
  %1243 = load i16, ptr %7, !tbaa !2
  %1244 = icmp eq i16 %1242, %1243
  %1245 = sext i1 %1244 to i8
  store i8 %1245, ptr %3, !tbaa !2
  %1246 = icmp ne i8 %1245, 0
  br i1 %1246, label %b295, label %b296

b294:
  call addrspace(1) void @N$EBND()
  unreachable

b295:
  %1247 = load i16, ptr %6, !tbaa !2
  %1248 = getelementptr i8, ptr %1115, i16 -4
  %1249 = load i16, ptr %1248
  %1250 = icmp ult i16 %1247, %1249
  %1251 = sext i1 %1250 to i8
  %1252 = icmp ne i8 %1251, 0
  br i1 %1252, label %b297, label %b298

b296:
  %1253 = load i8, ptr %3, !tbaa !2
  %1254 = icmp ne i8 %1253, 0
  br i1 %1254, label %b299, label %b300

b297:
  %1255 = mul i16 %1247, 6
  %1256 = getelementptr i8, ptr %1115, i16 %1255
  %1257 = getelementptr i8, ptr %1256, i16 2
  %1258 = load i16, ptr %1257
  %1259 = load i16, ptr addrspace(1) %1116, !tbaa !2
  %1260 = call addrspace(1) i8 @i16.eq(i16 %1258, i16 %1259)
  store i8 %1260, ptr %3, !tbaa !2
  br label %b296

b298:
  call addrspace(1) void @N$EBND()
  unreachable

b299:
  store i8 -1, ptr %5, !tbaa !2
  br label %b290

b300:
  br label %b301

b301:
  %1261 = load i16, ptr %6, !tbaa !2
  %1262 = add i16 %1261, 1
  %1263 = load i16, ptr %4, !tbaa !2
  %1264 = and i16 %1262, %1263
  store i16 %1264, ptr %6, !tbaa !2
  br label %b288

b302:
  %1265 = load i16, ptr %6, !tbaa !2
  %1266 = getelementptr i8, ptr %1115, i16 -4
  %1267 = load i16, ptr %1266
  %1268 = icmp ult i16 %1265, %1267
  %1269 = sext i1 %1268 to i8
  %1270 = icmp ne i8 %1269, 0
  br i1 %1270, label %b304, label %b305

b303:
  call addrspace(1) void @N$EKEY()
  unreachable

b304:
  %1271 = mul i16 %1265, 6
  %1272 = getelementptr i8, ptr %1115, i16 %1271
  %1273 = getelementptr i8, ptr %1272, i16 4
  %1274 = load i16, ptr %1273
  call addrspace(1) void @N$PI2(i16 %1274)
  call addrspace(1) void @N$PN()
  %1275 = load ptr, ptr %9, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %1275)
  %1276 = load ptr, ptr %18, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %1276)
  %1277 = load ptr, ptr %47, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %1277)
  %1278 = load ptr, ptr %48, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %1278)
  %1279 = load ptr, ptr %49, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %1279)
  %1280 = load ptr, ptr %62, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %1280)
  %1281 = load ptr, ptr %63, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %1281)
  %1282 = load ptr, ptr %74, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %1282)
  %1283 = load ptr, ptr %86, !tbaa !2
  call addrspace(1) void @N$BDRP(ptr %1283)
  %1284 = load ptr, ptr %105, !tbaa !2
  %1285 = icmp ne ptr %1284, null
  %1286 = sext i1 %1285 to i8
  %1287 = icmp ne i8 %1286, 0
  br i1 %1287, label %b307, label %b306

b305:
  call addrspace(1) void @N$EBND()
  unreachable

b306:
  call addrspace(1) void @N$BDRP(ptr %1284)
  %1288 = load ptr, ptr %106, !tbaa !2
  %1289 = icmp ne ptr %1288, null
  %1290 = sext i1 %1289 to i8
  %1291 = icmp ne i8 %1290, 0
  br i1 %1291, label %b312, label %b311

b307:
  %1292 = getelementptr i8, ptr %1284, i16 -4
  %1293 = load i16, ptr %1292
  store i16 0, ptr %2, !tbaa !2
  br label %b308

b308:
  %1294 = load i16, ptr %2, !tbaa !2
  %1295 = icmp ult i16 %1294, %1293
  %1296 = sext i1 %1295 to i8
  %1297 = icmp ne i8 %1296, 0
  br i1 %1297, label %b310, label %b309

b309:
  br label %b306

b310:
  %1298 = mul i16 %1294, 6
  %1299 = getelementptr i8, ptr %1284, i16 %1298
  %1300 = getelementptr i8, ptr %1299, i16 2
  %1301 = load ptr, ptr %1300
  call addrspace(1) void @N$BDRP(ptr %1301)
  %1302 = add i16 %1294, 1
  store i16 %1302, ptr %2, !tbaa !2
  br label %b308

b311:
  call addrspace(1) void @N$BDRP(ptr %1288)
  %1303 = load ptr, ptr %107, !tbaa !2
  %1304 = icmp ne ptr %1303, null
  %1305 = sext i1 %1304 to i8
  %1306 = icmp ne i8 %1305, 0
  br i1 %1306, label %b317, label %b316

b312:
  %1307 = getelementptr i8, ptr %1288, i16 -4
  %1308 = load i16, ptr %1307
  store i16 0, ptr %1, !tbaa !2
  br label %b313

b313:
  %1309 = load i16, ptr %1, !tbaa !2
  %1310 = icmp ult i16 %1309, %1308
  %1311 = sext i1 %1310 to i8
  %1312 = icmp ne i8 %1311, 0
  br i1 %1312, label %b315, label %b314

b314:
  br label %b311

b315:
  %1313 = mul i16 %1309, 6
  %1314 = getelementptr i8, ptr %1288, i16 %1313
  %1315 = getelementptr i8, ptr %1314, i16 2
  %1316 = load ptr, ptr %1315
  call addrspace(1) void @N$BDRP(ptr %1316)
  %1317 = add i16 %1309, 1
  store i16 %1317, ptr %1, !tbaa !2
  br label %b313

b316:
  call addrspace(1) void @N$BDRP(ptr %1303)
  ret i16 0

b317:
  %1318 = getelementptr i8, ptr %1303, i16 -4
  %1319 = load i16, ptr %1318
  store i16 0, ptr %0, !tbaa !2
  br label %b318

b318:
  %1320 = load i16, ptr %0, !tbaa !2
  %1321 = icmp ult i16 %1320, %1319
  %1322 = sext i1 %1321 to i8
  %1323 = icmp ne i8 %1322, 0
  br i1 %1323, label %b320, label %b319

b319:
  br label %b316

b320:
  %1324 = mul i16 %1320, 2
  %1325 = getelementptr i8, ptr %1303, i16 %1324
  %1326 = load ptr, ptr %1325
  call addrspace(1) void @N$BDRP(ptr %1326)
  %1327 = add i16 %1320, 1
  store i16 %1327, ptr %0, !tbaa !2
  br label %b318
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

define internal i8 @Card.eq(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) {
b1:
  %2 = alloca i8
  store i8 0, ptr %2
  %3 = load i8, ptr addrspace(1) %0
  %4 = load i8, ptr addrspace(1) %1
  %5 = call addrspace(1) i8 @u8.eq(i8 %3, i8 %4)
  store i8 %5, ptr %2, !tbaa !2
  %6 = icmp ne i8 %5, 0
  br i1 %6, label %b2, label %b3

b2:
  %7 = getelementptr i8, ptr addrspace(1) %0, i16 1
  %8 = load i8, ptr addrspace(1) %7
  %9 = getelementptr i8, ptr addrspace(1) %1, i16 1
  %10 = load i8, ptr addrspace(1) %9
  %11 = call addrspace(1) i8 @Suit.eq(i8 %8, i8 %10)
  store i8 %11, ptr %2, !tbaa !2
  br label %b3

b3:
  %12 = load i8, ptr %2, !tbaa !2
  ret i8 %12
}

define internal i8 @u8.eq(i8 %0, i8 %1) addrspace(1) {
b1:
  %2 = zext i8 %0 to i16
  %3 = zext i8 %1 to i16
  %4 = icmp eq i16 %2, %3
  %5 = sext i1 %4 to i8
  ret i8 %5
}

define internal i16 @Card.hash(ptr addrspace(1) %0) addrspace(1) {
b1:
  %1 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %1, !tbaa !2
  %2 = load i16, ptr %1, !tbaa !2
  %3 = mul i16 %2, 31
  %4 = load i8, ptr addrspace(1) %0
  %5 = call addrspace(1) i16 @u8.hash(i8 %4)
  %6 = add i16 %3, %5
  store i16 %6, ptr %1, !tbaa !2
  %7 = load i16, ptr %1, !tbaa !2
  %8 = mul i16 %7, 31
  %9 = getelementptr i8, ptr addrspace(1) %0, i16 1
  %10 = load i8, ptr addrspace(1) %9
  %11 = call addrspace(1) i16 @Suit.hash(i8 %10)
  %12 = add i16 %8, %11
  store i16 %12, ptr %1, !tbaa !2
  %13 = load i16, ptr %1, !tbaa !2
  ret i16 %13
}

define internal i16 @u8.hash(i8 %0) addrspace(1) {
b1:
  %1 = zext i8 %0 to i16
  ret i16 %1
}

define internal i8 @Suit.eq(i8 %0, i8 %1) addrspace(1) {
b1:
  %2 = icmp eq i8 %0, %1
  %3 = sext i1 %2 to i8
  ret i8 %3
}

define internal i16 @Suit.hash(i8 %0) addrspace(1) {
b1:
  %1 = zext i8 %0 to i16
  ret i16 %1
}

define internal i8 @string.eq(ptr addrspace(1) noalias readonly dereferenceable(8) %0, ptr addrspace(1) noalias readonly dereferenceable(8) %1) addrspace(1) {
b1:
  %2 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %0, ptr addrspace(1) %1)
  %3 = icmp eq i8 %2, 0
  %4 = sext i1 %3 to i8
  ret i8 %4
}

define internal i16 @string.hash(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) {
b1:
  %1 = alloca i16
  %2 = alloca i16
  store i16 0, ptr %1
  store i16 0, ptr %2
  store i16 5381, ptr %2, !tbaa !2
  %3 = load i16, ptr addrspace(1) %0
  store i16 0, ptr %1, !tbaa !2
  br label %b2

b2:
  %4 = load i16, ptr %1, !tbaa !2
  %5 = icmp ult i16 %4, %3
  %6 = sext i1 %5 to i8
  %7 = icmp ne i8 %6, 0
  br i1 %7, label %b3, label %b5

b3:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %9 = load ptr addrspace(1), ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %9, i16 %4
  %11 = load i16, ptr %2, !tbaa !2
  %12 = mul i16 %11, 33
  %13 = load i8, ptr addrspace(1) %10
  %14 = zext i8 %13 to i16
  %15 = xor i16 %12, %14
  store i16 %15, ptr %2, !tbaa !2
  br label %b4

b4:
  %16 = load i16, ptr %1, !tbaa !2
  %17 = add i16 %16, 1
  store i16 %17, ptr %1, !tbaa !2
  br label %b2

b5:
  %18 = load i16, ptr %2, !tbaa !2
  ret i16 %18
}

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nounwind willreturn memory(argmem: write)

declare ptr @N$BGRW(ptr, i16, i16) addrspace(1)

declare ptr @N$DRES(ptr, i16) addrspace(1)

declare ptr @N$BCLN(ptr, i16) addrspace(1)

declare void @N$EBND() addrspace(1)

declare void @N$BDRP(ptr) addrspace(1)

declare void @N$EKEY() addrspace(1)

declare void @N$PU2(i16) addrspace(1)

declare void @N$PS(ptr) addrspace(1)

declare void @N$PI2(i16) addrspace(1)

declare void @N$PN() addrspace(1)

declare void @N$PB(i8) addrspace(1)

declare i8 @N$VCMP(ptr addrspace(1), ptr addrspace(1)) addrspace(1)

!0 = !{!"llrm hir"}
!1 = !{!"place", !0, i64 0}
!2 = !{!1, !1, i64 0}
!3 = !{!"allocation", !0, i64 0}
!4 = !{!3, !3, i64 0}
