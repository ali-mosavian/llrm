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
  %0 = alloca [2 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = alloca [8 x i8]
  call void @llvm.memset.p0.i16(ptr %0, i8 0, i16 2, i1 false)
  call void @llvm.memset.p0.i16(ptr %1, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %2, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %3, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %4, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %5, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %6, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %7, i8 0, i16 8, i1 false)
  call void @llvm.memset.p0.i16(ptr %8, i8 0, i16 8, i1 false)
  %9 = getelementptr i8, ptr @$str1, i16 6
  %10 = call addrspace(1) ptr @N$BGRW(ptr %9, i16 9, i16 2)
  %11 = getelementptr i8, ptr %10, i16 0
  %12 = getelementptr i8, ptr @$str2, i16 6
  store ptr %12, ptr %11
  %13 = getelementptr i8, ptr %10, i16 2
  %14 = getelementptr i8, ptr @$str3, i16 6
  store ptr %14, ptr %13
  %15 = getelementptr i8, ptr %10, i16 4
  %16 = getelementptr i8, ptr @$str4, i16 6
  store ptr %16, ptr %15
  %17 = getelementptr i8, ptr %10, i16 6
  store ptr %12, ptr %17
  %18 = getelementptr i8, ptr %10, i16 8
  %19 = getelementptr i8, ptr @$str5, i16 6
  store ptr %19, ptr %18
  %20 = getelementptr i8, ptr %10, i16 10
  %21 = getelementptr i8, ptr @$str6, i16 6
  store ptr %21, ptr %20
  %22 = getelementptr i8, ptr %10, i16 12
  store ptr %12, ptr %22
  %23 = getelementptr i8, ptr %10, i16 14
  store ptr %14, ptr %23
  %24 = getelementptr i8, ptr %10, i16 16
  %25 = getelementptr i8, ptr @$str7, i16 6
  store ptr %25, ptr %24
  %26 = getelementptr i8, ptr %10, i16 -4
  %27 = load i16, ptr %26
  %28 = getelementptr inbounds i8, ptr %6, i16 2
  %29 = getelementptr inbounds i8, ptr %6, i16 4
  %30 = addrspacecast ptr %6 to ptr addrspace(1)
  %31 = getelementptr inbounds i8, ptr %5, i16 2
  %32 = getelementptr inbounds i8, ptr %5, i16 4
  %33 = addrspacecast ptr %5 to ptr addrspace(1)
  %34 = getelementptr inbounds i8, ptr %8, i16 2
  %35 = getelementptr inbounds i8, ptr %8, i16 4
  %36 = addrspacecast ptr %8 to ptr addrspace(1)
  %37 = getelementptr inbounds i8, ptr %7, i16 2
  %38 = getelementptr inbounds i8, ptr %7, i16 4
  %39 = addrspacecast ptr %7 to ptr addrspace(1)
  br label %b2

b2:
  %40 = phi ptr [ %9, %b1 ], [ %45, %b53 ]
  %41 = phi i16 [ 0, %b1 ], [ %185, %b53 ]
  %42 = icmp ult i16 %41, %27
  br i1 %42, label %b3, label %b5

b3:
  %43 = shl i16 %41, 1
  %44 = getelementptr i8, ptr %10, i16 %43
  %45 = call addrspace(1) ptr @N$DRES(ptr %40, i16 6)
  %46 = load ptr, ptr %44
  %47 = call addrspace(1) ptr @N$BCLN(ptr %46, i16 1)
  %48 = getelementptr i8, ptr %47, i16 -4
  %49 = load i16, ptr %48
  %50 = addrspacecast ptr %47 to ptr addrspace(1)
  br label %51

51:
  %52 = phi i16 [ 5381, %b3 ], [ %60, %55 ]
  %53 = phi i16 [ 0, %b3 ], [ %61, %55 ]
  %54 = icmp ult i16 %53, %49
  br i1 %54, label %55, label %62

55:
  %56 = getelementptr i8, ptr addrspace(1) %50, i16 %53
  %57 = mul i16 %52, 33
  %58 = load i8, ptr addrspace(1) %56
  %59 = zext i8 %58 to i16
  %60 = xor i16 %57, %59
  %61 = add i16 %53, 1
  br label %51

62:
  %63 = or i16 %52, 1
  %64 = getelementptr i8, ptr %45, i16 -4
  %65 = load i16, ptr %64
  %66 = icmp ne i16 %65, 0
  br i1 %66, label %b6, label %b8

b5:
  %67 = getelementptr i8, ptr %12, i16 -4
  %68 = load i16, ptr %67
  %69 = addrspacecast ptr %12 to ptr addrspace(1)
  br label %70

70:
  %71 = phi i16 [ 5381, %b5 ], [ %79, %74 ]
  %72 = phi i16 [ 0, %b5 ], [ %80, %74 ]
  %73 = icmp ult i16 %72, %68
  br i1 %73, label %74, label %81

74:
  %75 = getelementptr i8, ptr addrspace(1) %69, i16 %72
  %76 = mul i16 %71, 33
  %77 = load i8, ptr addrspace(1) %75
  %78 = zext i8 %77 to i16
  %79 = xor i16 %76, %78
  %80 = add i16 %72, 1
  br label %70

81:
  %82 = or i16 %71, 1
  %83 = getelementptr i8, ptr %40, i16 -4
  %84 = load i16, ptr %83
  %85 = icmp ne i16 %84, 0
  br i1 %85, label %b56, label %b58

b6:
  %86 = add i16 %65, -1
  %87 = and i16 %63, %86
  br label %b9

b8:
  %88 = phi i16 [ 0, %62 ], [ %92, %b11 ]
  %89 = phi i8 [ 0, %62 ], [ %98, %b11 ]
  %90 = xor i8 %89, -1
  %91 = icmp ne i8 %90, 0
  br i1 %91, label %b23, label %b25

b9:
  %92 = phi i16 [ %87, %b6 ], [ %115, %b21 ]
  %93 = load i16, ptr %64
  %94 = icmp ult i16 %92, %93
  br i1 %94, label %b12, label %b13

b10:
  %95 = load i16, ptr %100
  %96 = icmp eq i16 %95, %63
  %97 = sext i1 %96 to i8
  br i1 %96, label %b18, label %b17

b11:
  %98 = phi i8 [ 0, %b12 ], [ -1, %b20 ]
  br label %b8

b12:
  %99 = mul i16 %92, 6
  %100 = getelementptr i8, ptr %45, i16 %99
  %101 = load i16, ptr %100
  %102 = icmp ne i16 %101, 0
  br i1 %102, label %b10, label %b11

b13:
  call addrspace(1) void @N$EBND()
  unreachable

b17:
  %103 = phi i8 [ %97, %b10 ], [ %113, %b18 ]
  %104 = icmp ne i8 %103, 0
  br i1 %104, label %b20, label %b21

b18:
  %105 = getelementptr i8, ptr %100, i16 2
  %106 = load ptr, ptr %105
  %107 = getelementptr i8, ptr %106, i16 -4
  %108 = load i16, ptr %107
  %109 = addrspacecast ptr %106 to ptr addrspace(1)
  store i16 %108, ptr %8, !tbaa !2
  store i16 %108, ptr %34, !tbaa !2
  store ptr addrspace(1) %109, ptr %35, !tbaa !2
  %110 = load i16, ptr %48
  store i16 %110, ptr %7, !tbaa !2
  store i16 %110, ptr %37, !tbaa !2
  store ptr addrspace(1) %50, ptr %38, !tbaa !2
  %111 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %36, ptr addrspace(1) %39)
  %112 = icmp eq i8 %111, 0
  %113 = sext i1 %112 to i8
  br label %b17

