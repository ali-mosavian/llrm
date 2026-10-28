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

define cc 1005 i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
b1:
  %0 = alloca [8 x i8]
  %1 = alloca [8 x i8]
  %2 = alloca [8 x i8]
  %3 = alloca [8 x i8]
  %4 = alloca [8 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [8 x i8]
  %7 = alloca [8 x i8]
  %8 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 9, i16 2)
  store ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), ptr %8
  %9 = getelementptr inbounds nuw i8, ptr %8, i16 2
  store ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), ptr %9
  %10 = getelementptr inbounds nuw i8, ptr %8, i16 4
  store ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6), ptr %10
  %11 = getelementptr inbounds nuw i8, ptr %8, i16 6
  store ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), ptr %11
  %12 = getelementptr inbounds nuw i8, ptr %8, i16 8
  store ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6), ptr %12
  %13 = getelementptr inbounds nuw i8, ptr %8, i16 10
  store ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6), ptr %13
  %14 = getelementptr inbounds nuw i8, ptr %8, i16 12
  store ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6), ptr %14
  %15 = getelementptr inbounds nuw i8, ptr %8, i16 14
  store ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6), ptr %15
  %16 = getelementptr inbounds nuw i8, ptr %8, i16 16
  store ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6), ptr %16
  %17 = getelementptr i8, ptr %8, i16 -4
  %18 = load i16, ptr %17
  %19 = icmp eq i16 %18, 0
  br i1 %19, label %55, label %20

20:
  %21 = addrspacecast ptr %7 to ptr addrspace(1)
  %22 = addrspacecast ptr %6 to ptr addrspace(1)
  %23 = addrspacecast ptr %5 to ptr addrspace(1)
  %24 = addrspacecast ptr %4 to ptr addrspace(1)
  br label %25

25:
  %26 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %20 ], [ %30, %b51 ]
  %27 = phi i16 [ 0, %20 ], [ %167, %b51 ]
  %28 = shl i16 %27, 1
  %29 = getelementptr inbounds i8, ptr %8, i16 %28
  %30 = call cc 1005 addrspace(1) ptr @N$DRES(ptr nonnull %26, i16 6)
  %31 = load ptr, ptr %29
  %32 = call cc 1005 addrspace(1) ptr @N$BCLN(ptr %31, i16 1)
  %33 = getelementptr i8, ptr %32, i16 -4
  %34 = load i16, ptr %33
  %35 = addrspacecast ptr %32 to ptr addrspace(1)
  %36 = icmp eq i16 %34, 0
  br i1 %36, label %50, label %37

37:
  br label %38

38:
  %39 = phi ptr [ %32, %37 ], [ %47, %38 ]
  %40 = phi i16 [ %34, %37 ], [ %46, %38 ]
  %41 = phi i16 [ %45, %38 ], [ 5381, %37 ]
  %42 = mul i16 %41, 33
  %43 = load i8, ptr %39
  %44 = zext i8 %43 to i16
  %45 = xor i16 %42, %44
  %46 = add i16 %40, -1
  %47 = getelementptr i8, ptr %39, i16 1
  %exitcond3 = icmp eq i16 %46, 0
  br i1 %exitcond3, label %48, label %38

48:
  %49 = or i16 %45, 1
  br label %50

50:
  %51 = phi i16 [ 5381, %25 ], [ %49, %48 ]
  %52 = getelementptr i8, ptr %30, i16 -4
  %53 = load i16, ptr %52
  %54 = icmp eq i16 %53, 0
  br i1 %54, label %b26, label %60

55:
  %56 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %b1 ], [ %30, %b51 ]
  %57 = getelementptr i8, ptr %56, i16 -4
  %58 = load i16, ptr %57
  %59 = icmp eq i16 %58, 0
  br i1 %59, label %b71, label %172

60:
  %61 = add i16 %53, -1
  %62 = and i16 %61, %51
  br label %b12

b12:
  %63 = phi i16 [ %53, %60 ], [ %69, %b16 ]
  %64 = phi i16 [ %62, %60 ], [ %71, %b16 ]
  %65 = mul i16 %64, 6
  %66 = getelementptr inbounds i8, ptr %30, i16 %65
  %67 = load i16, ptr %66
  %68 = icmp eq i16 %67, 0
  br i1 %68, label %b22, label %b18

b13:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b16:
  %69 = phi i16 [ %91, %89 ], [ %63, %b18 ]
  %70 = add nuw i16 %64, 1
  %71 = and i16 %70, %61
  %72 = icmp ult i16 %71, %69
  br i1 %72, label %b12, label %b13

b18:
  %73 = icmp eq i16 %67, %51
  br i1 %73, label %74, label %b16

