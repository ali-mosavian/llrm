target datalayout = "e-p:16:16-p1:32:16:16:16-p2:16:16-p3:32:16:16:32-p4:32:16:16:16-i32:16-i64:16-f80:16-n8:16:32-ni:2:3"

@$str1 = internal constant [7 x i8] c"\08\00\00\00\00\00\00"
@$str3 = internal constant [15 x i8] c"\08\00\08\00\08\00 planes \00"
@$str4 = internal constant [14 x i8] c"\08\00\07\00\07\00 nodes \00"
@$str5 = internal constant [15 x i8] c"\08\00\08\00\08\00 faces, \00"
@$str6 = internal constant [13 x i8] c"\08\00\06\00\06\00 bytes\00"
@$str7 = internal constant [8 x i8] c"\08\00\01\00\01\00 \00"
@$str8 = internal constant [8 x i8] c"\08\00\01\00\01\00(\00"
@$str9 = internal constant [8 x i8] c"\08\00\01\00\01\00,\00"
@$str10 = internal constant [14 x i8] c"\08\00\07\00\07\00) leaf \00"
@$str11 = internal constant [13 x i8] c"\08\00\06\00\06\00 node \00"
@$str12 = internal constant [8 x i8] c"\08\00\01\00\01\00:\00"
@$str13 = internal constant [37 x i8] c"\08\00\1E\00\1E\00cannot read or write LEVEL.DAT\00"
@$str14 = internal constant [31 x i8] c"\08\00\18\00\18\00LEVEL.DAT is not a level\00"
@$str15 = internal constant [20 x i8] c"\08\00\0D\00\0D\00LEVEL.DAT is \00"
@$str16 = internal constant [19 x i8] c"\08\00\0C\00\0C\00 bytes short\00"
@switch.table.0 = private constant [4 x i8] [i8 1, i8 2, i8 0, i8 0]

define cc 1005 range(i16 0, 2) i16 @main() addrspace(1) nearcode "llrm-fp-eval"="x87-extended" {
  %1 = alloca [3 x i8]
  %2 = alloca [9 x i8]
  %3 = alloca [4 x i8]
  %4 = alloca [80 x i8]
  %5 = alloca [8 x i8]
  %6 = alloca [80 x i8]
  %7 = alloca [8 x i8]
  %8 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 3, i16 4)
  store i8 1, ptr %8
  %9 = getelementptr i8, ptr %8, i16 1
  store i8 0, ptr %9
  %10 = getelementptr i8, ptr %8, i16 2
  store i16 32, ptr %10
  %11 = getelementptr inbounds nuw i8, ptr %8, i16 4
  store i8 0, ptr %11, align 2
  %12 = getelementptr i8, ptr %8, i16 5
  store i8 1, ptr %12
  %13 = getelementptr i8, ptr %8, i16 6
  store i16 32, ptr %13
  %14 = getelementptr inbounds nuw i8, ptr %8, i16 8
  store i8 0, ptr %14, align 2
  %15 = getelementptr i8, ptr %8, i16 9
  store i8 1, ptr %15
  %16 = getelementptr i8, ptr %8, i16 10
  store i16 16, ptr %16
  %17 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 3, i16 9)
  store i16 0, ptr %17
  %18 = getelementptr i8, ptr %17, i16 2
  store i16 1, ptr %18
  %19 = getelementptr i8, ptr %17, i16 4
  store i16 2, ptr %19
  %20 = getelementptr i8, ptr %17, i16 6
  store i16 0, ptr %20
  %21 = getelementptr i8, ptr %17, i16 8
  store i8 1, ptr %21, align 2
  %22 = getelementptr inbounds nuw i8, ptr %17, i16 9
  store i16 1, ptr %22
  %23 = getelementptr i8, ptr %17, i16 11
  store i16 -1, ptr %23
  %24 = getelementptr i8, ptr %17, i16 13
  store i16 -2, ptr %24
  %25 = getelementptr i8, ptr %17, i16 15
  store i16 1, ptr %25
  %26 = getelementptr i8, ptr %17, i16 17
  store i8 2, ptr %26
  %27 = getelementptr inbounds nuw i8, ptr %17, i16 18
  store i16 2, ptr %27
  %28 = getelementptr i8, ptr %17, i16 20
  store i16 -3, ptr %28
  %29 = getelementptr i8, ptr %17, i16 22
  store i16 -4, ptr %29
  %30 = getelementptr i8, ptr %17, i16 24
  store i16 3, ptr %30
  %31 = getelementptr i8, ptr %17, i16 26
  store i8 2, ptr %31, align 2
  %32 = tail call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 5, i16 3)
  store i16 0, ptr %32
  %33 = getelementptr i8, ptr %32, i16 2
  store i8 2, ptr %33, align 2
  %34 = getelementptr inbounds nuw i8, ptr %32, i16 3
  store i16 1, ptr %34
  %35 = getelementptr i8, ptr %32, i16 5
  store i8 4, ptr %35
  %36 = getelementptr inbounds nuw i8, ptr %32, i16 6
  store i16 1, ptr %36
  %37 = getelementptr i8, ptr %32, i16 8
  store i8 5, ptr %37, align 2
  %38 = getelementptr inbounds nuw i8, ptr %32, i16 9
  store i16 2, ptr %38
  %39 = getelementptr i8, ptr %32, i16 11
  store i8 6, ptr %39
  %40 = getelementptr inbounds nuw i8, ptr %32, i16 12
  store i16 2, ptr %40
  %41 = getelementptr i8, ptr %32, i16 14
  store i8 9, ptr %41, align 2
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  tail call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %7)
  call void @llvm.lifetime.start.p0(i64 80, ptr nonnull %6)
  %42 = getelementptr inbounds nuw i8, ptr %6, i16 10
  call void @llvm.memset.p0.i16(ptr nonnull align 1 %42, i8 0, i16 70, i1 false)
  %43 = addrspacecast ptr %6 to ptr addrspace(5)
  %44 = addrspacecast ptr %6 to ptr addrspace(1)
  store i32 1163281740, ptr addrspace(5) %43
  %45 = getelementptr inbounds nuw i8, ptr addrspace(5) %43, i16 4
  store i32 1094987340, ptr addrspace(5) %45
  %46 = getelementptr inbounds nuw i8, ptr addrspace(5) %43, i16 8
  store i16 84, ptr addrspace(5) %46
  %47 = call addrspace(1) i16 @LL$CREATE(ptr addrspace(1) %44)
  %48 = icmp slt i16 %47, 0
  br i1 %48, label %49, label %54