b20:
  br label %b11

b21:
  %114 = add i16 %92, 1
  %115 = and i16 %114, %86
  br label %b9

b23:
  %116 = load i16, ptr %64
  %117 = icmp ult i16 %88, %116
  br i1 %117, label %b26, label %b27

b25:
  %118 = phi ptr [ %47, %b8 ], [ null, %b28 ]
  %119 = icmp ne i8 %89, 0
  br i1 %119, label %b31, label %b30

b26:
  %120 = mul i16 %88, 6
  %121 = getelementptr i8, ptr %45, i16 %120
  store i16 %63, ptr %121
  %122 = load i16, ptr %64
  %123 = icmp ult i16 %88, %122
  br i1 %123, label %b28, label %b29

b27:
  call addrspace(1) void @N$EBND()
  unreachable

b28:
  %124 = getelementptr i8, ptr %121, i16 2
  %125 = load ptr, ptr %124
  call addrspace(1) void @N$BDRP(ptr %125)
  store ptr %47, ptr %124
  br label %b25

b29:
  call addrspace(1) void @N$EBND()
  unreachable

b30:
  %126 = getelementptr i8, ptr %45, i16 -2
  %127 = load i16, ptr %126
  %128 = add i16 %127, 1
  store i16 %128, ptr %126
  br label %b31

b31:
  %129 = load i16, ptr %64
  %130 = icmp ult i16 %88, %129
  br i1 %130, label %b32, label %b33

b32:
  %131 = mul i16 %88, 6
  %132 = getelementptr i8, ptr %45, i16 %131
  %133 = load ptr, ptr %44
  %134 = getelementptr i8, ptr %133, i16 -4
  %135 = load i16, ptr %134
  %136 = addrspacecast ptr %133 to ptr addrspace(1)
  store i16 %135, ptr %6, !tbaa !2
  store i16 %135, ptr %28, !tbaa !2
  store ptr addrspace(1) %136, ptr %29, !tbaa !2
  br label %137

137:
  %138 = phi i16 [ 5381, %b32 ], [ %146, %141 ]
  %139 = phi i16 [ 0, %b32 ], [ %147, %141 ]
  %140 = icmp ult i16 %139, %135
  br i1 %140, label %141, label %148

141:
  %142 = getelementptr i8, ptr addrspace(1) %136, i16 %139
  %143 = mul i16 %138, 33
  %144 = load i8, ptr addrspace(1) %142
  %145 = zext i8 %144 to i16
  %146 = xor i16 %143, %145
  %147 = add i16 %139, 1
  br label %137

148:
  %149 = or i16 %138, 1
  %150 = load i16, ptr %64
  %151 = icmp ne i16 %150, 0
  br i1 %151, label %b34, label %b36

b33:
  call addrspace(1) void @N$EBND()
  unreachable

b34:
  %152 = add i16 %150, -1
  %153 = and i16 %149, %152
  br label %b37

b36:
  %154 = phi i16 [ 0, %148 ], [ %157, %b39 ]
  %155 = phi i8 [ 0, %148 ], [ %163, %b39 ]
  %156 = icmp ne i8 %155, 0
  br i1 %156, label %b51, label %b53

b37:
  %157 = phi i16 [ %153, %b34 ], [ %179, %b49 ]
  %158 = load i16, ptr %64
  %159 = icmp ult i16 %157, %158
  br i1 %159, label %b40, label %b41

b38:
  %160 = load i16, ptr %165
  %161 = icmp eq i16 %160, %149
  %162 = sext i1 %161 to i8
  br i1 %161, label %b46, label %b45

b39:
  %163 = phi i8 [ 0, %b40 ], [ -1, %b48 ]
  br label %b36

b40:
  %164 = mul i16 %157, 6
  %165 = getelementptr i8, ptr %45, i16 %164
  %166 = load i16, ptr %165
  %167 = icmp ne i16 %166, 0
  br i1 %167, label %b38, label %b39

b41:
  call addrspace(1) void @N$EBND()
  unreachable

b45:
  %168 = phi i8 [ %162, %b38 ], [ %177, %b46 ]
  %169 = icmp ne i8 %168, 0
  br i1 %169, label %b48, label %b49

b46:
  %170 = getelementptr i8, ptr %165, i16 2
  %171 = load ptr, ptr %170
  %172 = getelementptr i8, ptr %171, i16 -4
  %173 = load i16, ptr %172
  %174 = addrspacecast ptr %171 to ptr addrspace(1)
  store i16 %173, ptr %5, !tbaa !2
  store i16 %173, ptr %31, !tbaa !2
  store ptr addrspace(1) %174, ptr %32, !tbaa !2
  %175 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %33, ptr addrspace(1) %30)
  %176 = icmp eq i8 %175, 0
  %177 = sext i1 %176 to i8
  br label %b45

b48:
  br label %b39

b49:
  %178 = add i16 %157, 1
  %179 = and i16 %178, %152
  br label %b37

b51:
  %180 = load i16, ptr %64
  %181 = icmp ult i16 %154, %180
  br i1 %181, label %b54, label %b55

b53:
  %182 = phi i16 [ 0, %b36 ], [ %189, %b54 ]
  %183 = add i16 %182, 1
  %184 = getelementptr i8, ptr %132, i16 4
  store i16 %183, ptr %184
  call addrspace(1) void @N$BDRP(ptr %118)
  %185 = add i16 %41, 1
  br label %b2

b54:
  %186 = mul i16 %154, 6
  %187 = getelementptr i8, ptr %45, i16 %186
  %188 = getelementptr i8, ptr %187, i16 4
  %189 = load i16, ptr %188
  br label %b53

b55:
  call addrspace(1) void @N$EBND()
  unreachable

b56:
  %190 = add i16 %84, -1
  %191 = and i16 %82, %190
  %192 = getelementptr inbounds i8, ptr %4, i16 2
  %193 = getelementptr inbounds i8, ptr %4, i16 4
  %194 = addrspacecast ptr %4 to ptr addrspace(1)
  %195 = getelementptr inbounds i8, ptr %3, i16 2
  %196 = getelementptr inbounds i8, ptr %3, i16 4
  %197 = addrspacecast ptr %3 to ptr addrspace(1)
  br label %b59