74:
  %75 = getelementptr i8, ptr %30, i16 %65
  %76 = getelementptr i8, ptr %75, i16 2
  %77 = load ptr, ptr %76
  %78 = getelementptr i8, ptr %77, i16 -4
  %79 = load i16, ptr %78
  %80 = addrspacecast ptr %77 to ptr addrspace(1)
  store i16 %79, ptr %7, !tbaa !2
  %81 = getelementptr i8, ptr %7, i16 2
  store i16 %79, ptr %81, !tbaa !2
  %82 = getelementptr i8, ptr %7, i16 4
  store ptr addrspace(1) %80, ptr %82, !tbaa !2
  %83 = getelementptr i8, ptr %32, i16 -4
  %84 = load i16, ptr %83
  store i16 %84, ptr %6, !tbaa !2
  %85 = getelementptr i8, ptr %6, i16 2
  store i16 %84, ptr %85, !tbaa !2
  %86 = getelementptr i8, ptr %6, i16 4
  store ptr addrspace(1) %35, ptr %86, !tbaa !2
  %87 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %21, ptr addrspace(1) %22)
  %88 = icmp eq i8 %87, 0
  br i1 %88, label %b30, label %89

89:
  %90 = getelementptr i8, ptr %30, i16 -4
  %91 = load i16, ptr %90
  br label %b16

b22:
  %92 = icmp ult i16 %64, %63
  br i1 %92, label %b25, label %b26

b25:
  %93 = mul i16 %64, 6
  %94 = getelementptr inbounds i8, ptr %30, i16 %93
  store i16 %51, ptr %94
  %95 = getelementptr i8, ptr %30, i16 -4
  %96 = load i16, ptr %95
  %97 = icmp ult i16 %64, %96
  br i1 %97, label %b29, label %b28

b26:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b28:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b29:
  %98 = getelementptr i8, ptr %30, i16 %93
  %99 = getelementptr i8, ptr %98, i16 2
  %100 = load ptr, ptr %99
  call cc 1005 addrspace(1) void @N$BDRP(ptr %100)
  store ptr %32, ptr %99
  %101 = getelementptr i8, ptr %30, i16 -2
  %102 = load i16, ptr %101
  %103 = add i16 %102, 1
  store i16 %103, ptr %101
  br label %b30

b30:
  %104 = phi ptr [ null, %b29 ], [ %32, %74 ]
  %105 = getelementptr i8, ptr %30, i16 -4
  %106 = load i16, ptr %105
  %107 = icmp ult i16 %64, %106
  br i1 %107, label %108, label %b32

108:
  %109 = mul i16 %64, 6
  %110 = getelementptr i8, ptr %8, i16 %28
  %111 = load ptr, ptr %110
  %112 = getelementptr i8, ptr %111, i16 -4
  %113 = load i16, ptr %112
  %114 = addrspacecast ptr %111 to ptr addrspace(1)
  store i16 %113, ptr %5, !tbaa !2
  %115 = getelementptr i8, ptr %5, i16 2
  store i16 %113, ptr %115, !tbaa !2
  %116 = getelementptr i8, ptr %5, i16 4
  store ptr addrspace(1) %114, ptr %116, !tbaa !2
  %117 = icmp eq i16 %113, 0
  br i1 %117, label %131, label %118

118:
  br label %119

119:
  %120 = phi ptr addrspace(1) [ %114, %118 ], [ %128, %119 ]
  %121 = phi i16 [ %113, %118 ], [ %127, %119 ]
  %122 = phi i16 [ %126, %119 ], [ 5381, %118 ]
  %123 = mul i16 %122, 33
  %124 = load i8, ptr addrspace(1) %120
  %125 = zext i8 %124 to i16
  %126 = xor i16 %123, %125
  %127 = add i16 %121, -1
  %128 = getelementptr i8, ptr addrspace(1) %120, i16 1
  %exitcond1 = icmp eq i16 %127, 0
  br i1 %exitcond1, label %129, label %119

129:
  %130 = or i16 %126, 1
  br label %131

131:
  %132 = phi i16 [ 5381, %108 ], [ %130, %129 ]
  %133 = getelementptr i8, ptr %30, i16 -4
  %134 = load i16, ptr %133
  %135 = icmp eq i16 %134, 0
  br i1 %135, label %b51, label %136

b32:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

136:
  %137 = add i16 %134, -1
  %138 = and i16 %137, %132
  br label %b39

b39:
  %139 = phi i16 [ %134, %136 ], [ %146, %b43 ]
  %140 = phi i16 [ %138, %136 ], [ %148, %b43 ]
  %141 = mul i16 %140, 6
  %142 = getelementptr inbounds i8, ptr %30, i16 %141
  %143 = load i16, ptr %142
  %144 = icmp eq i16 %143, 0
  br i1 %144, label %b51, label %b45

b40:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b41:
  %145 = icmp ult i16 %140, %163
  br i1 %145, label %b52, label %b53

b43:
  %146 = phi i16 [ %163, %151 ], [ %139, %b45 ]
  %147 = add nuw i16 %140, 1
  %148 = and i16 %147, %137
  %149 = icmp ult i16 %148, %146
  br i1 %149, label %b39, label %b40

b45:
  %150 = icmp eq i16 %143, %132
  br i1 %150, label %151, label %b43