49:
  %50 = add i16 %47, 5
  %51 = icmp ult i16 %50, 4
  br i1 %51, label %switch.lookup, label %53

switch.lookup:
  %52 = getelementptr inbounds nuw i8, ptr @switch.table.0, i16 %50
  %switch.load = load i8, ptr %52
  %switch.ext = zext i8 %switch.load to i32
  br label %53

53:
  %common.op41 = phi i32 [ %switch.ext, %switch.lookup ], [ 2, %49 ]
  call void @llvm.lifetime.end.p0(i64 80, ptr nonnull %6)
  br label %136

54:
  call void @llvm.lifetime.end.p0(i64 80, ptr nonnull %6)
  %55 = getelementptr i8, ptr %8, i16 -4
  %56 = load i16, ptr %55
  %57 = getelementptr i8, ptr %17, i16 -4
  %58 = load i16, ptr %57
  %59 = getelementptr i8, ptr %32, i16 -4
  %60 = load i16, ptr %59
  store i16 22092, ptr %7, !tbaa !2
  %61 = getelementptr inbounds nuw i8, ptr %7, i16 2
  store i16 %56, ptr %61, !tbaa !2
  %62 = getelementptr inbounds nuw i8, ptr %7, i16 4
  store i16 %58, ptr %62, !tbaa !2
  %63 = getelementptr inbounds nuw i8, ptr %7, i16 6
  store i16 %60, ptr %63, !tbaa !2
  %64 = addrspacecast ptr %7 to ptr addrspace(1)
  %65 = call addrspace(1) i32 @LL$WRITE_FILE(i16 %47, ptr addrspace(1) %64, i16 8)
  %66 = icmp slt i32 %65, 0
  br i1 %66, label %67, label %79

67:
  %68 = trunc i32 %65 to i16
  %69 = sub i16 %68, -3
  %70 = icmp ule i16 %69, 1
  br i1 %70, label %77, label %71

71:
  %72 = icmp eq i16 %68, -5
  br i1 %72, label %73, label %74

73:
  br label %77

74:
  %75 = mul i32 %65, -65536
  %76 = or disjoint i32 %75, 2
  br label %77

77:
  %common.op31 = phi i32 [ 1, %73 ], [ %76, %74 ], [ 0, %67 ]
  %78 = call addrspace(1) i16 @LL$CLOSE(i16 %47)
  br label %136

79:
  %80 = addrspacecast ptr %8 to ptr addrspace(1)
  %81 = getelementptr i8, ptr %8, i16 -4
  %82 = load i16, ptr %81
  %83 = shl i16 %82, 2
  %84 = call addrspace(1) i32 @LL$WRITE_FILE(i16 %47, ptr addrspace(1) %80, i16 %83)
  %85 = icmp slt i32 %84, 0
  br i1 %85, label %86, label %98

86:
  %87 = trunc i32 %84 to i16
  %88 = sub i16 %87, -3
  %89 = icmp ule i16 %88, 1
  br i1 %89, label %96, label %90

90:
  %91 = icmp eq i16 %87, -5
  br i1 %91, label %92, label %93

92:
  br label %96