b58:
  %198 = phi i16 [ 0, %81 ], [ %201, %b61 ]
  %199 = phi i8 [ 0, %81 ], [ %207, %b61 ]
  %200 = icmp ne i8 %199, 0
  br i1 %200, label %b73, label %b74

b59:
  %201 = phi i16 [ %191, %b56 ], [ %224, %b71 ]
  %202 = load i16, ptr %83
  %203 = icmp ult i16 %201, %202
  br i1 %203, label %b62, label %b63

b60:
  %204 = load i16, ptr %209
  %205 = icmp eq i16 %204, %82
  %206 = sext i1 %205 to i8
  br i1 %205, label %b68, label %b67

b61:
  %207 = phi i8 [ 0, %b62 ], [ -1, %b70 ]
  br label %b58

b62:
  %208 = mul i16 %201, 6
  %209 = getelementptr i8, ptr %40, i16 %208
  %210 = load i16, ptr %209
  %211 = icmp ne i16 %210, 0
  br i1 %211, label %b60, label %b61

b63:
  call addrspace(1) void @N$EBND()
  unreachable

b67:
  %212 = phi i8 [ %206, %b60 ], [ %222, %b68 ]
  %213 = icmp ne i8 %212, 0
  br i1 %213, label %b70, label %b71

b68:
  %214 = getelementptr i8, ptr %209, i16 2
  %215 = load ptr, ptr %214
  %216 = getelementptr i8, ptr %215, i16 -4
  %217 = load i16, ptr %216
  %218 = addrspacecast ptr %215 to ptr addrspace(1)
  store i16 %217, ptr %4, !tbaa !2
  store i16 %217, ptr %192, !tbaa !2
  store ptr addrspace(1) %218, ptr %193, !tbaa !2
  %219 = load i16, ptr %67
  store i16 %219, ptr %3, !tbaa !2
  store i16 %219, ptr %195, !tbaa !2
  store ptr addrspace(1) %69, ptr %196, !tbaa !2
  %220 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %194, ptr addrspace(1) %197)
  %221 = icmp eq i8 %220, 0
  %222 = sext i1 %221 to i8
  br label %b67

b70:
  br label %b61

b71:
  %223 = add i16 %201, 1
  %224 = and i16 %223, %190
  br label %b59

b73:
  %225 = load i16, ptr %83
  %226 = icmp ult i16 %198, %225
  br i1 %226, label %b75, label %b76

b74:
  call addrspace(1) void @N$EKEY()
  unreachable

b75:
  %227 = mul i16 %198, 6
  %228 = getelementptr i8, ptr %40, i16 %227
  %229 = getelementptr i8, ptr %228, i16 4
  %230 = load i16, ptr %229
  %231 = getelementptr i8, ptr %40, i16 -2
  %232 = load i16, ptr %231
  %233 = getelementptr i8, ptr @$str8, i16 6
  %234 = getelementptr i8, ptr %233, i16 -4
  %235 = load i16, ptr %234
  %236 = addrspacecast ptr %233 to ptr addrspace(1)
  br label %237

237:
  %238 = phi i16 [ 5381, %b75 ], [ %246, %241 ]
  %239 = phi i16 [ 0, %b75 ], [ %247, %241 ]
  %240 = icmp ult i16 %239, %235
  br i1 %240, label %241, label %248

241:
  %242 = getelementptr i8, ptr addrspace(1) %236, i16 %239
  %243 = mul i16 %238, 33
  %244 = load i8, ptr addrspace(1) %242
  %245 = zext i8 %244 to i16
  %246 = xor i16 %243, %245
  %247 = add i16 %239, 1
  br label %237

248:
  %249 = or i16 %238, 1
  %250 = load i16, ptr %83
  %251 = icmp ne i16 %250, 0
  br i1 %251, label %b77, label %b79

b76:
  call addrspace(1) void @N$EBND()
  unreachable

b77:
  %252 = add i16 %250, -1
  %253 = and i16 %249, %252
  %254 = getelementptr inbounds i8, ptr %2, i16 2
  %255 = getelementptr inbounds i8, ptr %2, i16 4
  %256 = addrspacecast ptr %2 to ptr addrspace(1)
  %257 = getelementptr inbounds i8, ptr %1, i16 2
  %258 = getelementptr inbounds i8, ptr %1, i16 4
  %259 = addrspacecast ptr %1 to ptr addrspace(1)
  br label %b80

b79:
  %260 = phi i16 [ 0, %248 ], [ %263, %b82 ]
  %261 = phi i8 [ 0, %248 ], [ %269, %b82 ]
  %262 = icmp ne i8 %261, 0
  br i1 %262, label %b94, label %b96

b80:
  %263 = phi i16 [ %253, %b77 ], [ %286, %b92 ]
  %264 = load i16, ptr %83
  %265 = icmp ult i16 %263, %264
  br i1 %265, label %b83, label %b84

b81:
  %266 = load i16, ptr %271
  %267 = icmp eq i16 %266, %249
  %268 = sext i1 %267 to i8
  br i1 %267, label %b89, label %b88

b82:
  %269 = phi i8 [ 0, %b83 ], [ -1, %b91 ]
  br label %b79

b83:
  %270 = mul i16 %263, 6
  %271 = getelementptr i8, ptr %40, i16 %270
  %272 = load i16, ptr %271
  %273 = icmp ne i16 %272, 0
  br i1 %273, label %b81, label %b82

b84:
  call addrspace(1) void @N$EBND()
  unreachable

b88:
  %274 = phi i8 [ %268, %b81 ], [ %284, %b89 ]
  %275 = icmp ne i8 %274, 0
  br i1 %275, label %b91, label %b92

b89:
  %276 = getelementptr i8, ptr %271, i16 2
  %277 = load ptr, ptr %276
  %278 = getelementptr i8, ptr %277, i16 -4
  %279 = load i16, ptr %278
  %280 = addrspacecast ptr %277 to ptr addrspace(1)
  store i16 %279, ptr %2, !tbaa !2
  store i16 %279, ptr %254, !tbaa !2
  store ptr addrspace(1) %280, ptr %255, !tbaa !2
  %281 = load i16, ptr %234
  store i16 %281, ptr %1, !tbaa !2
  store i16 %281, ptr %257, !tbaa !2
  store ptr addrspace(1) %236, ptr %258, !tbaa !2
  %282 = call addrspace(1) i8 @N$VCMP(ptr addrspace(1) %256, ptr addrspace(1) %259)
  %283 = icmp eq i8 %282, 0
  %284 = sext i1 %283 to i8
  br label %b88

b91:
  br label %b82

b92:
  %285 = add i16 %263, 1
  %286 = and i16 %285, %252
  br label %b80

b94:
  %287 = load i16, ptr %83
  %288 = icmp ult i16 %260, %287
  br i1 %288, label %b97, label %b98