151:
  %152 = getelementptr i8, ptr %30, i16 %141
  %153 = getelementptr i8, ptr %152, i16 2
  %154 = load ptr, ptr %153
  %155 = getelementptr i8, ptr %154, i16 -4
  %156 = load i16, ptr %155
  %157 = addrspacecast ptr %154 to ptr addrspace(1)
  store i16 %156, ptr %4, !tbaa !2
  %158 = getelementptr i8, ptr %4, i16 2
  store i16 %156, ptr %158, !tbaa !2
  %159 = getelementptr i8, ptr %4, i16 4
  store ptr addrspace(1) %157, ptr %159, !tbaa !2
  %160 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %24, ptr addrspace(1) %23)
  %161 = icmp eq i8 %160, 0
  %162 = getelementptr i8, ptr %30, i16 -4
  %163 = load i16, ptr %162
  br i1 %161, label %b41, label %b43

b51:
  %164 = phi i16 [ %171, %b52 ], [ 1, %131 ], [ 1, %b39 ]
  %165 = getelementptr i8, ptr %30, i16 %109
  %166 = getelementptr i8, ptr %165, i16 4
  store i16 %164, ptr %166
  call cc 1005 addrspace(1) void @N$BDRP(ptr %104)
  %167 = add nuw i16 %27, 1
  %exitcond8 = icmp eq i16 %167, %18
  br i1 %exitcond8, label %55, label %25

b52:
  %168 = getelementptr i8, ptr %30, i16 %141
  %169 = getelementptr i8, ptr %168, i16 4
  %170 = load i16, ptr %169
  %171 = add i16 %170, 1
  br label %b51

b53:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

172:
  %173 = add i16 %58, -1
  %174 = and i16 %173, 25309
  %175 = addrspacecast ptr %3 to ptr addrspace(1)
  %176 = addrspacecast ptr %2 to ptr addrspace(1)
  br label %b60

b60:
  %177 = phi i16 [ %58, %172 ], [ %186, %b64 ]
  %178 = phi i16 [ %174, %172 ], [ %188, %b64 ]
  %179 = mul i16 %178, 6
  %180 = getelementptr inbounds i8, ptr %56, i16 %179
  %181 = load i16, ptr %180
  %182 = icmp eq i16 %181, 0
  br i1 %182, label %b71, label %183

183:
  %184 = icmp eq i16 %181, 25309
  br i1 %184, label %190, label %b64

b61:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b62:
  %185 = icmp ult i16 %178, %203
  br i1 %185, label %204, label %b73

b64:
  %186 = phi i16 [ %203, %190 ], [ %177, %183 ]
  %187 = add nuw i16 %178, 1
  %188 = and i16 %187, %173
  %189 = icmp ult i16 %188, %186
  br i1 %189, label %b60, label %b61

190:
  %191 = getelementptr i8, ptr %56, i16 %179
  %192 = getelementptr i8, ptr %191, i16 2
  %193 = load ptr, ptr %192
  %194 = getelementptr i8, ptr %193, i16 -4
  %195 = load i16, ptr %194
  %196 = addrspacecast ptr %193 to ptr addrspace(1)
  store i16 %195, ptr %3, !tbaa !2
  %197 = getelementptr i8, ptr %3, i16 2
  store i16 %195, ptr %197, !tbaa !2
  %198 = getelementptr i8, ptr %3, i16 4
  store ptr addrspace(1) %196, ptr %198, !tbaa !2
  store i32 196611, ptr %2
  %199 = getelementptr i8, ptr %2, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6) to ptr addrspace(1)), ptr %199, !tbaa !2
  %200 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %175, ptr addrspace(1) %176)
  %201 = icmp eq i8 %200, 0
  %202 = getelementptr i8, ptr %56, i16 -4
  %203 = load i16, ptr %202
  br i1 %201, label %b62, label %b64

b71:
  call cc 1005 addrspace(1) void @N$EKEY()
  unreachable

b73:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

204:
  %205 = getelementptr i8, ptr %56, i16 %179
  %206 = getelementptr i8, ptr %205, i16 4
  %207 = load i16, ptr %206
  %208 = getelementptr i8, ptr %56, i16 -2
  %209 = load i16, ptr %208
  %210 = add i16 %203, -1
  %211 = and i16 %210, 24433
  %212 = addrspacecast ptr %1 to ptr addrspace(1)
  %213 = addrspacecast ptr %0 to ptr addrspace(1)
  br label %b80

b80:
  %214 = phi i16 [ %203, %204 ], [ %223, %b84 ]
  %215 = phi i16 [ %211, %204 ], [ %225, %b84 ]
  %216 = mul i16 %215, 6
  %217 = getelementptr inbounds i8, ptr %56, i16 %216
  %218 = load i16, ptr %217
  %219 = icmp eq i16 %218, 0
  br i1 %219, label %241, label %220

220:
  %221 = icmp eq i16 %218, 24433
  br i1 %221, label %227, label %b84

b81:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b82:
  %222 = icmp ult i16 %215, %240
  br i1 %222, label %b93, label %b94

b84:
  %223 = phi i16 [ %240, %227 ], [ %214, %220 ]
  %224 = add nuw i16 %215, 1
  %225 = and i16 %224, %210
  %226 = icmp ult i16 %225, %223
  br i1 %226, label %b80, label %b81