93:
  %94 = mul i32 %84, -65536
  %95 = or disjoint i32 %94, 2
  br label %96

96:
  %.ph = phi i32 [ 1, %92 ], [ %95, %93 ], [ 0, %86 ]
  %97 = call addrspace(1) i16 @LL$CLOSE(i16 %47)
  br label %136

98:
  %99 = addrspacecast ptr %17 to ptr addrspace(1)
  %100 = getelementptr i8, ptr %17, i16 -4
  %101 = load i16, ptr %100
  %102 = mul i16 %101, 9
  %103 = call addrspace(1) i32 @LL$WRITE_FILE(i16 %47, ptr addrspace(1) %99, i16 %102)
  %104 = icmp slt i32 %103, 0
  br i1 %104, label %105, label %117

105:
  %106 = trunc i32 %103 to i16
  %107 = sub i16 %106, -3
  %108 = icmp ule i16 %107, 1
  br i1 %108, label %115, label %109

109:
  %110 = icmp eq i16 %106, -5
  br i1 %110, label %111, label %112

111:
  br label %115

112:
  %113 = mul i32 %103, -65536
  %114 = or disjoint i32 %113, 2
  br label %115

115:
  %.ph1 = phi i32 [ 1, %111 ], [ %114, %112 ], [ 0, %105 ]
  %116 = call addrspace(1) i16 @LL$CLOSE(i16 %47)
  br label %136

117:
  %118 = addrspacecast ptr %32 to ptr addrspace(1)
  %119 = getelementptr i8, ptr %32, i16 -4
  %120 = load i16, ptr %119
  %121 = mul i16 %120, 3
  %122 = call addrspace(1) i32 @LL$WRITE_FILE(i16 %47, ptr addrspace(1) %118, i16 %121)
  %123 = icmp slt i32 %122, 0
  br i1 %123, label %124, label %137

124:
  %125 = trunc i32 %122 to i16
  %126 = sub i16 %125, -3
  %127 = icmp ule i16 %126, 1
  br i1 %127, label %134, label %128

128:
  %129 = icmp eq i16 %125, -5
  br i1 %129, label %130, label %131

130:
  br label %134

131:
  %132 = mul i32 %122, -65536
  %133 = or disjoint i32 %132, 2
  br label %134

134:
  %.ph2 = phi i32 [ 1, %130 ], [ %133, %131 ], [ 0, %124 ]
  %135 = call addrspace(1) i16 @LL$CLOSE(i16 %47)
  br label %136

136:
  %.ph3 = phi i32 [ %common.op41, %53 ], [ %common.op31, %77 ], [ %.ph, %96 ], [ %.ph1, %115 ], [ %.ph2, %134 ]
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %7)
  %extract.trunc1 = trunc i32 %.ph3 to i16
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %32)
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %17)
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %8)
  br label %b3

137:
  %138 = call addrspace(1) i16 @LL$CLOSE(i16 %47)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %7)
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %32)
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %17)
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %8)
  call void @llvm.lifetime.start.p0(i64 8, ptr nonnull %5)
  call void @llvm.lifetime.start.p0(i64 80, ptr nonnull %4)
  %139 = getelementptr inbounds nuw i8, ptr %4, i16 10
  call void @llvm.memset.p0.i16(ptr nonnull align 1 %139, i8 0, i16 70, i1 false)
  %140 = addrspacecast ptr %4 to ptr addrspace(5)
  %141 = addrspacecast ptr %4 to ptr addrspace(1)
  store i32 1163281740, ptr addrspace(5) %140
  %142 = getelementptr inbounds nuw i8, ptr addrspace(5) %140, i16 4
  store i32 1094987340, ptr addrspace(5) %142
  %143 = getelementptr inbounds nuw i8, ptr addrspace(5) %140, i16 8
  store i16 84, ptr addrspace(5) %143
  %144 = call addrspace(1) i16 @LL$OPEN(ptr addrspace(1) %141, i8 zeroext 0)
  %145 = icmp slt i16 %144, 0
  br i1 %145, label %146, label %152

146:
  %147 = add i16 %144, 5
  %148 = icmp ult i16 %147, 4
  br i1 %148, label %switch.lookup1, label %150

switch.lookup1:
  %149 = getelementptr inbounds nuw i8, ptr @switch.table.0, i16 %147
  %switch.load1 = load i8, ptr %149
  %switch.ext1 = zext i8 %switch.load1 to i16
  br label %150

150:
  %151 = phi i16 [ %switch.ext1, %switch.lookup1 ], [ 2, %146 ]
  call void @llvm.lifetime.end.p0(i64 80, ptr nonnull %4)
  br label %316

152:
  call void @llvm.lifetime.end.p0(i64 80, ptr nonnull %4)
  %153 = addrspacecast ptr %5 to ptr addrspace(1)
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %5, i8 0, i16 8, i1 false)
  %154 = call addrspace(1) i32 @LL$READ(i16 %144, ptr addrspace(1) %153, i16 8)
  %155 = icmp slt i32 %154, 0
  %156 = trunc i32 %154 to i16
  br i1 %155, label %157, label %166