b96:
  %289 = phi i16 [ 0, %b79 ], [ %306, %b97 ]
  call addrspace(1) void @N$PU2(i16 %232)
  %290 = getelementptr i8, ptr @$str9, i16 6
  call addrspace(1) void @N$PS(ptr %290)
  call addrspace(1) void @N$PI2(i16 %230)
  %291 = getelementptr i8, ptr @$str10, i16 6
  call addrspace(1) void @N$PS(ptr %291)
  call addrspace(1) void @N$PI2(i16 %289)
  call addrspace(1) void @N$PN()
  %292 = call addrspace(1) ptr @N$BGRW(ptr %9, i16 3, i16 2)
  %293 = getelementptr i8, ptr %292, i16 0
  store i8 12, ptr %293
  %294 = getelementptr i8, ptr %293, i16 1
  store i8 0, ptr %294
  %295 = getelementptr i8, ptr %292, i16 2
  store i8 3, ptr %295
  %296 = getelementptr i8, ptr %295, i16 1
  store i8 1, ptr %296
  %297 = getelementptr i8, ptr %292, i16 4
  store i8 12, ptr %297
  %298 = getelementptr i8, ptr %297, i16 1
  store i8 0, ptr %298
  %299 = call addrspace(1) ptr @N$DRES(ptr %9, i16 6)
  %300 = getelementptr i8, ptr %299, i16 -4
  %301 = load i16, ptr %300
  %302 = icmp ne i16 %301, 0
  br i1 %302, label %b99, label %b101

b97:
  %303 = mul i16 %260, 6
  %304 = getelementptr i8, ptr %40, i16 %303
  %305 = getelementptr i8, ptr %304, i16 4
  %306 = load i16, ptr %305
  br label %b96

b98:
  call addrspace(1) void @N$EBND()
  unreachable

b99:
  %307 = add i16 %301, -1
  %308 = and i16 %307, 1
  br label %b102

b101:
  %309 = phi i16 [ 0, %b96 ], [ %313, %b104 ]
  %310 = phi i8 [ 0, %b96 ], [ %319, %b104 ]
  %311 = xor i8 %310, -1
  %312 = icmp ne i8 %311, 0
  br i1 %312, label %b116, label %b118

b102:
  %313 = phi i16 [ %308, %b99 ], [ %331, %b114 ]
  %314 = load i16, ptr %300
  %315 = icmp ult i16 %313, %314
  br i1 %315, label %b105, label %b106

b103:
  %316 = load i16, ptr %321
  %317 = icmp eq i16 %316, 1
  %318 = sext i1 %317 to i8
  br i1 %317, label %b111, label %b110

b104:
  %319 = phi i8 [ 0, %b105 ], [ -1, %b113 ]
  br label %b101

b105:
  %320 = mul i16 %313, 6
  %321 = getelementptr i8, ptr %299, i16 %320
  %322 = load i16, ptr %321
  %323 = icmp ne i16 %322, 0
  br i1 %323, label %b103, label %b104

b106:
  call addrspace(1) void @N$EBND()
  unreachable

b110:
  %324 = phi i8 [ %318, %b103 ], [ %329, %b111 ]
  %325 = icmp ne i8 %324, 0
  br i1 %325, label %b113, label %b114

b111:
  %326 = getelementptr i8, ptr %321, i16 2
  %327 = load i8, ptr %326
  %328 = icmp eq i8 %327, 0
  %329 = sext i1 %328 to i8
  br label %b110

b113:
  br label %b104

b114:
  %330 = add i16 %313, 1
  %331 = and i16 %330, %307
  br label %b102

b116:
  %332 = load i16, ptr %300
  %333 = icmp ult i16 %309, %332
  br i1 %333, label %b119, label %b120

b118:
  %334 = icmp ne i8 %310, 0
  br i1 %334, label %b124, label %b123

b119:
  %335 = mul i16 %309, 6
  %336 = getelementptr i8, ptr %299, i16 %335
  store i16 1, ptr %336
  %337 = load i16, ptr %300
  %338 = icmp ult i16 %309, %337
  br i1 %338, label %b121, label %b122

b120:
  call addrspace(1) void @N$EBND()
  unreachable

b121:
  %339 = getelementptr i8, ptr %336, i16 2
  store i8 0, ptr %339
  br label %b118

b122:
  call addrspace(1) void @N$EBND()
  unreachable

b123:
  %340 = getelementptr i8, ptr %299, i16 -2
  %341 = load i16, ptr %340
  %342 = add i16 %341, 1
  store i16 %342, ptr %340
  br label %b124

b124:
  %343 = load i16, ptr %300
  %344 = icmp ult i16 %309, %343
  br i1 %344, label %b125, label %b126

b125:
  %345 = mul i16 %309, 6
  %346 = getelementptr i8, ptr %299, i16 %345
  %347 = getelementptr i8, ptr %346, i16 4
  store i16 0, ptr %347
  %348 = call addrspace(1) ptr @N$DRES(ptr %299, i16 6)
  %349 = getelementptr i8, ptr %348, i16 -4
  %350 = load i16, ptr %349
  %351 = icmp ne i16 %350, 0
  br i1 %351, label %b127, label %b129

b126:
  call addrspace(1) void @N$EBND()
  unreachable

b127:
  %352 = add i16 %350, -1
  %353 = and i16 %352, 1
  br label %b130

b129:
  %354 = phi i16 [ 0, %b125 ], [ %358, %b132 ]
  %355 = phi i8 [ 0, %b125 ], [ %364, %b132 ]
  %356 = xor i8 %355, -1
  %357 = icmp ne i8 %356, 0
  br i1 %357, label %b144, label %b146

b130:
  %358 = phi i16 [ %353, %b127 ], [ %376, %b142 ]
  %359 = load i16, ptr %349
  %360 = icmp ult i16 %358, %359
  br i1 %360, label %b133, label %b134

b131:
  %361 = load i16, ptr %366
  %362 = icmp eq i16 %361, 1
  %363 = sext i1 %362 to i8
  br i1 %362, label %b139, label %b138

b132:
  %364 = phi i8 [ 0, %b133 ], [ -1, %b141 ]
  br label %b129

b133:
  %365 = mul i16 %358, 6
  %366 = getelementptr i8, ptr %348, i16 %365
  %367 = load i16, ptr %366
  %368 = icmp ne i16 %367, 0
  br i1 %368, label %b131, label %b132

b134:
  call addrspace(1) void @N$EBND()
  unreachable

b138:
  %369 = phi i8 [ %363, %b131 ], [ %374, %b139 ]
  %370 = icmp ne i8 %369, 0
  br i1 %370, label %b141, label %b142

b139:
  %371 = getelementptr i8, ptr %366, i16 2
  %372 = load i8, ptr %371
  %373 = icmp eq i8 %372, 1
  %374 = sext i1 %373 to i8
  br label %b138

b141:
  br label %b132

b142:
  %375 = add i16 %358, 1
  %376 = and i16 %375, %352
  br label %b130

b144:
  %377 = load i16, ptr %349
  %378 = icmp ult i16 %354, %377
  br i1 %378, label %b147, label %b148

