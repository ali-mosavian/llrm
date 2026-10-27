target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"

define internal cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca i16
  %1 = alloca i16
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca i16
  %8 = alloca i8
  %9 = alloca i16
  %10 = alloca i16
  %11 = alloca i16
  %12 = alloca i16
  %13 = alloca i16
  %14 = alloca i8
  %15 = alloca i16
  %16 = alloca i16
  %17 = alloca i16
  %18 = alloca ptr
  %19 = alloca i16
  %20 = alloca i8
  %21 = alloca i16
  %22 = alloca i16
  %23 = alloca i16
  %24 = alloca i16
  %25 = alloca ptr
  %26 = alloca i16
  %27 = alloca i16
  %28 = alloca [8 x i8]
  store i16 4, ptr %26, !tbaa !2
  store i16 4, ptr %27, !tbaa !2
  %29 = getelementptr inbounds i16, ptr %28, i16 0
  store i16 1, ptr %29, !tbaa !2
  %30 = getelementptr inbounds i16, ptr %28, i16 1
  store i16 2, ptr %30, !tbaa !2
  %31 = getelementptr inbounds i16, ptr %28, i16 2
  store i16 1, ptr %31, !tbaa !2
  %32 = getelementptr inbounds i16, ptr %28, i16 3
  store i16 3, ptr %32, !tbaa !2
  %33 = getelementptr i8, ptr @$str1, i16 6
  store ptr %33, ptr %25, !tbaa !2
  store i16 0, ptr %24, !tbaa !2
  br label %b2

b2:
  %34 = load i16, ptr %24, !tbaa !2
  %35 = icmp ult i16 %34, 4
  br i1 %35, label %b3, label %b5

b3:
  %36 = load ptr, ptr %25, !tbaa !2
  %37 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %36, i16 6)
  store ptr %37, ptr %25, !tbaa !2
  %38 = sub i16 %34, 0
  %39 = getelementptr inbounds i16, ptr %28, i16 %38
  %40 = load i16, ptr %39, !tbaa !2
  store i16 %40, ptr %23, !tbaa !2
  %41 = load i16, ptr %23, !tbaa !2
  %42 = call cc 1005 addrspace(1) i16 @i16.hash(i16 %41)
  %43 = or i16 %42, 1
  store i16 %43, ptr %22, !tbaa !2
  store i16 0, ptr %21, !tbaa !2
  store i8 0, ptr %20, !tbaa !2
  %44 = getelementptr i8, ptr %37, i16 -4
  %45 = load i16, ptr %44
  %46 = icmp ne i16 %45, 0
  br i1 %46, label %b6, label %b7

b4:
  %47 = load i16, ptr %24, !tbaa !2
  %48 = add i16 %47, 1
  store i16 %48, ptr %24, !tbaa !2
  br label %b2

b5:
  %49 = load ptr, ptr %25, !tbaa !2
  store ptr null, ptr %25, !tbaa !2
  store ptr %49, ptr %18, !tbaa !2
  %50 = load ptr, ptr %18, !tbaa !2
  store i16 1, ptr %17, !tbaa !2
  %51 = load i16, ptr %17, !tbaa !2
  %52 = call cc 1005 addrspace(1) i16 @i16.hash(i16 %51)
  %53 = or i16 %52, 1
  store i16 %53, ptr %16, !tbaa !2
  store i16 0, ptr %15, !tbaa !2
  store i8 0, ptr %14, !tbaa !2
  %54 = getelementptr i8, ptr %50, i16 -4
  %55 = load i16, ptr %54
  %56 = icmp ne i16 %55, 0
  br i1 %56, label %b33, label %b34

b6:
  %57 = getelementptr i8, ptr %37, i16 -4
  %58 = load i16, ptr %57
  %59 = sub i16 %58, 1
  store i16 %59, ptr %19, !tbaa !2
  %60 = load i16, ptr %22, !tbaa !2
  %61 = load i16, ptr %19, !tbaa !2
  %62 = and i16 %60, %61
  store i16 %62, ptr %21, !tbaa !2
  br label %b9

b7:
  br label %b8

b8:
  %63 = load i8, ptr %20, !tbaa !2, !range !5
  %64 = icmp ne i8 %63, 0
  br i1 %64, label %b23, label %b22