227:
  %228 = getelementptr i8, ptr %56, i16 %216
  %229 = getelementptr i8, ptr %228, i16 2
  %230 = load ptr, ptr %229
  %231 = getelementptr i8, ptr %230, i16 -4
  %232 = load i16, ptr %231
  %233 = addrspacecast ptr %230 to ptr addrspace(1)
  store i16 %232, ptr %1, !tbaa !2
  %234 = getelementptr i8, ptr %1, i16 2
  store i16 %232, ptr %234, !tbaa !2
  %235 = getelementptr i8, ptr %1, i16 4
  store ptr addrspace(1) %233, ptr %235, !tbaa !2
  store i32 196611, ptr %0
  %236 = getelementptr i8, ptr %0, i16 4
  store ptr addrspace(1) addrspacecast (ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6) to ptr addrspace(1)), ptr %236, !tbaa !2
  %237 = call cc 1005 addrspace(1) i8 @N$VCMP(ptr addrspace(1) %212, ptr addrspace(1) %213)
  %238 = icmp eq i8 %237, 0
  %239 = getelementptr i8, ptr %56, i16 -4
  %240 = load i16, ptr %239
  br i1 %238, label %b82, label %b84

241:
  %242 = phi i16 [ %255, %b93 ], [ %218, %b80 ]
  call cc 1005 addrspace(1) void @N$PU2(i16 %209)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %207)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %242)
  call cc 1005 addrspace(1) void @N$PN()
  %243 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 3, i16 2)
  store i8 12, ptr %243
  %244 = getelementptr i8, ptr %243, i16 1
  store i8 0, ptr %244
  %245 = getelementptr inbounds nuw i8, ptr %243, i16 2
  store i8 3, ptr %245
  %246 = getelementptr i8, ptr %243, i16 3
  store i8 1, ptr %246
  %247 = getelementptr inbounds nuw i8, ptr %243, i16 4
  store i8 12, ptr %247
  %248 = getelementptr i8, ptr %243, i16 5
  store i8 0, ptr %248
  %249 = call cc 1005 addrspace(1) ptr @N$DRES(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 6)
  %250 = getelementptr i8, ptr %249, i16 -4
  %251 = load i16, ptr %250
  %252 = icmp eq i16 %251, 0
  br i1 %252, label %b111, label %b95

b93:
  %253 = getelementptr i8, ptr %56, i16 %216
  %254 = getelementptr i8, ptr %253, i16 4
  %255 = load i16, ptr %254
  br label %241

b94:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b95:
  %256 = add i16 %251, -1
  %257 = and i16 %256, 1
  br label %b98

b98:
  %258 = phi i16 [ %257, %b95 ], [ %267, %b105 ]
  %259 = icmp ult i16 %258, %251
  br i1 %259, label %b101, label %b102

b101:
  %260 = mul i16 %258, 6
  %261 = getelementptr inbounds i8, ptr %249, i16 %260
  %262 = load i16, ptr %261
  %263 = icmp eq i16 %262, 0
  br i1 %263, label %b111, label %264

264:
  %265 = icmp eq i16 %262, 1
  br i1 %265, label %268, label %b105

b102:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b105:
  %266 = add nuw i16 %258, 1
  %267 = and i16 %266, %256
  br label %b98

268:
  %269 = getelementptr i8, ptr %249, i16 %260
  %270 = getelementptr i8, ptr %269, i16 2
  %271 = load i8, ptr %270, align 2
  %272 = icmp eq i8 %271, 0
  br i1 %272, label %b119, label %b105

b111:
  %.ph6 = phi i16 [ %258, %b101 ], [ 0, %241 ]
  %273 = icmp ult i16 %.ph6, %251
  br i1 %273, label %b114, label %b115

b114:
  %274 = mul i16 %.ph6, 6
  %275 = getelementptr inbounds i8, ptr %249, i16 %274
  store i16 1, ptr %275
  %276 = getelementptr i8, ptr %249, i16 -4
  %277 = load i16, ptr %276
  %278 = icmp ult i16 %.ph6, %277
  br i1 %278, label %b118, label %b117

b115:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b117:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b118:
  %279 = getelementptr i8, ptr %249, i16 %274
  %280 = getelementptr i8, ptr %279, i16 2
  store i8 0, ptr %280, align 2
  %281 = getelementptr i8, ptr %249, i16 -2
  %282 = load i16, ptr %281
  %283 = add i16 %282, 1
  store i16 %283, ptr %281
  %284 = getelementptr i8, ptr %249, i16 -4
  %285 = load i16, ptr %284
  br label %b119

b119:
  %286 = phi i16 [ %251, %268 ], [ %285, %b118 ]
  %287 = phi i16 [ %258, %268 ], [ %.ph6, %b118 ]
  %288 = icmp ult i16 %287, %286
  br i1 %288, label %289, label %b121

289:
  %290 = mul i16 %287, 6
  %291 = getelementptr inbounds i8, ptr %249, i16 %290
  %292 = getelementptr i8, ptr %291, i16 4
  store i16 0, ptr %292
  %293 = call cc 1005 addrspace(1) ptr @N$DRES(ptr nonnull %249, i16 6)
  %294 = getelementptr i8, ptr %293, i16 -4
  %295 = load i16, ptr %294
  %296 = icmp eq i16 %295, 0
  br i1 %296, label %b138, label %b122