b146:
  %379 = icmp ne i8 %355, 0
  br i1 %379, label %b152, label %b151

b147:
  %380 = mul i16 %354, 6
  %381 = getelementptr i8, ptr %348, i16 %380
  store i16 1, ptr %381
  %382 = load i16, ptr %349
  %383 = icmp ult i16 %354, %382
  br i1 %383, label %b149, label %b150

b148:
  call addrspace(1) void @N$EBND()
  unreachable

b149:
  %384 = getelementptr i8, ptr %381, i16 2
  store i8 1, ptr %384
  br label %b146

b150:
  call addrspace(1) void @N$EBND()
  unreachable

b151:
  %385 = getelementptr i8, ptr %348, i16 -2
  %386 = load i16, ptr %385
  %387 = add i16 %386, 1
  store i16 %387, ptr %385
  br label %b152

b152:
  %388 = load i16, ptr %349
  %389 = icmp ult i16 %354, %388
  br i1 %389, label %b153, label %b154

b153:
  %390 = mul i16 %354, 6
  %391 = getelementptr i8, ptr %348, i16 %390
  %392 = getelementptr i8, ptr %391, i16 4
  store i16 0, ptr %392
  %393 = getelementptr i8, ptr %292, i16 -4
  %394 = load i16, ptr %393
  %395 = getelementptr inbounds i8, ptr %0, i16 1
  br label %b155

b154:
  call addrspace(1) void @N$EBND()
  unreachable

b155:
  %396 = phi ptr [ %348, %b153 ], [ %402, %b213 ]
  %397 = phi ptr [ %9, %b153 ], [ %457, %b213 ]
  %398 = phi i16 [ 0, %b153 ], [ %521, %b213 ]
  %399 = icmp ult i16 %398, %394
  br i1 %399, label %b156, label %410

b156:
  %400 = shl i16 %398, 1
  %401 = getelementptr i8, ptr %292, i16 %400
  %402 = call addrspace(1) ptr @N$DRES(ptr %396, i16 6)
  %403 = getelementptr i8, ptr %401, i16 1
  %404 = load i8, ptr %403
  %405 = zext i8 %404 to i16
  %406 = or i16 %405, 1
  %407 = getelementptr i8, ptr %402, i16 -4
  %408 = load i16, ptr %407
  %409 = icmp ne i16 %408, 0
  br i1 %409, label %b159, label %b161

410:
  %411 = getelementptr i8, ptr %396, i16 -4
  %412 = load i16, ptr %411
  %413 = icmp ne i16 %412, 0
  br i1 %413, label %b215, label %b217

b159:
  %414 = add i16 %408, -1
  %415 = and i16 %406, %414
  br label %b162

b161:
  %416 = phi i16 [ 0, %b156 ], [ %420, %b164 ]
  %417 = phi i8 [ 0, %b156 ], [ %426, %b164 ]
  %418 = xor i8 %417, -1
  %419 = icmp ne i8 %418, 0
  br i1 %419, label %b176, label %b178

b162:
  %420 = phi i16 [ %415, %b159 ], [ %438, %b174 ]
  %421 = load i16, ptr %407
  %422 = icmp ult i16 %420, %421
  br i1 %422, label %b165, label %b166

b163:
  %423 = load i16, ptr %428
  %424 = icmp eq i16 %423, %406
  %425 = sext i1 %424 to i8
  br i1 %424, label %b171, label %b170

b164:
  %426 = phi i8 [ 0, %b165 ], [ -1, %b173 ]
  br label %b161

b165:
  %427 = mul i16 %420, 6
  %428 = getelementptr i8, ptr %402, i16 %427
  %429 = load i16, ptr %428
  %430 = icmp ne i16 %429, 0
  br i1 %430, label %b163, label %b164

b166:
  call addrspace(1) void @N$EBND()
  unreachable

b170:
  %431 = phi i8 [ %425, %b163 ], [ %436, %b171 ]
  %432 = icmp ne i8 %431, 0
  br i1 %432, label %b173, label %b174

b171:
  %433 = getelementptr i8, ptr %428, i16 2
  %434 = load i8, ptr %433
  %435 = icmp eq i8 %434, %404
  %436 = sext i1 %435 to i8
  br label %b170

b173:
  br label %b164

b174:
  %437 = add i16 %420, 1
  %438 = and i16 %437, %414
  br label %b162

b176:
  %439 = load i16, ptr %407
  %440 = icmp ult i16 %416, %439
  br i1 %440, label %b179, label %b180

b178:
  %441 = icmp ne i8 %417, 0
  br i1 %441, label %b184, label %b183

b179:
  %442 = mul i16 %416, 6
  %443 = getelementptr i8, ptr %402, i16 %442
  store i16 %406, ptr %443
  %444 = load i16, ptr %407
  %445 = icmp ult i16 %416, %444
  br i1 %445, label %b181, label %b182

b180:
  call addrspace(1) void @N$EBND()
  unreachable

b181:
  %446 = getelementptr i8, ptr %443, i16 2
  store i8 %404, ptr %446
  br label %b178

b182:
  call addrspace(1) void @N$EBND()
  unreachable

b183:
  %447 = getelementptr i8, ptr %402, i16 -2
  %448 = load i16, ptr %447
  %449 = add i16 %448, 1
  store i16 %449, ptr %447
  br label %b184

b184:
  %450 = load i16, ptr %407
  %451 = icmp ult i16 %416, %450
  br i1 %451, label %b185, label %b186

b185:
  %452 = mul i16 %416, 6
  %453 = getelementptr i8, ptr %402, i16 %452
  %454 = getelementptr i8, ptr %453, i16 4
  %455 = load i16, ptr %454
  %456 = add i16 %455, 1
  store i16 %456, ptr %454
  %457 = call addrspace(1) ptr @N$DRES(ptr %397, i16 6)
  %458 = load i8, ptr %401
  %459 = load i8, ptr %403
  store i8 %458, ptr %0, !tbaa !2
  store i8 %459, ptr %395, !tbaa !2
  %460 = zext i8 %458 to i16
  %461 = mul i16 %460, 31
  %462 = zext i8 %459 to i16
  %463 = add i16 %461, %462
  %464 = or i16 %463, 1
  %465 = getelementptr i8, ptr %457, i16 -4
  %466 = load i16, ptr %465
  %467 = icmp ne i16 %466, 0
  br i1 %467, label %b187, label %b189

b186:
  call addrspace(1) void @N$EBND()
  unreachable

b187:
  %468 = add i16 %466, -1
  %469 = and i16 %464, %468
  br label %b190

b189:
  %470 = phi i16 [ 0, %b185 ], [ %474, %b192 ]
  %471 = phi i8 [ 0, %b185 ], [ %480, %b192 ]
  %472 = xor i8 %471, -1
  %473 = icmp ne i8 %472, 0
  br i1 %473, label %b204, label %b206