b9:
  %65 = load i16, ptr %21, !tbaa !2
  %66 = getelementptr i8, ptr %37, i16 -4
  %67 = load i16, ptr %66
  %68 = icmp ult i16 %65, %67
  br i1 %68, label %b12, label %b13

b10:
  %69 = load i16, ptr %21, !tbaa !2
  %70 = getelementptr i8, ptr %37, i16 -4
  %71 = load i16, ptr %70
  %72 = icmp ult i16 %69, %71
  br i1 %72, label %b18, label %b19

b11:
  br label %b8

b12:
  %73 = mul i16 %65, 6
  %74 = getelementptr inbounds i8, ptr %37, i16 %73
  %75 = load i16, ptr %74
  %76 = icmp ne i16 %75, 0
  br i1 %76, label %b10, label %b11

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b14:
  store i8 1, ptr %20, !tbaa !2
  br label %b11

b15:
  br label %b16

b16:
  %77 = load i16, ptr %21, !tbaa !2
  %78 = add i16 %77, 1
  %79 = load i16, ptr %19, !tbaa !2
  %80 = and i16 %78, %79
  store i16 %80, ptr %21, !tbaa !2
  br label %b9

b17:
  %81 = load i16, ptr %21, !tbaa !2
  %82 = getelementptr i8, ptr %37, i16 -4
  %83 = load i16, ptr %82
  %84 = icmp ult i16 %81, %83
  br i1 %84, label %b20, label %b21

b18:
  %85 = mul i16 %69, 6
  %86 = getelementptr inbounds i8, ptr %37, i16 %85
  %87 = load i16, ptr %86
  %88 = load i16, ptr %22, !tbaa !2
  %89 = icmp eq i16 %87, %88
  br i1 %89, label %b17, label %b15

b19:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b20:
  %90 = mul i16 %81, 6
  %91 = getelementptr inbounds i8, ptr %37, i16 %90
  %92 = getelementptr i8, ptr %91, i16 2
  %93 = load i16, ptr %92
  %94 = load i16, ptr %23, !tbaa !2
  %95 = call cc 1005 addrspace(1) i8 @i16.eq(i16 %93, i16 %94)
  %96 = icmp ne i8 %95, 0
  br i1 %96, label %b14, label %b15

b21:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b22:
  %97 = load i16, ptr %21, !tbaa !2
  %98 = getelementptr i8, ptr %37, i16 -4
  %99 = load i16, ptr %98
  %100 = icmp ult i16 %97, %99
  br i1 %100, label %b25, label %b26

b23:
  br label %b24

b24:
  %101 = load i8, ptr %20, !tbaa !2, !range !6
  %102 = icmp ne i8 %101, 0
  br i1 %102, label %b30, label %b29

b25:
  %103 = mul i16 %97, 6
  %104 = getelementptr inbounds i8, ptr %37, i16 %103
  %105 = load i16, ptr %22, !tbaa !2
  store i16 %105, ptr %104
  %106 = load i16, ptr %21, !tbaa !2
  %107 = getelementptr i8, ptr %37, i16 -4
  %108 = load i16, ptr %107
  %109 = icmp ult i16 %106, %108
  br i1 %109, label %b27, label %b28

b26:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b27:
  %110 = mul i16 %106, 6
  %111 = getelementptr inbounds i8, ptr %37, i16 %110
  %112 = load i16, ptr %23, !tbaa !2
  %113 = getelementptr i8, ptr %111, i16 2
  store i16 %112, ptr %113
  br label %b24

b28:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b29:
  %114 = getelementptr i8, ptr %37, i16 -2
  %115 = load i16, ptr %114
  %116 = add i16 %115, 1
  %117 = getelementptr i8, ptr %37, i16 -2
  store i16 %116, ptr %117
  br label %b30

b30:
  %118 = load i16, ptr %21, !tbaa !2
  %119 = getelementptr i8, ptr %37, i16 -4
  %120 = load i16, ptr %119
  %121 = icmp ult i16 %118, %120
  br i1 %121, label %b31, label %b32