b121:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b122:
  %297 = add i16 %295, -1
  %298 = and i16 %297, 1
  br label %b125

b125:
  %299 = phi i16 [ %298, %b122 ], [ %308, %b132 ]
  %300 = icmp ult i16 %299, %295
  br i1 %300, label %b128, label %b129

b128:
  %301 = mul i16 %299, 6
  %302 = getelementptr inbounds i8, ptr %293, i16 %301
  %303 = load i16, ptr %302
  %304 = icmp eq i16 %303, 0
  br i1 %304, label %b138, label %305

305:
  %306 = icmp eq i16 %303, 1
  br i1 %306, label %309, label %b132

b129:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b132:
  %307 = add nuw i16 %299, 1
  %308 = and i16 %307, %297
  br label %b125

309:
  %310 = getelementptr i8, ptr %293, i16 %301
  %311 = getelementptr i8, ptr %310, i16 2
  %312 = load i8, ptr %311, align 2
  %313 = icmp eq i8 %312, 1
  br i1 %313, label %b146, label %b132

b138:
  %.ph7 = phi i16 [ %299, %b128 ], [ 0, %289 ]
  %314 = icmp ult i16 %.ph7, %295
  br i1 %314, label %b141, label %b142

b141:
  %315 = mul i16 %.ph7, 6
  %316 = getelementptr inbounds i8, ptr %293, i16 %315
  store i16 1, ptr %316
  %317 = getelementptr i8, ptr %293, i16 -4
  %318 = load i16, ptr %317
  %319 = icmp ult i16 %.ph7, %318
  br i1 %319, label %b145, label %b144

b142:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b144:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b145:
  %320 = getelementptr i8, ptr %293, i16 %315
  %321 = getelementptr i8, ptr %320, i16 2
  store i8 1, ptr %321, align 2
  %322 = getelementptr i8, ptr %293, i16 -2
  %323 = load i16, ptr %322
  %324 = add i16 %323, 1
  store i16 %324, ptr %322
  %325 = getelementptr i8, ptr %293, i16 -4
  %326 = load i16, ptr %325
  br label %b146

b146:
  %327 = phi i16 [ %295, %309 ], [ %326, %b145 ]
  %328 = phi i16 [ %299, %309 ], [ %.ph7, %b145 ]
  %329 = icmp ult i16 %328, %327
  br i1 %329, label %b147, label %b148

b147:
  %330 = mul i16 %328, 6
  %331 = getelementptr inbounds i8, ptr %293, i16 %330
  %332 = getelementptr i8, ptr %331, i16 4
  store i16 0, ptr %332
  %333 = getelementptr i8, ptr %243, i16 -4
  %334 = load i16, ptr %333
  %335 = icmp eq i16 %334, 0
  br i1 %335, label %b152, label %336

336:
  br label %337

b148:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

337:
  %338 = phi i16 [ %446, %b205 ], [ 0, %336 ]
  %339 = phi ptr [ %392, %b205 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %336 ]
  %340 = phi ptr [ %343, %b205 ], [ %293, %336 ]
  %341 = shl i16 %338, 1
  %342 = getelementptr inbounds i8, ptr %243, i16 %341
  %343 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %340, i16 6)
  %344 = getelementptr i8, ptr %342, i16 1
  %345 = load i8, ptr %344
  %346 = zext i8 %345 to i16
  %347 = or i16 %346, 1
  %348 = getelementptr i8, ptr %343, i16 -4
  %349 = load i16, ptr %348
  %350 = icmp eq i16 %349, 0
  br i1 %350, label %b169, label %356

b152:
  %351 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %b147 ], [ %392, %b205 ]
  %352 = phi ptr [ %293, %b147 ], [ %343, %b205 ]
  %353 = getelementptr i8, ptr %352, i16 -4
  %354 = load i16, ptr %353
  %355 = icmp eq i16 %354, 0
  br i1 %355, label %b224, label %b207

356:
  %357 = add i16 %349, -1
  br label %b159

b159:
  %sink = phi i16 [ %363, %b163 ], [ %347, %356 ]
  %358 = and i16 %sink, %357
  %359 = mul i16 %358, 6
  %360 = getelementptr inbounds i8, ptr %343, i16 %359
  %361 = load i16, ptr %360
  %362 = icmp eq i16 %361, 0
  br i1 %362, label %b169, label %b165

b163:
  %363 = add i16 %358, 1
  br label %b159

b165:
  %364 = icmp eq i16 %361, %347
  br i1 %364, label %365, label %b163

365:
  %366 = getelementptr i8, ptr %343, i16 %359
  %367 = getelementptr i8, ptr %366, i16 2
  %368 = load i8, ptr %367, align 2
  %369 = icmp eq i8 %368, %345
  br i1 %369, label %b177, label %b163

b169:
  %.ph8 = phi i16 [ 0, %337 ], [ %358, %b159 ]
  %370 = icmp ult i16 %.ph8, %349
  br i1 %370, label %b172, label %b173