b190:
  %474 = phi i16 [ %469, %b187 ], [ %501, %b202 ]
  %475 = load i16, ptr %465
  %476 = icmp ult i16 %474, %475
  br i1 %476, label %b193, label %b194

b191:
  %477 = load i16, ptr %482
  %478 = icmp eq i16 %477, %464
  %479 = sext i1 %478 to i8
  br i1 %478, label %b199, label %b198

b192:
  %480 = phi i8 [ 0, %b193 ], [ -1, %b201 ]
  br label %b189

b193:
  %481 = mul i16 %474, 6
  %482 = getelementptr i8, ptr %457, i16 %481
  %483 = load i16, ptr %482
  %484 = icmp ne i16 %483, 0
  br i1 %484, label %b191, label %b192

b194:
  call addrspace(1) void @N$EBND()
  unreachable

b198:
  %485 = phi i8 [ %479, %b191 ], [ %499, %498 ]
  %486 = icmp ne i8 %485, 0
  br i1 %486, label %b201, label %b202

b199:
  %487 = getelementptr i8, ptr %482, i16 2
  %488 = addrspacecast ptr %487 to ptr addrspace(1)
  %489 = load i8, ptr addrspace(1) %488
  %490 = zext i8 %489 to i16
  %491 = icmp eq i16 %490, %460
  %492 = sext i1 %491 to i8
  br i1 %491, label %493, label %498

493:
  %494 = getelementptr i8, ptr addrspace(1) %488, i16 1
  %495 = load i8, ptr addrspace(1) %494
  %496 = icmp eq i8 %495, %459
  %497 = sext i1 %496 to i8
  br label %498

498:
  %499 = phi i8 [ %492, %b199 ], [ %497, %493 ]
  br label %b198

b201:
  br label %b192

b202:
  %500 = add i16 %474, 1
  %501 = and i16 %500, %468
  br label %b190

b204:
  %502 = load i16, ptr %465
  %503 = icmp ult i16 %470, %502
  br i1 %503, label %b207, label %b208

b206:
  %504 = icmp ne i8 %471, 0
  br i1 %504, label %b212, label %b211

b207:
  %505 = mul i16 %470, 6
  %506 = getelementptr i8, ptr %457, i16 %505
  store i16 %464, ptr %506
  %507 = load i16, ptr %465
  %508 = icmp ult i16 %470, %507
  br i1 %508, label %b209, label %b210

b208:
  call addrspace(1) void @N$EBND()
  unreachable

b209:
  %509 = load i8, ptr %0, !tbaa !2
  %510 = load i8, ptr %395, !tbaa !2
  %511 = getelementptr i8, ptr %506, i16 2
  store i8 %509, ptr %511
  %512 = getelementptr i8, ptr %506, i16 3
  store i8 %510, ptr %512
  br label %b206

b210:
  call addrspace(1) void @N$EBND()
  unreachable

b211:
  %513 = getelementptr i8, ptr %457, i16 -2
  %514 = load i16, ptr %513
  %515 = add i16 %514, 1
  store i16 %515, ptr %513
  br label %b212

b212:
  %516 = load i16, ptr %465
  %517 = icmp ult i16 %470, %516
  br i1 %517, label %b213, label %b214

b213:
  %518 = mul i16 %470, 6
  %519 = getelementptr i8, ptr %457, i16 %518
  %520 = getelementptr i8, ptr %519, i16 4
  store i8 -1, ptr %520
  %521 = add i16 %398, 1
  br label %b155

b214:
  call addrspace(1) void @N$EBND()
  unreachable

b215:
  %522 = add i16 %412, -1
  %523 = and i16 %522, 1
  br label %b218

b217:
  %524 = phi i16 [ 0, %410 ], [ %527, %b220 ]
  %525 = phi i8 [ 0, %410 ], [ %533, %b220 ]
  %526 = icmp ne i8 %525, 0
  br i1 %526, label %b232, label %b233

b218:
  %527 = phi i16 [ %523, %b215 ], [ %545, %b230 ]
  %528 = load i16, ptr %411
  %529 = icmp ult i16 %527, %528
  br i1 %529, label %b221, label %b222

b219:
  %530 = load i16, ptr %535
  %531 = icmp eq i16 %530, 1
  %532 = sext i1 %531 to i8
  br i1 %531, label %b227, label %b226

b220:
  %533 = phi i8 [ 0, %b221 ], [ -1, %b229 ]
  br label %b217

b221:
  %534 = mul i16 %527, 6
  %535 = getelementptr i8, ptr %396, i16 %534
  %536 = load i16, ptr %535
  %537 = icmp ne i16 %536, 0
  br i1 %537, label %b219, label %b220

b222:
  call addrspace(1) void @N$EBND()
  unreachable

b226:
  %538 = phi i8 [ %532, %b219 ], [ %543, %b227 ]
  %539 = icmp ne i8 %538, 0
  br i1 %539, label %b229, label %b230

b227:
  %540 = getelementptr i8, ptr %535, i16 2
  %541 = load i8, ptr %540
  %542 = icmp eq i8 %541, 0
  %543 = sext i1 %542 to i8
  br label %b226

b229:
  br label %b220

b230:
  %544 = add i16 %527, 1
  %545 = and i16 %544, %522
  br label %b218

b232:
  %546 = load i16, ptr %411
  %547 = icmp ult i16 %524, %546
  br i1 %547, label %b234, label %b235

b233:
  call addrspace(1) void @N$EKEY()
  unreachable

b234:
  %548 = mul i16 %524, 6
  %549 = getelementptr i8, ptr %396, i16 %548
  %550 = getelementptr i8, ptr %549, i16 4
  %551 = load i16, ptr %550
  %552 = getelementptr i8, ptr %397, i16 -2
  %553 = load i16, ptr %552
  %554 = getelementptr i8, ptr %397, i16 -4
  %555 = load i16, ptr %554
  %556 = icmp ne i16 %555, 0
  br i1 %556, label %b236, label %b238

b235:
  call addrspace(1) void @N$EBND()
  unreachable

b236:
  %557 = add i16 %555, -1
  %558 = and i16 %557, 373
  br label %b239

b238:
  %559 = phi i8 [ 0, %b234 ], [ %569, %b241 ]
  %560 = getelementptr i8, ptr @$str11, i16 6
  call addrspace(1) void @N$PS(ptr %560)
  call addrspace(1) void @N$PI2(i16 %551)
  %561 = getelementptr i8, ptr @$str12, i16 6
  call addrspace(1) void @N$PS(ptr %561)
  call addrspace(1) void @N$PU2(i16 %553)
  %562 = getelementptr i8, ptr @$str13, i16 6
  call addrspace(1) void @N$PS(ptr %562)
  call addrspace(1) void @N$PB(i8 %559)
  call addrspace(1) void @N$PN()
  br label %b253

b239:
  %563 = phi i16 [ %558, %b236 ], [ %590, %b251 ]
  %564 = load i16, ptr %554
  %565 = icmp ult i16 %563, %564
  br i1 %565, label %b242, label %b243