b31:
  %122 = mul i16 %118, 6
  %123 = getelementptr inbounds i8, ptr %37, i16 %122
  %124 = sub i16 %34, 0
  %125 = getelementptr inbounds i16, ptr %28, i16 %124
  %126 = load i16, ptr %125, !tbaa !2
  %127 = mul i16 %126, 10
  %128 = getelementptr i8, ptr %123, i16 4
  store i16 %127, ptr %128
  br label %b4

b32:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b33:
  %129 = getelementptr i8, ptr %50, i16 -4
  %130 = load i16, ptr %129
  %131 = sub i16 %130, 1
  store i16 %131, ptr %13, !tbaa !2
  %132 = load i16, ptr %16, !tbaa !2
  %133 = load i16, ptr %13, !tbaa !2
  %134 = and i16 %132, %133
  store i16 %134, ptr %15, !tbaa !2
  br label %b36

b34:
  br label %b35

b35:
  %135 = load i8, ptr %14, !tbaa !2, !range !7
  %136 = icmp ne i8 %135, 0
  br i1 %136, label %b49, label %b50

b36:
  %137 = load i16, ptr %15, !tbaa !2
  %138 = getelementptr i8, ptr %50, i16 -4
  %139 = load i16, ptr %138
  %140 = icmp ult i16 %137, %139
  br i1 %140, label %b39, label %b40

b37:
  %141 = load i16, ptr %15, !tbaa !2
  %142 = getelementptr i8, ptr %50, i16 -4
  %143 = load i16, ptr %142
  %144 = icmp ult i16 %141, %143
  br i1 %144, label %b45, label %b46

b38:
  br label %b35

b39:
  %145 = mul i16 %137, 6
  %146 = getelementptr inbounds i8, ptr %50, i16 %145
  %147 = load i16, ptr %146
  %148 = icmp ne i16 %147, 0
  br i1 %148, label %b37, label %b38

b40:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b41:
  store i8 1, ptr %14, !tbaa !2
  br label %b38

b42:
  br label %b43

b43:
  %149 = load i16, ptr %15, !tbaa !2
  %150 = add i16 %149, 1
  %151 = load i16, ptr %13, !tbaa !2
  %152 = and i16 %150, %151
  store i16 %152, ptr %15, !tbaa !2
  br label %b36

b44:
  %153 = load i16, ptr %15, !tbaa !2
  %154 = getelementptr i8, ptr %50, i16 -4
  %155 = load i16, ptr %154
  %156 = icmp ult i16 %153, %155
  br i1 %156, label %b47, label %b48

b45:
  %157 = mul i16 %141, 6
  %158 = getelementptr inbounds i8, ptr %50, i16 %157
  %159 = load i16, ptr %158
  %160 = load i16, ptr %16, !tbaa !2
  %161 = icmp eq i16 %159, %160
  br i1 %161, label %b44, label %b42

b46:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b47:
  %162 = mul i16 %153, 6
  %163 = getelementptr inbounds i8, ptr %50, i16 %162
  %164 = getelementptr i8, ptr %163, i16 2
  %165 = load i16, ptr %164
  %166 = load i16, ptr %17, !tbaa !2
  %167 = call cc 1005 addrspace(1) i8 @i16.eq(i16 %165, i16 %166)
  %168 = icmp ne i8 %167, 0
  br i1 %168, label %b41, label %b42

b48:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b49:
  %169 = load i16, ptr %15, !tbaa !2
  %170 = getelementptr i8, ptr %50, i16 -4
  %171 = load i16, ptr %170
  %172 = icmp ult i16 %169, %171
  br i1 %172, label %b52, label %b53

b50:
  store i16 0, ptr %12, !tbaa !2
  br label %b51

b51:
  %173 = load i16, ptr %12, !tbaa !2
  %174 = load ptr, ptr %18, !tbaa !2
  store i16 3, ptr %11, !tbaa !2
  %175 = load i16, ptr %11, !tbaa !2
  %176 = call cc 1005 addrspace(1) i16 @i16.hash(i16 %175)
  %177 = or i16 %176, 1
  store i16 %177, ptr %10, !tbaa !2
  store i16 0, ptr %9, !tbaa !2
  store i8 0, ptr %8, !tbaa !2
  %178 = getelementptr i8, ptr %174, i16 -4
  %179 = load i16, ptr %178
  %180 = icmp ne i16 %179, 0
  br i1 %180, label %b54, label %b55