b172:
  %371 = mul i16 %.ph8, 6
  %372 = getelementptr inbounds i8, ptr %343, i16 %371
  store i16 %347, ptr %372
  %373 = getelementptr i8, ptr %343, i16 -4
  %374 = load i16, ptr %373
  %375 = icmp ult i16 %.ph8, %374
  br i1 %375, label %b176, label %b175

b173:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b175:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b176:
  %376 = getelementptr i8, ptr %343, i16 %371
  %377 = getelementptr i8, ptr %376, i16 2
  store i8 %345, ptr %377, align 2
  %378 = getelementptr i8, ptr %343, i16 -2
  %379 = load i16, ptr %378
  %380 = add i16 %379, 1
  store i16 %380, ptr %378
  %381 = getelementptr i8, ptr %343, i16 -4
  %382 = load i16, ptr %381
  br label %b177

b177:
  %383 = phi i16 [ %382, %b176 ], [ %349, %365 ]
  %384 = phi i16 [ %.ph8, %b176 ], [ %358, %365 ]
  %385 = icmp ult i16 %384, %383
  br i1 %385, label %386, label %b179

386:
  %387 = mul i16 %384, 6
  %388 = getelementptr inbounds i8, ptr %343, i16 %387
  %389 = getelementptr i8, ptr %388, i16 4
  %390 = load i16, ptr %389
  %391 = add i16 %390, 1
  store i16 %391, ptr %389
  %392 = call cc 1005 addrspace(1) ptr @N$DRES(ptr %339, i16 6)
  %393 = getelementptr i8, ptr %243, i16 %341
  %394 = load i8, ptr %393
  %395 = getelementptr i8, ptr %243, i16 %341
  %396 = getelementptr i8, ptr %395, i16 1
  %397 = load i8, ptr %396
  %398 = zext i8 %394 to i16
  %399 = mul nuw nsw i16 %398, 31
  %400 = zext i8 %397 to i16
  %401 = add nuw nsw i16 %399, %400
  %402 = or i16 %401, 1
  %403 = getelementptr i8, ptr %392, i16 -4
  %404 = load i16, ptr %403
  %405 = icmp eq i16 %404, 0
  br i1 %405, label %b196, label %406

b179:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

406:
  %407 = add i16 %404, -1
  br label %b186

b186:
  %sink1 = phi i16 [ %413, %b190 ], [ %402, %406 ]
  %408 = and i16 %sink1, %407
  %409 = mul i16 %408, 6
  %410 = getelementptr inbounds i8, ptr %392, i16 %409
  %411 = load i16, ptr %410
  %412 = icmp eq i16 %411, 0
  br i1 %412, label %b196, label %b192

b190:
  %413 = add i16 %408, 1
  br label %b186

b192:
  %414 = icmp eq i16 %411, %402
  br i1 %414, label %415, label %b190

415:
  %416 = getelementptr i8, ptr %392, i16 %409
  %417 = getelementptr i8, ptr %416, i16 2
  %418 = load i8, ptr %417, align 2
  %419 = icmp eq i8 %418, %394
  %420 = getelementptr i8, ptr %392, i16 %409
  %421 = getelementptr i8, ptr %420, i16 3
  %422 = load i8, ptr %421
  %423 = icmp eq i8 %422, %397
  %424 = select i1 %419, i1 %423, i1 false
  br i1 %424, label %b204, label %b190

b196:
  %.ph9 = phi i16 [ 0, %386 ], [ %408, %b186 ]
  %425 = icmp ult i16 %.ph9, %404
  br i1 %425, label %b199, label %b200

b199:
  %426 = mul i16 %.ph9, 6
  %427 = getelementptr inbounds i8, ptr %392, i16 %426
  store i16 %402, ptr %427
  %428 = getelementptr i8, ptr %392, i16 -4
  %429 = load i16, ptr %428
  %430 = icmp ult i16 %.ph9, %429
  br i1 %430, label %b203, label %b202

b200:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b202:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b203:
  %431 = getelementptr i8, ptr %392, i16 %426
  %432 = getelementptr i8, ptr %431, i16 2
  store i8 %394, ptr %432, align 2
  %433 = getelementptr i8, ptr %392, i16 %426
  %434 = getelementptr i8, ptr %433, i16 3
  store i8 %397, ptr %434
  %435 = getelementptr i8, ptr %392, i16 -2
  %436 = load i16, ptr %435
  %437 = add i16 %436, 1
  store i16 %437, ptr %435
  %438 = getelementptr i8, ptr %392, i16 -4
  %439 = load i16, ptr %438
  br label %b204

b204:
  %440 = phi i16 [ %439, %b203 ], [ %404, %415 ]
  %441 = phi i16 [ %.ph9, %b203 ], [ %408, %415 ]
  %442 = icmp ult i16 %441, %440
  br i1 %442, label %b205, label %b206

b205:
  %443 = mul i16 %441, 6
  %444 = getelementptr inbounds i8, ptr %392, i16 %443
  %445 = getelementptr i8, ptr %444, i16 4
  store i8 1, ptr %445
  %446 = add nuw i16 %338, 1
  %exitcond7 = icmp eq i16 %446, %334
  br i1 %exitcond7, label %b152, label %337