157:
  %158 = sub i16 %156, -3
  %159 = icmp ule i16 %158, 1
  br i1 %159, label %170, label %160

160:
  %161 = icmp eq i16 %156, -5
  br i1 %161, label %162, label %163

162:
  br label %170

163:
  %164 = mul i32 %154, -65536
  %165 = or disjoint i32 %164, 2
  br label %170

166:
  %167 = icmp ult i16 %156, 8
  br i1 %167, label %168, label %175

168:
  %169 = sub nsw i32 8, %154
  br label %170

170:
  %171 = phi i32 [ %169, %168 ], [ 1, %162 ], [ %165, %163 ], [ 0, %157 ]
  %172 = phi i16 [ 2, %168 ], [ 0, %163 ], [ 0, %162 ], [ 0, %157 ]
  %173 = inttoptr i16 %172 to ptr
  %extract.trunc3 = trunc i32 %171 to i16
  %174 = call addrspace(1) i16 @LL$CLOSE(i16 %144)
  br label %316

175:
  %176 = load i16, ptr %5, !tbaa !2
  %177 = icmp eq i16 %176, 22092
  br i1 %177, label %180, label %178

178:
  %179 = call addrspace(1) i16 @LL$CLOSE(i16 %144)
  br label %316

180:
  %181 = getelementptr i8, ptr %5, i16 2
  %182 = load i16, ptr %181, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 4, ptr nonnull %3)
  %183 = addrspacecast ptr %3 to ptr addrspace(5)
  %184 = addrspacecast ptr %3 to ptr addrspace(1)
  store i16 0, ptr addrspace(5) %183, align 1
  %185 = getelementptr inbounds nuw i8, ptr %3, i16 2
  store i16 0, ptr %185, !tbaa !2
  %186 = icmp eq i16 %182, 0
  br i1 %186, label %225, label %187

187:
  br label %188

188:
  %189 = phi i16 [ %219, %207 ], [ %182, %187 ]
  %190 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %187 ], [ %209, %207 ]
  %191 = call addrspace(1) i32 @LL$READ(i16 %144, ptr addrspace(1) %184, i16 4)
  %192 = icmp slt i32 %191, 0
  %193 = trunc i32 %191 to i16
  br i1 %192, label %194, label %203

194:
  %195 = sub i16 %193, -3
  %196 = icmp ule i16 %195, 1
  br i1 %196, label %220, label %197

197:
  %198 = icmp eq i16 %193, -5
  br i1 %198, label %199, label %200

199:
  br label %220

200:
  %201 = mul i32 %191, -65536
  %202 = or disjoint i32 %201, 2
  br label %220

203:
  %204 = icmp ult i16 %193, 4
  br i1 %204, label %205, label %207

205:
  %206 = sub nsw i32 4, %191
  br label %220

207:
  %.phi.trans.insert = getelementptr i8, ptr %190, i16 -4
  %208 = load i16, ptr %.phi.trans.insert
  %209 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %190, i16 1, i16 4)
  %210 = shl i16 %208, 2
  %211 = getelementptr inbounds i8, ptr %209, i16 %210
  %212 = getelementptr i8, ptr %3, i16 2
  %213 = load i16, ptr %212, !tbaa !2
  %214 = load i8, ptr addrspace(5) %183, align 2
  store i8 %214, ptr %211
  %215 = getelementptr inbounds nuw i8, ptr %211, i16 1
  %216 = getelementptr i8, ptr addrspace(5) %183, i16 1
  %217 = load i8, ptr addrspace(5) %216
  store i8 %217, ptr %215
  %218 = getelementptr i8, ptr %211, i16 2
  store i16 %213, ptr %218
  %219 = add i16 %189, -1
  %exitcond22 = icmp eq i16 %219, 0
  br i1 %exitcond22, label %225, label %188

220:
  %221 = phi i32 [ %206, %205 ], [ 1, %199 ], [ %202, %200 ], [ 0, %194 ]
  %222 = phi i16 [ 2, %205 ], [ 0, %200 ], [ 0, %199 ], [ 0, %194 ]
  %223 = inttoptr i16 %222 to ptr
  call cc 1005 addrspace(1) void @N$BDRP(ptr %190)
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %3)
  %extract.trunc4 = trunc i32 %221 to i16
  %224 = call addrspace(1) i16 @LL$CLOSE(i16 %144)
  br label %316