b52:
  %181 = mul i16 %169, 6
  %182 = getelementptr inbounds i8, ptr %50, i16 %181
  %183 = getelementptr i8, ptr %182, i16 4
  %184 = load i16, ptr %183
  store i16 %184, ptr %12, !tbaa !2
  br label %b51

b53:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b54:
  %185 = getelementptr i8, ptr %174, i16 -4
  %186 = load i16, ptr %185
  %187 = sub i16 %186, 1
  store i16 %187, ptr %7, !tbaa !2
  %188 = load i16, ptr %10, !tbaa !2
  %189 = load i16, ptr %7, !tbaa !2
  %190 = and i16 %188, %189
  store i16 %190, ptr %9, !tbaa !2
  br label %b57

b55:
  br label %b56

b56:
  %191 = load i8, ptr %8, !tbaa !2, !range !8
  %192 = icmp ne i8 %191, 0
  br i1 %192, label %b70, label %b71

b57:
  %193 = load i16, ptr %9, !tbaa !2
  %194 = getelementptr i8, ptr %174, i16 -4
  %195 = load i16, ptr %194
  %196 = icmp ult i16 %193, %195
  br i1 %196, label %b60, label %b61

b58:
  %197 = load i16, ptr %9, !tbaa !2
  %198 = getelementptr i8, ptr %174, i16 -4
  %199 = load i16, ptr %198
  %200 = icmp ult i16 %197, %199
  br i1 %200, label %b66, label %b67

b59:
  br label %b56

b60:
  %201 = mul i16 %193, 6
  %202 = getelementptr inbounds i8, ptr %174, i16 %201
  %203 = load i16, ptr %202
  %204 = icmp ne i16 %203, 0
  br i1 %204, label %b58, label %b59

b61:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b62:
  store i8 1, ptr %8, !tbaa !2
  br label %b59

b63:
  br label %b64

b64:
  %205 = load i16, ptr %9, !tbaa !2
  %206 = add i16 %205, 1
  %207 = load i16, ptr %7, !tbaa !2
  %208 = and i16 %206, %207
  store i16 %208, ptr %9, !tbaa !2
  br label %b57

b65:
  %209 = load i16, ptr %9, !tbaa !2
  %210 = getelementptr i8, ptr %174, i16 -4
  %211 = load i16, ptr %210
  %212 = icmp ult i16 %209, %211
  br i1 %212, label %b68, label %b69

b66:
  %213 = mul i16 %197, 6
  %214 = getelementptr inbounds i8, ptr %174, i16 %213
  %215 = load i16, ptr %214
  %216 = load i16, ptr %10, !tbaa !2
  %217 = icmp eq i16 %215, %216
  br i1 %217, label %b65, label %b63

b67:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b68:
  %218 = mul i16 %209, 6
  %219 = getelementptr inbounds i8, ptr %174, i16 %218
  %220 = getelementptr i8, ptr %219, i16 2
  %221 = load i16, ptr %220
  %222 = load i16, ptr %11, !tbaa !2
  %223 = call cc 1005 addrspace(1) i8 @i16.eq(i16 %221, i16 %222)
  %224 = icmp ne i8 %223, 0
  br i1 %224, label %b62, label %b63

b69:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b70:
  %225 = load i16, ptr %9, !tbaa !2
  %226 = getelementptr i8, ptr %174, i16 -4
  %227 = load i16, ptr %226
  %228 = icmp ult i16 %225, %227
  br i1 %228, label %b73, label %b74

b71:
  store i16 0, ptr %6, !tbaa !2
  br label %b72

b72:
  %229 = load i16, ptr %6, !tbaa !2
  %230 = add i16 %173, %229
  %231 = load ptr, ptr %18, !tbaa !2
  store i16 9, ptr %5, !tbaa !2
  %232 = load i16, ptr %5, !tbaa !2
  %233 = call cc 1005 addrspace(1) i16 @i16.hash(i16 %232)
  %234 = or i16 %233, 1
  store i16 %234, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i8 0, ptr %2, !tbaa !2
  %235 = getelementptr i8, ptr %231, i16 -4
  %236 = load i16, ptr %235
  %237 = icmp ne i16 %236, 0
  br i1 %237, label %b75, label %b76