b206:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b207:
  %447 = add i16 %354, -1
  %448 = and i16 %447, 1
  br label %b210

b210:
  %449 = phi i16 [ %448, %b207 ], [ %458, %b217 ]
  %450 = icmp ult i16 %449, %354
  br i1 %450, label %b213, label %b214

b213:
  %451 = mul i16 %449, 6
  %452 = getelementptr inbounds i8, ptr %352, i16 %451
  %453 = load i16, ptr %452
  %454 = icmp eq i16 %453, 0
  br i1 %454, label %b224, label %455

455:
  %456 = icmp eq i16 %453, 1
  br i1 %456, label %459, label %b217

b214:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b217:
  %457 = add nuw i16 %449, 1
  %458 = and i16 %457, %447
  br label %b210

459:
  %460 = getelementptr i8, ptr %352, i16 %451
  %461 = getelementptr i8, ptr %460, i16 2
  %462 = load i8, ptr %461, align 2
  %463 = icmp eq i8 %462, 0
  br i1 %463, label %464, label %b217

b224:
  call cc 1005 addrspace(1) void @N$EKEY()
  unreachable

464:
  %465 = getelementptr i8, ptr %352, i16 %451
  %466 = getelementptr i8, ptr %465, i16 4
  %467 = load i16, ptr %466
  %468 = getelementptr i8, ptr %351, i16 -2
  %469 = load i16, ptr %468
  %470 = getelementptr i8, ptr %351, i16 -4
  %471 = load i16, ptr %470
  %472 = icmp eq i16 %471, 0
  br i1 %472, label %b229, label %b227

b227:
  %473 = add i16 %471, -1
  %474 = and i16 %473, 373
  br label %b230

b229:
  %475 = phi i8 [ 0, %b233 ], [ 0, %464 ], [ 1, %486 ]
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 %467)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %469)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6))
  call cc 1005 addrspace(1) void @N$PB(i8 %475)
  call cc 1005 addrspace(1) void @N$PN()
  br label %496

b230:
  %476 = phi i16 [ %474, %b227 ], [ %485, %b237 ]
  %477 = icmp ult i16 %476, %471
  br i1 %477, label %b233, label %b234

b233:
  %478 = mul i16 %476, 6
  %479 = getelementptr inbounds i8, ptr %351, i16 %478
  %480 = load i16, ptr %479
  %481 = icmp eq i16 %480, 0
  br i1 %481, label %b229, label %482

482:
  %483 = icmp eq i16 %480, 373
  br i1 %483, label %486, label %b237

b234:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b237:
  %484 = add nuw i16 %476, 1
  %485 = and i16 %484, %473
  br label %b230

486:
  %487 = getelementptr i8, ptr %351, i16 %478
  %488 = getelementptr i8, ptr %487, i16 2
  %489 = load i8, ptr %488, align 2
  %490 = icmp eq i8 %489, 12
  %491 = getelementptr i8, ptr %351, i16 %478
  %492 = getelementptr i8, ptr %491, i16 3
  %493 = load i8, ptr %492
  %494 = icmp eq i8 %493, 0
  %495 = select i1 %490, i1 %494, i1 false
  br i1 %495, label %b229, label %b237

496:
  %497 = phi i16 [ 0, %b229 ], [ %546, %b272 ]
  %498 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %b229 ], [ %499, %b272 ]
  %499 = call cc 1005 addrspace(1) ptr @N$DRES(ptr nonnull %498, i16 6)
  %500 = or i16 %497, 1
  %501 = getelementptr i8, ptr %499, i16 -4
  %502 = load i16, ptr %501
  %503 = icmp eq i16 %502, 0
  br i1 %503, label %b263, label %510

504:
  %505 = getelementptr i8, ptr %499, i16 -2
  %506 = load i16, ptr %505
  call cc 1005 addrspace(1) void @N$PU2(i16 %506)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6))
  call cc 1005 addrspace(1) void @N$PI2(i16 37)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6))
  %507 = getelementptr i8, ptr %499, i16 -4
  %508 = load i16, ptr %507
  %509 = icmp eq i16 %508, 0
  br i1 %509, label %b291, label %b274

510:
  %511 = add i16 %502, -1
  %512 = and i16 %511, %500
  br label %b253

b253:
  %513 = phi i16 [ %512, %510 ], [ %519, %b257 ]
  %514 = mul i16 %513, 6
  %515 = getelementptr inbounds i8, ptr %499, i16 %514
  %516 = load i16, ptr %515
  %517 = icmp eq i16 %516, 0
  br i1 %517, label %b263, label %b259

b257:
  %518 = add nuw i16 %513, 1
  %519 = and i16 %518, %511
  br label %b253

b259:
  %520 = icmp eq i16 %516, %500
  br i1 %520, label %521, label %b257

521:
  %522 = getelementptr i8, ptr %499, i16 %514
  %523 = getelementptr i8, ptr %522, i16 2
  %524 = load i16, ptr %523
  %525 = icmp eq i16 %524, %497
  br i1 %525, label %b271, label %b257