225:
  %.ph7 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %180 ], [ %209, %207 ]
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call void @llvm.lifetime.end.p0(i64 4, ptr nonnull %3)
  %226 = getelementptr i8, ptr %5, i16 4
  %227 = load i16, ptr %226, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 9, ptr nonnull %2)
  %228 = getelementptr inbounds nuw i8, ptr %2, i16 2
  %229 = addrspacecast ptr %228 to ptr addrspace(5)
  %230 = icmp eq i16 %227, 0
  call void @llvm.memset.p0.i16(ptr nonnull align 2 %2, i8 0, i16 9, i1 false)
  br i1 %230, label %275, label %231

231:
  %232 = addrspacecast ptr %2 to ptr addrspace(1)
  br label %233

233:
  %234 = phi i16 [ %269, %252 ], [ %227, %231 ]
  %235 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %231 ], [ %254, %252 ]
  %236 = call addrspace(1) i32 @LL$READ(i16 %144, ptr addrspace(1) %232, i16 9)
  %237 = icmp slt i32 %236, 0
  %238 = trunc i32 %236 to i16
  br i1 %237, label %239, label %248

239:
  %240 = sub i16 %238, -3
  %241 = icmp ule i16 %240, 1
  br i1 %241, label %270, label %242

242:
  %243 = icmp eq i16 %238, -5
  br i1 %243, label %244, label %245

244:
  br label %270

245:
  %246 = mul i32 %236, -65536
  %247 = or disjoint i32 %246, 2
  br label %270

248:
  %249 = icmp ult i16 %238, 9
  br i1 %249, label %250, label %252

250:
  %251 = sub nsw i32 9, %236
  br label %270

252:
  %.phi.trans.insert1 = getelementptr i8, ptr %235, i16 -4
  %253 = load i16, ptr %.phi.trans.insert1
  %254 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %235, i16 1, i16 9)
  %255 = mul i16 %253, 9
  %256 = getelementptr inbounds i8, ptr %254, i16 %255
  %257 = load i16, ptr %2, !tbaa !2
  %258 = getelementptr i8, ptr %2, i16 6
  %259 = load i16, ptr %258, !tbaa !2
  %260 = getelementptr i8, ptr %2, i16 8
  %261 = load i8, ptr %260, align 2, !tbaa !2
  store i16 %257, ptr %256
  %262 = getelementptr i8, ptr %256, i16 2
  %263 = load i16, ptr addrspace(5) %229
  store i16 %263, ptr %262
  %264 = getelementptr i8, ptr %256, i16 4
  %265 = getelementptr i8, ptr addrspace(5) %229, i16 2
  %266 = load i16, ptr addrspace(5) %265
  store i16 %266, ptr %264
  %267 = getelementptr i8, ptr %256, i16 6
  store i16 %259, ptr %267
  %268 = getelementptr i8, ptr %256, i16 8
  store i8 %261, ptr %268, align 2
  %269 = add i16 %234, -1
  %exitcond21 = icmp eq i16 %269, 0
  br i1 %exitcond21, label %275, label %233

270:
  %271 = phi i32 [ %251, %250 ], [ 1, %244 ], [ %247, %245 ], [ 0, %239 ]
  %272 = phi i16 [ 2, %250 ], [ 0, %245 ], [ 0, %244 ], [ 0, %239 ]
  %273 = inttoptr i16 %272 to ptr
  call cc 1005 addrspace(1) void @N$BDRP(ptr %235)
  call void @llvm.lifetime.end.p0(i64 9, ptr nonnull %2)
  %extract.trunc5 = trunc i32 %271 to i16
  call cc 1005 addrspace(1) void @N$BDRP(ptr %.ph7)
  %274 = call addrspace(1) i16 @LL$CLOSE(i16 %144)
  br label %316

275:
  %.ph8 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %225 ], [ %254, %252 ]
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call void @llvm.lifetime.end.p0(i64 9, ptr nonnull %2)
  %276 = getelementptr i8, ptr %5, i16 6
  %277 = load i16, ptr %276, !tbaa !2
  call void @llvm.lifetime.start.p0(i64 3, ptr nonnull %1)
  store i16 0, ptr %1, !tbaa !2
  %278 = getelementptr inbounds nuw i8, ptr %1, i16 2
  store i8 0, ptr %278, align 2, !tbaa !2
  %279 = icmp eq i16 %277, 0
  br i1 %279, label %317, label %280

280:
  %281 = addrspacecast ptr %1 to ptr addrspace(1)
  br label %282

282:
  %283 = phi i16 [ %310, %301 ], [ %277, %280 ]
  %284 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %280 ], [ %303, %301 ]
  %285 = call addrspace(1) i32 @LL$READ(i16 %144, ptr addrspace(1) %281, i16 3)
  %286 = icmp slt i32 %285, 0
  %287 = trunc i32 %285 to i16
  br i1 %286, label %288, label %297

288:
  %289 = sub i16 %287, -3
  %290 = icmp ule i16 %289, 1
  br i1 %290, label %311, label %291

291:
  %292 = icmp eq i16 %287, -5
  br i1 %292, label %293, label %294

293:
  br label %311