b240:
  %566 = load i16, ptr %571
  %567 = icmp eq i16 %566, 373
  %568 = sext i1 %567 to i8
  br i1 %567, label %b248, label %b247

b241:
  %569 = phi i8 [ 0, %b242 ], [ -1, %b250 ]
  br label %b238

b242:
  %570 = mul i16 %563, 6
  %571 = getelementptr i8, ptr %397, i16 %570
  %572 = load i16, ptr %571
  %573 = icmp ne i16 %572, 0
  br i1 %573, label %b240, label %b241

b243:
  call addrspace(1) void @N$EBND()
  unreachable

b247:
  %574 = phi i8 [ %568, %b240 ], [ %588, %587 ]
  %575 = icmp ne i8 %574, 0
  br i1 %575, label %b250, label %b251

b248:
  %576 = getelementptr i8, ptr %571, i16 2
  %577 = addrspacecast ptr %576 to ptr addrspace(1)
  %578 = load i8, ptr addrspace(1) %577
  %579 = zext i8 %578 to i16
  %580 = icmp eq i16 %579, 12
  %581 = sext i1 %580 to i8
  br i1 %580, label %582, label %587

582:
  %583 = getelementptr i8, ptr addrspace(1) %577, i16 1
  %584 = load i8, ptr addrspace(1) %583
  %585 = icmp eq i8 %584, 0
  %586 = sext i1 %585 to i8
  br label %587

587:
  %588 = phi i8 [ %581, %b248 ], [ %586, %582 ]
  br label %b247

b250:
  br label %b241

b251:
  %589 = add i16 %563, 1
  %590 = and i16 %589, %557
  br label %b239

b253:
  %591 = phi ptr [ %9, %b238 ], [ %594, %b283 ]
  %592 = phi i16 [ 0, %b238 ], [ %648, %b283 ]
  %593 = icmp slt i16 %592, 40
  br i1 %593, label %b254, label %b256

b254:
  %594 = call addrspace(1) ptr @N$DRES(ptr %591, i16 6)
  %595 = or i16 %592, 1
  %596 = getelementptr i8, ptr %594, i16 -4
  %597 = load i16, ptr %596
  %598 = icmp ne i16 %597, 0
  br i1 %598, label %b257, label %b259

b256:
  %599 = getelementptr i8, ptr %591, i16 -2
  %600 = load i16, ptr %599
  call addrspace(1) void @N$PU2(i16 %600)
  %601 = getelementptr i8, ptr @$str14, i16 6
  call addrspace(1) void @N$PS(ptr %601)
  call addrspace(1) void @N$PI2(i16 37)
  %602 = getelementptr i8, ptr @$str15, i16 6
  call addrspace(1) void @N$PS(ptr %602)
  %603 = getelementptr i8, ptr %591, i16 -4
  %604 = load i16, ptr %603
  %605 = icmp ne i16 %604, 0
  br i1 %605, label %b285, label %b287

b257:
  %606 = add i16 %597, -1
  %607 = and i16 %595, %606
  br label %b260

b259:
  %608 = phi i16 [ 0, %b254 ], [ %612, %b262 ]
  %609 = phi i8 [ 0, %b254 ], [ %618, %b262 ]
  %610 = xor i8 %609, -1
  %611 = icmp ne i8 %610, 0
  br i1 %611, label %b274, label %b276

b260:
  %612 = phi i16 [ %607, %b257 ], [ %630, %b272 ]
  %613 = load i16, ptr %596
  %614 = icmp ult i16 %612, %613
  br i1 %614, label %b263, label %b264

b261:
  %615 = load i16, ptr %620
  %616 = icmp eq i16 %615, %595
  %617 = sext i1 %616 to i8
  br i1 %616, label %b269, label %b268

b262:
  %618 = phi i8 [ 0, %b263 ], [ -1, %b271 ]
  br label %b259

b263:
  %619 = mul i16 %612, 6
  %620 = getelementptr i8, ptr %594, i16 %619
  %621 = load i16, ptr %620
  %622 = icmp ne i16 %621, 0
  br i1 %622, label %b261, label %b262

b264:
  call addrspace(1) void @N$EBND()
  unreachable

b268:
  %623 = phi i8 [ %617, %b261 ], [ %628, %b269 ]
  %624 = icmp ne i8 %623, 0
  br i1 %624, label %b271, label %b272

b269:
  %625 = getelementptr i8, ptr %620, i16 2
  %626 = load i16, ptr %625
  %627 = icmp eq i16 %626, %592
  %628 = sext i1 %627 to i8
  br label %b268

b271:
  br label %b262

b272:
  %629 = add i16 %612, 1
  %630 = and i16 %629, %606
  br label %b260

b274:
  %631 = load i16, ptr %596
  %632 = icmp ult i16 %608, %631
  br i1 %632, label %b277, label %b278

b276:
  %633 = icmp ne i8 %609, 0
  br i1 %633, label %b282, label %b281

b277:
  %634 = mul i16 %608, 6
  %635 = getelementptr i8, ptr %594, i16 %634
  store i16 %595, ptr %635
  %636 = load i16, ptr %596
  %637 = icmp ult i16 %608, %636
  br i1 %637, label %b279, label %b280

b278:
  call addrspace(1) void @N$EBND()
  unreachable

b279:
  %638 = getelementptr i8, ptr %635, i16 2
  store i16 %592, ptr %638
  br label %b276

b280:
  call addrspace(1) void @N$EBND()
  unreachable

b281:
  %639 = getelementptr i8, ptr %594, i16 -2
  %640 = load i16, ptr %639
  %641 = add i16 %640, 1
  store i16 %641, ptr %639
  br label %b282

b282:
  %642 = load i16, ptr %596
  %643 = icmp ult i16 %608, %642
  br i1 %643, label %b283, label %b284

b283:
  %644 = mul i16 %608, 6
  %645 = getelementptr i8, ptr %594, i16 %644
  %646 = mul i16 %592, %592
  %647 = getelementptr i8, ptr %645, i16 4
  store i16 %646, ptr %647
  %648 = add i16 %592, 1
  br label %b253

b284:
  call addrspace(1) void @N$EBND()
  unreachable

b285:
  %649 = add i16 %604, -1
  %650 = and i16 %649, 37
  br label %b288

b287:
  %651 = phi i16 [ 0, %b256 ], [ %654, %b290 ]
  %652 = phi i8 [ 0, %b256 ], [ %660, %b290 ]
  %653 = icmp ne i8 %652, 0
  br i1 %653, label %b302, label %b303

b288:
  %654 = phi i16 [ %650, %b285 ], [ %672, %b300 ]
  %655 = load i16, ptr %603
  %656 = icmp ult i16 %654, %655
  br i1 %656, label %b291, label %b292

b289:
  %657 = load i16, ptr %662
  %658 = icmp eq i16 %657, 37
  %659 = sext i1 %658 to i8
  br i1 %658, label %b297, label %b296