b263:
  %.ph10 = phi i16 [ 0, %496 ], [ %513, %b253 ]
  %526 = icmp ult i16 %.ph10, %502
  br i1 %526, label %b266, label %b267

b266:
  %527 = mul i16 %.ph10, 6
  %528 = getelementptr inbounds i8, ptr %499, i16 %527
  store i16 %500, ptr %528
  %529 = getelementptr i8, ptr %499, i16 -4
  %530 = load i16, ptr %529
  %531 = icmp ult i16 %.ph10, %530
  br i1 %531, label %b270, label %b269

b267:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b269:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b270:
  %532 = getelementptr i8, ptr %499, i16 %527
  %533 = getelementptr i8, ptr %532, i16 2
  store i16 %497, ptr %533
  %534 = getelementptr i8, ptr %499, i16 -2
  %535 = load i16, ptr %534
  %536 = add i16 %535, 1
  store i16 %536, ptr %534
  %537 = getelementptr i8, ptr %499, i16 -4
  %538 = load i16, ptr %537
  br label %b271

b271:
  %539 = phi i16 [ %538, %b270 ], [ %502, %521 ]
  %540 = phi i16 [ %.ph10, %b270 ], [ %513, %521 ]
  %541 = icmp ult i16 %540, %539
  br i1 %541, label %b272, label %b273

b272:
  %542 = mul i16 %540, 6
  %543 = getelementptr inbounds i8, ptr %499, i16 %542
  %544 = mul nuw i16 %497, %497
  %545 = getelementptr i8, ptr %543, i16 4
  store i16 %544, ptr %545
  %546 = add nuw nsw i16 %497, 1
  %exitcond6 = icmp eq i16 %546, 40
  br i1 %exitcond6, label %504, label %496

b273:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b274:
  %547 = add i16 %508, -1
  %548 = and i16 %547, 37
  br label %b277

b277:
  %549 = phi i16 [ %548, %b274 ], [ %558, %b284 ]
  %550 = icmp ult i16 %549, %508
  br i1 %550, label %b280, label %b281

b280:
  %551 = mul i16 %549, 6
  %552 = getelementptr inbounds i8, ptr %499, i16 %551
  %553 = load i16, ptr %552
  %554 = icmp eq i16 %553, 0
  br i1 %554, label %b291, label %555

555:
  %556 = icmp eq i16 %553, 37
  br i1 %556, label %559, label %b284

b281:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

b284:
  %557 = add nuw i16 %549, 1
  %558 = and i16 %557, %547
  br label %b277

559:
  %560 = getelementptr i8, ptr %499, i16 %551
  %561 = getelementptr i8, ptr %560, i16 2
  %562 = load i16, ptr %561
  %563 = icmp eq i16 %562, 37
  br i1 %563, label %b292, label %b284

b291:
  call cc 1005 addrspace(1) void @N$EKEY()
  unreachable

b292:
  %564 = getelementptr i8, ptr %499, i16 %551
  %565 = getelementptr i8, ptr %564, i16 4
  %566 = load i16, ptr %565
  call cc 1005 addrspace(1) void @N$PI2(i16 %566)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %499)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %351)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %352)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %243)
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  call cc 1005 addrspace(1) void @N$BDRP(ptr getelementptr inbounds nuw (i8, ptr @$str2, i16 6))
  %567 = icmp eq ptr %56, null
  br i1 %567, label %b294, label %b295

b294:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %56)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %568 = icmp eq ptr %8, null
  br i1 %568, label %b304, label %b305

b295:
  %569 = getelementptr i8, ptr %56, i16 -4
  %570 = load i16, ptr %569
  %571 = icmp eq i16 %570, 0
  br i1 %571, label %b294, label %572

572:
  %573 = getelementptr i8, ptr %56, i16 2
  br label %b298

b298:
  %574 = phi ptr [ %573, %572 ], [ %578, %b298 ]
  %575 = phi i16 [ %570, %572 ], [ %577, %b298 ]
  %576 = load ptr, ptr %574
  call cc 1005 addrspace(1) void @N$BDRP(ptr %576)
  %577 = add i16 %575, -1
  %578 = getelementptr i8, ptr %574, i16 6
  %exitcond5 = icmp eq i16 %577, 0
  br i1 %exitcond5, label %b294, label %b298

b304:
  call cc 1005 addrspace(1) void @N$BDRP(ptr %8)
  ret i16 0

b305:
  %579 = getelementptr i8, ptr %8, i16 -4
  %580 = load i16, ptr %579
  %581 = icmp eq i16 %580, 0
  br i1 %581, label %b304, label %582

582:
  br label %b308

b308:
  %583 = phi ptr [ %8, %582 ], [ %587, %b308 ]
  %584 = phi i16 [ %580, %582 ], [ %586, %b308 ]
  %585 = load ptr, ptr %583
  call cc 1005 addrspace(1) void @N$BDRP(ptr %585)
  %586 = add i16 %584, -1
  %587 = getelementptr i8, ptr %583, i16 2
  %exitcond4 = icmp eq i16 %586, 0
  br i1 %exitcond4, label %b304, label %b308
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