294:
  %295 = mul i32 %285, -65536
  %296 = or disjoint i32 %295, 2
  br label %311

297:
  %298 = icmp ult i16 %287, 3
  br i1 %298, label %299, label %301

299:
  %300 = sub nsw i32 3, %285
  br label %311

301:
  %.phi.trans.insert2 = getelementptr i8, ptr %284, i16 -4
  %302 = load i16, ptr %.phi.trans.insert2
  %303 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr %284, i16 1, i16 3)
  %304 = mul i16 %302, 3
  %305 = getelementptr inbounds i8, ptr %303, i16 %304
  %306 = load i16, ptr %1, !tbaa !2
  %307 = getelementptr i8, ptr %1, i16 2
  %308 = load i8, ptr %307, align 2, !tbaa !2
  store i16 %306, ptr %305
  %309 = getelementptr i8, ptr %305, i16 2
  store i8 %308, ptr %309, align 2
  %310 = add i16 %283, -1
  %exitcond2 = icmp eq i16 %310, 0
  br i1 %exitcond2, label %317, label %282

311:
  %312 = phi i32 [ %300, %299 ], [ 1, %293 ], [ %296, %294 ], [ 0, %288 ]
  %313 = phi i16 [ 2, %299 ], [ 0, %294 ], [ 0, %293 ], [ 0, %288 ]
  %314 = inttoptr i16 %313 to ptr
  call cc 1005 addrspace(1) void @N$BDRP(ptr %284)
  call void @llvm.lifetime.end.p0(i64 3, ptr nonnull %1)
  %extract.trunc6 = trunc i32 %312 to i16
  call cc 1005 addrspace(1) void @N$BDRP(ptr %.ph8)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %.ph7)
  %315 = call addrspace(1) i16 @LL$CLOSE(i16 %144)
  br label %316

316:
  %.ph4 = phi i16 [ %151, %150 ], [ %extract.trunc3, %170 ], [ %extract.trunc6, %311 ], [ %extract.trunc5, %270 ], [ %extract.trunc4, %220 ], [ undef, %178 ]
  %.ph10 = phi ptr [ null, %150 ], [ %173, %170 ], [ %314, %311 ], [ %273, %270 ], [ %223, %220 ], [ inttoptr (i16 1 to ptr), %178 ]
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %5)
  br label %b3

317:
  %.ph9 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %275 ], [ %303, %301 ]
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call void @llvm.lifetime.end.p0(i64 3, ptr nonnull %1)
  %318 = ptrtoint ptr %.ph8 to i16
  %319 = ptrtoint ptr %.ph9 to i16
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  call cc 1005 addrspace(1) void @N$BDRP(ptr null)
  %320 = call addrspace(1) i16 @LL$CLOSE(i16 %144)
  call void @llvm.lifetime.end.p0(i64 8, ptr nonnull %5)
  %321 = inttoptr i16 %318 to ptr
  %322 = inttoptr i16 %319 to ptr
  %323 = getelementptr i8, ptr %.ph7, i16 -4
  %324 = load i16, ptr %323
  %325 = getelementptr i8, ptr %321, i16 -4
  %326 = load i16, ptr %325
  %327 = getelementptr i8, ptr %322, i16 -4
  %328 = load i16, ptr %327
  %329 = shl i16 %324, 2
  %330 = add i16 %329, 8
  %331 = mul i16 %326, 9
  %332 = add i16 %330, %331
  %333 = mul i16 %328, 3
  %334 = add i16 %332, %333
  call cc 1005 addrspace(1) void @N$PU2(i16 %324)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str3, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %326)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str4, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %328)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str5, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %334)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str6, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  %335 = call cc 1005 addrspace(1) ptr @N$BGRW(ptr getelementptr inbounds nuw (i8, ptr @$str1, i16 6), i16 4, i16 4)
  store i16 40, ptr %335
  %336 = getelementptr i8, ptr %335, i16 2
  store i16 50, ptr %336
  %337 = getelementptr inbounds nuw i8, ptr %335, i16 4
  store i16 40, ptr %337
  %338 = getelementptr i8, ptr %335, i16 6
  store i16 10, ptr %338
  %339 = getelementptr inbounds nuw i8, ptr %335, i16 8
  store i16 8, ptr %339
  %340 = getelementptr i8, ptr %335, i16 10
  store i16 20, ptr %340
  %341 = getelementptr inbounds nuw i8, ptr %335, i16 12
  store i16 8, ptr %341
  %342 = getelementptr i8, ptr %335, i16 14
  store i16 4, ptr %342
  %343 = getelementptr i8, ptr %335, i16 -4
  %344 = load i16, ptr %343
  %345 = icmp eq i16 %344, 0
  br i1 %345, label %469, label %346

346:
  %347 = ptrtoint ptr %.ph9 to i16
  %348 = inttoptr i16 %347 to ptr
  %349 = getelementptr i8, ptr %348, i16 2
  br label %350