b73:
  %238 = mul i16 %225, 6
  %239 = getelementptr inbounds i8, ptr %174, i16 %238
  %240 = getelementptr i8, ptr %239, i16 4
  %241 = load i16, ptr %240
  store i16 %241, ptr %6, !tbaa !2
  br label %b72

b74:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b75:
  %242 = getelementptr i8, ptr %231, i16 -4
  %243 = load i16, ptr %242
  %244 = sub i16 %243, 1
  store i16 %244, ptr %1, !tbaa !2
  %245 = load i16, ptr %4, !tbaa !2
  %246 = load i16, ptr %1, !tbaa !2
  %247 = and i16 %245, %246
  store i16 %247, ptr %3, !tbaa !2
  br label %b78

b76:
  br label %b77

b77:
  %248 = load i8, ptr %2, !tbaa !2, !range !9
  %249 = icmp ne i8 %248, 0
  br i1 %249, label %b91, label %b92

b78:
  %250 = load i16, ptr %3, !tbaa !2
  %251 = getelementptr i8, ptr %231, i16 -4
  %252 = load i16, ptr %251
  %253 = icmp ult i16 %250, %252
  br i1 %253, label %b81, label %b82

b79:
  %254 = load i16, ptr %3, !tbaa !2
  %255 = getelementptr i8, ptr %231, i16 -4
  %256 = load i16, ptr %255
  %257 = icmp ult i16 %254, %256
  br i1 %257, label %b87, label %b88

b80:
  br label %b77

b81:
  %258 = mul i16 %250, 6
  %259 = getelementptr inbounds i8, ptr %231, i16 %258
  %260 = load i16, ptr %259
  %261 = icmp ne i16 %260, 0
  br i1 %261, label %b79, label %b80

b82:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b83:
  store i8 1, ptr %2, !tbaa !2
  br label %b80

b84:
  br label %b85

b85:
  %262 = load i16, ptr %3, !tbaa !2
  %263 = add i16 %262, 1
  %264 = load i16, ptr %1, !tbaa !2
  %265 = and i16 %263, %264
  store i16 %265, ptr %3, !tbaa !2
  br label %b78

b86:
  %266 = load i16, ptr %3, !tbaa !2
  %267 = getelementptr i8, ptr %231, i16 -4
  %268 = load i16, ptr %267
  %269 = icmp ult i16 %266, %268
  br i1 %269, label %b89, label %b90

b87:
  %270 = mul i16 %254, 6
  %271 = getelementptr inbounds i8, ptr %231, i16 %270
  %272 = load i16, ptr %271
  %273 = load i16, ptr %4, !tbaa !2
  %274 = icmp eq i16 %272, %273
  br i1 %274, label %b86, label %b84

b88:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b89:
  %275 = mul i16 %266, 6
  %276 = getelementptr inbounds i8, ptr %231, i16 %275
  %277 = getelementptr i8, ptr %276, i16 2
  %278 = load i16, ptr %277
  %279 = load i16, ptr %5, !tbaa !2
  %280 = call cc 1005 addrspace(1) i8 @i16.eq(i16 %278, i16 %279)
  %281 = icmp ne i8 %280, 0
  br i1 %281, label %b83, label %b84

b90:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b91:
  %282 = load i16, ptr %3, !tbaa !2
  %283 = getelementptr i8, ptr %231, i16 -4
  %284 = load i16, ptr %283
  %285 = icmp ult i16 %282, %284
  br i1 %285, label %b94, label %b95

b92:
  store i16 5, ptr %0, !tbaa !2
  br label %b93

b93:
  %286 = load i16, ptr %0, !tbaa !2
  %287 = add i16 %230, %286
  %288 = load ptr, ptr %18, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %288)
  %289 = load ptr, ptr %25, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %289)
  ret i16 %287

b94:
  %290 = mul i16 %282, 6
  %291 = getelementptr inbounds i8, ptr %231, i16 %290
  %292 = getelementptr i8, ptr %291, i16 4
  %293 = load i16, ptr %292
  store i16 %293, ptr %0, !tbaa !2
  br label %b93