b290:
  %660 = phi i8 [ 0, %b291 ], [ -1, %b299 ]
  br label %b287

b291:
  %661 = mul i16 %654, 6
  %662 = getelementptr i8, ptr %591, i16 %661
  %663 = load i16, ptr %662
  %664 = icmp ne i16 %663, 0
  br i1 %664, label %b289, label %b290

b292:
  call addrspace(1) void @N$EBND()
  unreachable

b296:
  %665 = phi i8 [ %659, %b289 ], [ %670, %b297 ]
  %666 = icmp ne i8 %665, 0
  br i1 %666, label %b299, label %b300

b297:
  %667 = getelementptr i8, ptr %662, i16 2
  %668 = load i16, ptr %667
  %669 = icmp eq i16 %668, 37
  %670 = sext i1 %669 to i8
  br label %b296

b299:
  br label %b290

b300:
  %671 = add i16 %654, 1
  %672 = and i16 %671, %649
  br label %b288

b302:
  %673 = load i16, ptr %603
  %674 = icmp ult i16 %651, %673
  br i1 %674, label %b304, label %b305

b303:
  call addrspace(1) void @N$EKEY()
  unreachable

b304:
  %675 = mul i16 %651, 6
  %676 = getelementptr i8, ptr %591, i16 %675
  %677 = getelementptr i8, ptr %676, i16 4
  %678 = load i16, ptr %677
  call addrspace(1) void @N$PI2(i16 %678)
  call addrspace(1) void @N$PN()
  call addrspace(1) void @N$BDRP(ptr %591)
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$BDRP(ptr %397)
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$BDRP(ptr %396)
  call addrspace(1) void @N$BDRP(ptr null)
  call addrspace(1) void @N$BDRP(ptr %292)
  call addrspace(1) void @N$BDRP(ptr %233)
  call addrspace(1) void @N$BDRP(ptr %12)
  %679 = icmp ne ptr %40, null
  br i1 %679, label %b307, label %b306

b305:
  call addrspace(1) void @N$EBND()
  unreachable

b306:
  call addrspace(1) void @N$BDRP(ptr %40)
  call addrspace(1) void @N$BDRP(ptr null)
  %680 = icmp ne ptr %10, null
  br i1 %680, label %b317, label %b316

b307:
  %681 = load i16, ptr %83
  br label %b308

b308:
  %682 = phi i16 [ 0, %b307 ], [ %688, %b310 ]
  %683 = icmp ult i16 %682, %681
  br i1 %683, label %b310, label %b306

b310:
  %684 = mul i16 %682, 6
  %685 = getelementptr i8, ptr %40, i16 %684
  %686 = getelementptr i8, ptr %685, i16 2
  %687 = load ptr, ptr %686
  call addrspace(1) void @N$BDRP(ptr %687)
  %688 = add i16 %682, 1
  br label %b308

b316:
  call addrspace(1) void @N$BDRP(ptr %10)
  ret i16 0

b317:
  %689 = load i16, ptr %26
  br label %b318

b318:
  %690 = phi i16 [ 0, %b317 ], [ %695, %b320 ]
  %691 = icmp ult i16 %690, %689
  br i1 %691, label %b320, label %b316

b320:
  %692 = shl i16 %690, 1
  %693 = getelementptr i8, ptr %10, i16 %692
  %694 = load ptr, ptr %693
  call addrspace(1) void @N$BDRP(ptr %694)
  %695 = add i16 %690, 1
  br label %b318
}

define internal i8 @i16.eq(i16 %0, i16 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = icmp eq i16 %0, %1
  %3 = sext i1 %2 to i8
  ret i8 %3
}

define internal i16 @i16.hash(i16 %0) addrspace(1) memory(none) willreturn {
b1:
  ret i16 %0
}

define internal i8 @Card.eq(ptr addrspace(1) %0, ptr addrspace(1) %1) addrspace(1) memory(argmem: read) willreturn {
b1:
  %2 = load i8, ptr addrspace(1) %0
  %3 = load i8, ptr addrspace(1) %1
  %4 = zext i8 %2 to i16
  %5 = zext i8 %3 to i16
  %6 = icmp eq i16 %4, %5
  %7 = sext i1 %6 to i8
  br i1 %6, label %b2, label %b3

b2:
  %8 = getelementptr i8, ptr addrspace(1) %0, i16 1
  %9 = load i8, ptr addrspace(1) %8
  %10 = getelementptr i8, ptr addrspace(1) %1, i16 1
  %11 = load i8, ptr addrspace(1) %10
  %12 = icmp eq i8 %9, %11
  %13 = sext i1 %12 to i8
  br label %b3

b3:
  %14 = phi i8 [ %7, %b1 ], [ %13, %b2 ]
  ret i8 %14
}

define internal i8 @u8.eq(i8 %0, i8 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = zext i8 %0 to i16
  %3 = zext i8 %1 to i16
  %4 = icmp eq i16 %2, %3
  %5 = sext i1 %4 to i8
  ret i8 %5
}

define internal i16 @Card.hash(ptr addrspace(1) %0) addrspace(1) memory(argmem: read) willreturn {
b1:
  %1 = load i8, ptr addrspace(1) %0
  %2 = zext i8 %1 to i16
  %3 = mul i16 %2, 31
  %4 = getelementptr i8, ptr addrspace(1) %0, i16 1
  %5 = load i8, ptr addrspace(1) %4
  %6 = zext i8 %5 to i16
  %7 = add i16 %3, %6
  ret i16 %7
}

define internal i16 @u8.hash(i8 %0) addrspace(1) memory(none) willreturn {
b1:
  %1 = zext i8 %0 to i16
  ret i16 %1
}

define internal i8 @Suit.eq(i8 %0, i8 %1) addrspace(1) memory(none) willreturn {
b1:
  %2 = icmp eq i8 %0, %1
  %3 = sext i1 %2 to i8
  ret i8 %3
}

define internal i16 @Suit.hash(i8 %0) addrspace(1) memory(none) willreturn {
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

define internal i16 @string.hash(ptr addrspace(1) noalias readonly dereferenceable(8) %0) addrspace(1) willreturn {
b1:
  %1 = load i16, ptr addrspace(1) %0
  %2 = getelementptr i8, ptr addrspace(1) %0, i16 4
  %3 = load ptr addrspace(1), ptr addrspace(1) %2
  br label %b2

b2:
  %4 = phi i16 [ 5381, %b1 ], [ %11, %b3 ]
  %5 = phi i16 [ 0, %b1 ], [ %12, %b3 ]
  %6 = icmp ult i16 %5, %1
  br i1 %6, label %b3, label %b5

b3:
  %7 = getelementptr i8, ptr addrspace(1) %3, i16 %5
  %8 = mul i16 %4, 33
  %9 = load i8, ptr addrspace(1) %7
  %10 = zext i8 %9 to i16
  %11 = xor i16 %8, %10
  %12 = add i16 %5, 1
  br label %b2

b5:
  ret i16 %4
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