350:
  %351 = phi i16 [ %456, %449 ], [ 0, %346 ]
  %352 = inttoptr i16 %318 to ptr
  %353 = shl i16 %351, 2
  %354 = getelementptr inbounds i8, ptr %335, i16 %353
  %355 = getelementptr i8, ptr %354, i16 2
  %356 = load i16, ptr %354
  %357 = load i16, ptr %355
  %358 = getelementptr i8, ptr %352, i16 -4
  %359 = load i16, ptr %358
  %360 = icmp eq i16 %359, 0
  br i1 %360, label %371, label %392

361:
  %362 = phi i16 [ %389, %372 ], [ %418, %420 ]
  %363 = icmp ult i16 %362, %359
  br i1 %363, label %364, label %371

364:
  %365 = ptrtoint ptr %.ph8 to i16
  %366 = inttoptr i16 %365 to ptr
  %367 = mul i16 %362, 9
  %368 = getelementptr inbounds i8, ptr %366, i16 %367
  %369 = load i16, ptr %368
  %370 = icmp ult i16 %369, %397
  br i1 %370, label %372, label %391

371:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

372:
  %373 = shl i16 %369, 2
  %374 = getelementptr inbounds i8, ptr %.ph7, i16 %373
  %375 = load i8, ptr %374
  %376 = sext i8 %375 to i16
  %377 = mul i16 %356, %376
  %378 = getelementptr inbounds nuw i8, ptr %374, i16 1
  %379 = load i8, ptr %378
  %380 = sext i8 %379 to i16
  %381 = mul i16 %357, %380
  %382 = getelementptr i8, ptr %374, i16 2
  %383 = load i16, ptr %382
  %384 = sub i16 %377, %383
  %385 = add i16 %384, %381
  %386 = icmp sgt i16 %385, -1
  %387 = select i1 %386, i16 2, i16 4
  %388 = getelementptr i8, ptr %368, i16 %387
  %389 = load i16, ptr %388
  %390 = icmp sgt i16 %389, -1
  br i1 %390, label %361, label %421

391:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

392:
  %393 = ptrtoint ptr %.ph8 to i16
  %394 = inttoptr i16 %393 to ptr
  %395 = load i16, ptr %394
  %396 = getelementptr i8, ptr %.ph7, i16 -4
  %397 = load i16, ptr %396
  %398 = icmp ult i16 %395, %397
  br i1 %398, label %399, label %391

399:
  %400 = ptrtoint ptr %.ph8 to i16
  %401 = inttoptr i16 %400 to ptr
  %402 = shl i16 %395, 2
  %403 = getelementptr inbounds i8, ptr %.ph7, i16 %402
  %404 = load i8, ptr %403
  %405 = sext i8 %404 to i16
  %406 = mul i16 %356, %405
  %407 = getelementptr inbounds nuw i8, ptr %403, i16 1
  %408 = load i8, ptr %407
  %409 = sext i8 %408 to i16
  %410 = mul i16 %357, %409
  %411 = getelementptr i8, ptr %403, i16 2
  %412 = load i16, ptr %411
  %413 = sub i16 %406, %412
  %414 = add i16 %413, %410
  %415 = icmp sgt i16 %414, -1
  %416 = select i1 %415, i16 2, i16 4
  %417 = getelementptr i8, ptr %401, i16 %416
  %418 = load i16, ptr %417
  %419 = icmp sgt i16 %418, -1
  br i1 %419, label %420, label %421

420:
  br label %361

421:
  %422 = phi i16 [ %418, %399 ], [ %389, %372 ]
  %423 = phi i16 [ 0, %399 ], [ %362, %372 ]
  %424 = ptrtoint ptr %.ph8 to i16
  %425 = inttoptr i16 %424 to ptr
  %426 = xor i16 %422, -1
  %427 = mul i16 %423, 9
  %428 = getelementptr inbounds i8, ptr %425, i16 %427
  %429 = getelementptr i8, ptr %428, i16 6
  %430 = load i16, ptr %429
  %431 = getelementptr i8, ptr %428, i16 8
  %432 = load i8, ptr %431, align 2
  %433 = zext i8 %432 to i16
  %434 = zext i8 %432 to i16
  %435 = add i16 %430, %434
  %436 = icmp ult i16 %430, %435
  br i1 %436, label %437, label %449

437:
  %438 = mul i16 %430, 3
  %439 = getelementptr i8, ptr %349, i16 %438
  br label %440

440:
  %441 = phi ptr [ %439, %437 ], [ %467, %457 ]
  %442 = phi i16 [ %433, %437 ], [ %466, %457 ]
  %443 = phi i16 [ %465, %457 ], [ %430, %437 ]
  %444 = phi ptr [ %464, %457 ], [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %437 ]
  %445 = inttoptr i16 %319 to ptr
  %446 = getelementptr i8, ptr %445, i16 -4
  %447 = load i16, ptr %446
  %448 = icmp ult i16 %443, %447
  br i1 %448, label %457, label %468