b95:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
}

define internal cc 1005 i16 @count() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca ptr
  %1 = alloca i16
  %2 = alloca i8
  %3 = alloca i16
  %4 = alloca i16
  %5 = alloca i16
  %6 = alloca i16
  %7 = alloca ptr
  %8 = alloca i16
  %9 = alloca i16
  %10 = alloca [8 x i8]
  store i16 4, ptr %8, !tbaa !2
  store i16 4, ptr %9, !tbaa !2
  %11 = getelementptr inbounds i16, ptr %10, i16 0
  store i16 1, ptr %11, !tbaa !2
  %12 = getelementptr inbounds i16, ptr %10, i16 1
  store i16 2, ptr %12, !tbaa !2
  %13 = getelementptr inbounds i16, ptr %10, i16 2
  store i16 1, ptr %13, !tbaa !2
  %14 = getelementptr inbounds i16, ptr %10, i16 3
  store i16 3, ptr %14, !tbaa !2
  %15 = getelementptr i8, ptr @$str1, i16 6
  store ptr %15, ptr %7, !tbaa !2
  store i16 0, ptr %6, !tbaa !2
  br label %b2

b2:
  %16 = load i16, ptr %6, !tbaa !2
  %17 = icmp ult i16 %16, 4
  br i1 %17, label %b3, label %b5

b3:
  %18 = load ptr, ptr %7, !tbaa !2
  %19 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %18, i16 6)
  store ptr %19, ptr %7, !tbaa !2
  %20 = sub i16 %16, 0
  %21 = getelementptr inbounds i16, ptr %10, i16 %20
  %22 = load i16, ptr %21, !tbaa !2
  store i16 %22, ptr %5, !tbaa !2
  %23 = load i16, ptr %5, !tbaa !2
  %24 = call cc 1005 addrspace(1) i16 @i16.hash(i16 %23)
  %25 = or i16 %24, 1
  store i16 %25, ptr %4, !tbaa !2
  store i16 0, ptr %3, !tbaa !2
  store i8 0, ptr %2, !tbaa !2
  %26 = getelementptr i8, ptr %19, i16 -4
  %27 = load i16, ptr %26
  %28 = icmp ne i16 %27, 0
  br i1 %28, label %b6, label %b7

b4:
  %29 = load i16, ptr %6, !tbaa !2
  %30 = add i16 %29, 1
  store i16 %30, ptr %6, !tbaa !2
  br label %b2

b5:
  %31 = load ptr, ptr %7, !tbaa !2
  store ptr null, ptr %7, !tbaa !2
  store ptr %31, ptr %0, !tbaa !2
  %32 = load ptr, ptr %0, !tbaa !2
  %33 = getelementptr i8, ptr %32, i16 -2
  %34 = load i16, ptr %33
  %35 = load ptr, ptr %0, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %35)
  %36 = load ptr, ptr %7, !tbaa !2
  call cc 1005 addrspace(1) void @N$BDRP(ptr %36)
  ret i16 %34

b6:
  %37 = getelementptr i8, ptr %19, i16 -4
  %38 = load i16, ptr %37
  %39 = sub i16 %38, 1
  store i16 %39, ptr %1, !tbaa !2
  %40 = load i16, ptr %4, !tbaa !2
  %41 = load i16, ptr %1, !tbaa !2
  %42 = and i16 %40, %41
  store i16 %42, ptr %3, !tbaa !2
  br label %b9

b7:
  br label %b8

b8:
  %43 = load i8, ptr %2, !tbaa !2, !range !10
  %44 = icmp ne i8 %43, 0
  br i1 %44, label %b23, label %b22

b9:
  %45 = load i16, ptr %3, !tbaa !2
  %46 = getelementptr i8, ptr %19, i16 -4
  %47 = load i16, ptr %46
  %48 = icmp ult i16 %45, %47
  br i1 %48, label %b12, label %b13

b10:
  %49 = load i16, ptr %3, !tbaa !2
  %50 = getelementptr i8, ptr %19, i16 -4
  %51 = load i16, ptr %50
  %52 = icmp ult i16 %49, %51
  br i1 %52, label %b18, label %b19