449:
  %450 = phi ptr [ getelementptr inbounds nuw (i8, ptr @$str1, i16 6), %421 ], [ %464, %457 ]
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str8, i16 6))
  %451 = getelementptr i8, ptr %335, i16 %353
  %452 = load i16, ptr %451
  call cc 1005 addrspace(1) void @N$PI2(i16 %452)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str9, i16 6))
  %453 = getelementptr i8, ptr %335, i16 %353
  %454 = getelementptr i8, ptr %453, i16 2
  %455 = load i16, ptr %454
  call cc 1005 addrspace(1) void @N$PI2(i16 %455)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str10, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %426)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str11, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %423)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str12, i16 6))
  call cc 1005 addrspace(1) void @N$PS(ptr %450)
  call cc 1005 addrspace(1) void @N$PN()
  call cc 1005 addrspace(1) void @N$BDRP(ptr %450)
  %456 = add nuw i16 %351, 1
  %exitcond3 = icmp eq i16 %456, %344
  br i1 %exitcond3, label %469, label %350

457:
  call cc 1005 addrspace(1) void @N$PBEG()
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str7, i16 6))
  %458 = load i8, ptr %441
  %459 = and i8 %458, 1
  %460 = icmp eq i8 %459, 0
  %461 = select i1 %460, i8 102, i8 98
  call cc 1005 addrspace(1) void @N$PC(i8 zeroext %461)
  %462 = lshr i8 %458, 1
  call cc 1005 addrspace(1) void @N$PU1(i8 zeroext %462)
  %463 = call cc 1005 addrspace(1) ptr @N$PEND()
  %464 = call cc 1005 addrspace(1) ptr @N$TCAT(ptr %444, ptr %463)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %444)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %463)
  %465 = add nuw i16 %443, 1
  %466 = add i16 %442, -1
  %467 = getelementptr i8, ptr %441, i16 3
  %exitcond1 = icmp eq i16 %466, 0
  br i1 %exitcond1, label %449, label %440

468:
  call cc 1005 addrspace(1) void @N$EBND()
  unreachable

469:
  %470 = ptrtoint ptr %.ph9 to i16
  %471 = ptrtoint ptr %.ph8 to i16
  %472 = inttoptr i16 %470 to ptr
  %473 = inttoptr i16 %471 to ptr
  call cc 1005 addrspace(1) void @N$BDRP(ptr nonnull %335)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %472)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %473)
  call cc 1005 addrspace(1) void @N$BDRP(ptr %.ph7)
  br label %common.ret

common.ret:
  %common.op = phi i16 [ 1, %b7 ], [ 1, %b10 ], [ 1, %b8 ], [ 0, %469 ]
  ret i16 %common.op

b3:
  %.ph6 = phi i16 [ %extract.trunc1, %136 ], [ %.ph4, %316 ]
  %.ph11 = phi ptr [ null, %136 ], [ %.ph10, %316 ]
  %474 = ptrtoint ptr %.ph11 to i16
  %extract.trunc = trunc i16 %474 to i8
  %475 = icmp eq i8 %extract.trunc, 1
  br i1 %475, label %b10, label %476

476:
  %477 = icmp eq i8 %extract.trunc, 0
  br i1 %477, label %b7, label %b8

b7:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str13, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %common.ret

b8:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str15, i16 6))
  call cc 1005 addrspace(1) void @N$PU2(i16 %.ph6)
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str16, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %common.ret

b10:
  call cc 1005 addrspace(1) void @N$PS(ptr getelementptr inbounds nuw (i8, ptr @$str14, i16 6))
  call cc 1005 addrspace(1) void @N$PN()
  br label %common.ret
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

declare cc 1005 void @N$PU2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PS(ptr) addrspace(1)

declare cc 1005 void @N$PN() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PI2(i16) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PBEG() addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PC(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 void @N$PU1(i8) addrspace(1) memory(inaccessiblemem: readwrite)

declare cc 1005 ptr @N$PEND() addrspace(1)

declare cc 1005 ptr @N$TCAT(ptr, ptr) addrspace(1)

declare void @llvm.memset.p1.i16(ptr addrspace(1) nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memset.p0.i16(ptr nocapture writeonly, i8, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i16 @llvm.umax.i16(i16, i16) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) noalias nocapture writeonly, ptr addrspace(1) noalias nocapture readonly, i16, i1 immarg) nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)

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
!22 = !{i8 0, i8 3}
!23 = !{i8 0, i8 2}
!24 = !{i8 0, i8 2}
!25 = !{i8 0, i8 2}
!26 = !{i8 0, i8 2}
!27 = !{i8 0, i8 2}
!28 = !{i8 0, i8 2}