b11:
  br label %b8

b12:
  %53 = mul i16 %45, 6
  %54 = getelementptr inbounds i8, ptr %19, i16 %53
  %55 = load i16, ptr %54
  %56 = icmp ne i16 %55, 0
  br i1 %56, label %b10, label %b11

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b14:
  store i8 1, ptr %2, !tbaa !2
  br label %b11

b15:
  br label %b16

b16:
  %57 = load i16, ptr %3, !tbaa !2
  %58 = add i16 %57, 1
  %59 = load i16, ptr %1, !tbaa !2
  %60 = and i16 %58, %59
  store i16 %60, ptr %3, !tbaa !2
  br label %b9

b17:
  %61 = load i16, ptr %3, !tbaa !2
  %62 = getelementptr i8, ptr %19, i16 -4
  %63 = load i16, ptr %62
  %64 = icmp ult i16 %61, %63
  br i1 %64, label %b20, label %b21

b18:
  %65 = mul i16 %49, 6
  %66 = getelementptr inbounds i8, ptr %19, i16 %65
  %67 = load i16, ptr %66
  %68 = load i16, ptr %4, !tbaa !2
  %69 = icmp eq i16 %67, %68
  br i1 %69, label %b17, label %b15

b19:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b20:
  %70 = mul i16 %61, 6
  %71 = getelementptr inbounds i8, ptr %19, i16 %70
  %72 = getelementptr i8, ptr %71, i16 2
  %73 = load i16, ptr %72
  %74 = load i16, ptr %5, !tbaa !2
  %75 = call cc 1005 addrspace(1) i8 @i16.eq(i16 %73, i16 %74)
  %76 = icmp ne i8 %75, 0
  br i1 %76, label %b14, label %b15

b21:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b22:
  %77 = load i16, ptr %3, !tbaa !2
  %78 = getelementptr i8, ptr %19, i16 -4
  %79 = load i16, ptr %78
  %80 = icmp ult i16 %77, %79
  br i1 %80, label %b25, label %b26

b23:
  br label %b24

b24:
  %81 = load i8, ptr %2, !tbaa !2, !range !11
  %82 = icmp ne i8 %81, 0
  br i1 %82, label %b30, label %b29

b25:
  %83 = mul i16 %77, 6
  %84 = getelementptr inbounds i8, ptr %19, i16 %83
  %85 = load i16, ptr %4, !tbaa !2
  store i16 %85, ptr %84
  %86 = load i16, ptr %3, !tbaa !2
  %87 = getelementptr i8, ptr %19, i16 -4
  %88 = load i16, ptr %87
  %89 = icmp ult i16 %86, %88
  br i1 %89, label %b27, label %b28

b26:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b27:
  %90 = mul i16 %86, 6
  %91 = getelementptr inbounds i8, ptr %19, i16 %90
  %92 = load i16, ptr %5, !tbaa !2
  %93 = getelementptr i8, ptr %91, i16 2
  store i16 %92, ptr %93
  br label %b24

b28:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b29:
  %94 = getelementptr i8, ptr %19, i16 -2
  %95 = load i16, ptr %94
  %96 = add i16 %95, 1
  %97 = getelementptr i8, ptr %19, i16 -2
  store i16 %96, ptr %97
  br label %b30

b30:
  %98 = load i16, ptr %3, !tbaa !2
  %99 = getelementptr i8, ptr %19, i16 -4
  %100 = load i16, ptr %99
  %101 = icmp ult i16 %98, %100
  br i1 %101, label %b31, label %b32

b31:
  %102 = mul i16 %98, 6
  %103 = getelementptr inbounds i8, ptr %19, i16 %102
  %104 = sub i16 %16, 0
  %105 = getelementptr inbounds i16, ptr %10, i16 %104
  %106 = load i16, ptr %105, !tbaa !2
  %107 = mul i16 %106, 10
  %108 = getelementptr i8, ptr %103, i16 4
  store i16 %107, ptr %108
  br label %b4

b32:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable
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

declare cc 1005 ptr @N$DRES(ptr, i16) addrspace(1)

declare cc 1005 void @N$EBND() addrspace(1) noreturn memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$BDRP(ptr) addrspace(1)

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
