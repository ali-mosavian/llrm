; ModuleID = 'fixed_point.c'
source_filename = "fixed_point.c"
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
target triple = "msp430"

%union.anon = type { i8 }
%union.anon.0 = type { i16 }
%union.anon.1 = type { i32 }
%union.anon.2 = type { i8 }
%union.anon.3 = type { i16 }
%union.anon.4 = type { i32 }
%union.anon.5 = type { i16 }
%union.anon.6 = type { i32 }
%union.anon.7 = type { i64 }
%union.anon.8 = type { i16 }
%union.anon.9 = type { i32 }
%union.anon.10 = type { i64 }
%union.anon.11 = type { i8 }
%union.anon.12 = type { i16 }
%union.anon.13 = type { i32 }
%union.anon.14 = type { i16 }
%union.anon.15 = type { i16 }
%union.anon.16 = type { i64 }
%union.anon.17 = type { i64 }

@sf = dso_local global [3 x i8] c"@\A0~", align 1
@f = dso_local global [3 x i16] [i16 16384, i16 -24576, i16 32440], align 2
@lf = dso_local global [3 x i32] [i32 1073741824, i32 -1610612736, i32 2126008811], align 4
@usf = dso_local global [3 x i8] c"\80\C0\FD", align 1
@uf = dso_local global [3 x i16] [i16 -32768, i16 -16384, i16 -656], align 2
@ulf = dso_local global [3 x i32] [i32 -2147483648, i32 -1073741824, i32 -42949673], align 4
@sa = dso_local global [3 x i16] [i16 192, i16 -288, i16 12800], align 2
@a = dso_local global [3 x i32] [i32 49152, i32 -73728, i32 3276800], align 4
@la = dso_local global [3 x i64] [i64 3221225472, i64 -4831838208, i64 64424509440000], align 8
@usa = dso_local global [3 x i16] [i16 384, i16 576, i16 25600], align 2
@ua = dso_local global [3 x i32] [i32 98304, i32 147456, i32 6553600], align 4
@ula = dso_local global [3 x i64] [i64 6442450944, i64 9663676416, i64 128849018880000], align 8

; Function Attrs: nounwind
define dso_local i16 @main() #0 {
  %1 = alloca i16, align 2
  %2 = alloca ptr, align 2
  %3 = alloca i16, align 2
  %4 = alloca i16, align 2
  %5 = alloca i8, align 1
  %6 = alloca %union.anon, align 1
  %7 = alloca ptr, align 2
  %8 = alloca i16, align 2
  %9 = alloca i16, align 2
  %10 = alloca i16, align 2
  %11 = alloca %union.anon.0, align 1
  %12 = alloca ptr, align 2
  %13 = alloca i16, align 2
  %14 = alloca i16, align 2
  %15 = alloca i32, align 4
  %16 = alloca %union.anon.1, align 1
  %17 = alloca ptr, align 2
  %18 = alloca i16, align 2
  %19 = alloca i16, align 2
  %20 = alloca i8, align 1
  %21 = alloca %union.anon.2, align 1
  %22 = alloca ptr, align 2
  %23 = alloca i16, align 2
  %24 = alloca i16, align 2
  %25 = alloca i16, align 2
  %26 = alloca %union.anon.3, align 1
  %27 = alloca ptr, align 2
  %28 = alloca i16, align 2
  %29 = alloca i16, align 2
  %30 = alloca i32, align 4
  %31 = alloca %union.anon.4, align 1
  %32 = alloca ptr, align 2
  %33 = alloca i16, align 2
  %34 = alloca i16, align 2
  %35 = alloca i16, align 2
  %36 = alloca %union.anon.5, align 1
  %37 = alloca ptr, align 2
  %38 = alloca i16, align 2
  %39 = alloca i16, align 2
  %40 = alloca i32, align 4
  %41 = alloca %union.anon.6, align 1
  %42 = alloca ptr, align 2
  %43 = alloca i16, align 2
  %44 = alloca i16, align 2
  %45 = alloca i64, align 8
  %46 = alloca %union.anon.7, align 1
  %47 = alloca ptr, align 2
  %48 = alloca i16, align 2
  %49 = alloca i16, align 2
  %50 = alloca i16, align 2
  %51 = alloca %union.anon.8, align 1
  %52 = alloca ptr, align 2
  %53 = alloca i16, align 2
  %54 = alloca i16, align 2
  %55 = alloca i32, align 4
  %56 = alloca %union.anon.9, align 1
  %57 = alloca ptr, align 2
  %58 = alloca i16, align 2
  %59 = alloca i16, align 2
  %60 = alloca i64, align 8
  %61 = alloca %union.anon.10, align 1
  %62 = alloca ptr, align 2
  %63 = alloca i16, align 2
  %64 = alloca i16, align 2
  %65 = alloca i8, align 1
  %66 = alloca %union.anon.11, align 1
  %67 = alloca ptr, align 2
  %68 = alloca i16, align 2
  %69 = alloca i16, align 2
  %70 = alloca i16, align 2
  %71 = alloca %union.anon.12, align 1
  %72 = alloca ptr, align 2
  %73 = alloca i16, align 2
  %74 = alloca i16, align 2
  %75 = alloca i32, align 4
  %76 = alloca %union.anon.13, align 1
  %77 = alloca ptr, align 2
  %78 = alloca i16, align 2
  %79 = alloca i16, align 2
  %80 = alloca i16, align 2
  %81 = alloca %union.anon.14, align 1
  %82 = alloca ptr, align 2
  %83 = alloca i16, align 2
  %84 = alloca i16, align 2
  %85 = alloca i16, align 2
  %86 = alloca %union.anon.15, align 1
  %87 = alloca ptr, align 2
  %88 = alloca i16, align 2
  %89 = alloca i16, align 2
  %90 = alloca i64, align 8
  %91 = alloca %union.anon.16, align 1
  %92 = alloca ptr, align 2
  %93 = alloca i16, align 2
  %94 = alloca i16, align 2
  %95 = alloca i64, align 8
  %96 = alloca %union.anon.17, align 1
  store i16 0, ptr %1, align 2
  br label %97

97:                                               ; preds = %0
  call void @llvm.lifetime.start.p0(ptr %2) #4
  store ptr @sf, ptr %2, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %3) #4
  call void @llvm.lifetime.start.p0(ptr %4) #4
  store i16 0, ptr %3, align 2, !tbaa !10
  br label %98

98:                                               ; preds = %122, %97
  %99 = load i16, ptr %3, align 2, !tbaa !10
  %100 = icmp slt i16 %99, 3
  br i1 %100, label %101, label %125

101:                                              ; preds = %98
  store i16 0, ptr %4, align 2, !tbaa !10
  br label %102

102:                                              ; preds = %118, %101
  %103 = load i16, ptr %4, align 2, !tbaa !10
  %104 = icmp slt i16 %103, 3
  br i1 %104, label %105, label %121

105:                                              ; preds = %102
  call void @llvm.lifetime.start.p0(ptr %5) #4
  %106 = load ptr, ptr %2, align 2, !tbaa !7
  %107 = load i16, ptr %3, align 2, !tbaa !10
  %108 = getelementptr inbounds i8, ptr %106, i16 %107
  %109 = load volatile i8, ptr %108, align 1, !tbaa !11
  %110 = load ptr, ptr %2, align 2, !tbaa !7
  %111 = load i16, ptr %4, align 2, !tbaa !10
  %112 = getelementptr inbounds i8, ptr %110, i16 %111
  %113 = load volatile i8, ptr %112, align 1, !tbaa !11
  %114 = call i8 @llvm.smul.fix.sat.i8(i8 %109, i8 %113, i32 7)
  store i8 %114, ptr %5, align 1, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %6) #4
  %115 = load i8, ptr %5, align 1, !tbaa !11
  store i8 %115, ptr %6, align 1, !tbaa !13
  %116 = load i8, ptr %6, align 1, !tbaa !13
  %117 = zext i8 %116 to i32
  call void @report(i32 noundef %117)
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  br label %118

118:                                              ; preds = %105
  %119 = load i16, ptr %4, align 2, !tbaa !10
  %120 = add nsw i16 %119, 1
  store i16 %120, ptr %4, align 2, !tbaa !10
  br label %102, !llvm.loop !14

121:                                              ; preds = %102
  br label %122

122:                                              ; preds = %121
  %123 = load i16, ptr %3, align 2, !tbaa !10
  %124 = add nsw i16 %123, 1
  store i16 %124, ptr %3, align 2, !tbaa !10
  br label %98, !llvm.loop !16

125:                                              ; preds = %98
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  br label %126

126:                                              ; preds = %125
  br label %127

127:                                              ; preds = %126
  call void @llvm.lifetime.start.p0(ptr %7) #4
  store ptr @f, ptr %7, align 2, !tbaa !17
  call void @llvm.lifetime.start.p0(ptr %8) #4
  call void @llvm.lifetime.start.p0(ptr %9) #4
  store i16 0, ptr %8, align 2, !tbaa !10
  br label %128

128:                                              ; preds = %152, %127
  %129 = load i16, ptr %8, align 2, !tbaa !10
  %130 = icmp slt i16 %129, 3
  br i1 %130, label %131, label %155

131:                                              ; preds = %128
  store i16 0, ptr %9, align 2, !tbaa !10
  br label %132

132:                                              ; preds = %148, %131
  %133 = load i16, ptr %9, align 2, !tbaa !10
  %134 = icmp slt i16 %133, 3
  br i1 %134, label %135, label %151

135:                                              ; preds = %132
  call void @llvm.lifetime.start.p0(ptr %10) #4
  %136 = load ptr, ptr %7, align 2, !tbaa !17
  %137 = load i16, ptr %8, align 2, !tbaa !10
  %138 = getelementptr inbounds i16, ptr %136, i16 %137
  %139 = load volatile i16, ptr %138, align 2, !tbaa !19
  %140 = load ptr, ptr %7, align 2, !tbaa !17
  %141 = load i16, ptr %9, align 2, !tbaa !10
  %142 = getelementptr inbounds i16, ptr %140, i16 %141
  %143 = load volatile i16, ptr %142, align 2, !tbaa !19
  %144 = call i16 @llvm.smul.fix.sat.i16(i16 %139, i16 %143, i32 15)
  store i16 %144, ptr %10, align 2, !tbaa !19
  call void @llvm.lifetime.start.p0(ptr %11) #4
  %145 = load i16, ptr %10, align 2, !tbaa !19
  store i16 %145, ptr %11, align 1, !tbaa !13
  %146 = load i16, ptr %11, align 1, !tbaa !13
  %147 = zext i16 %146 to i32
  call void @report(i32 noundef %147)
  call void @llvm.lifetime.end.p0(ptr %11) #4
  call void @llvm.lifetime.end.p0(ptr %10) #4
  br label %148

148:                                              ; preds = %135
  %149 = load i16, ptr %9, align 2, !tbaa !10
  %150 = add nsw i16 %149, 1
  store i16 %150, ptr %9, align 2, !tbaa !10
  br label %132, !llvm.loop !21

151:                                              ; preds = %132
  br label %152

152:                                              ; preds = %151
  %153 = load i16, ptr %8, align 2, !tbaa !10
  %154 = add nsw i16 %153, 1
  store i16 %154, ptr %8, align 2, !tbaa !10
  br label %128, !llvm.loop !22

155:                                              ; preds = %128
  call void @llvm.lifetime.end.p0(ptr %9) #4
  call void @llvm.lifetime.end.p0(ptr %8) #4
  call void @llvm.lifetime.end.p0(ptr %7) #4
  br label %156

156:                                              ; preds = %155
  br label %157

157:                                              ; preds = %156
  call void @llvm.lifetime.start.p0(ptr %12) #4
  store ptr @lf, ptr %12, align 2, !tbaa !23
  call void @llvm.lifetime.start.p0(ptr %13) #4
  call void @llvm.lifetime.start.p0(ptr %14) #4
  store i16 0, ptr %13, align 2, !tbaa !10
  br label %158

158:                                              ; preds = %181, %157
  %159 = load i16, ptr %13, align 2, !tbaa !10
  %160 = icmp slt i16 %159, 3
  br i1 %160, label %161, label %184

161:                                              ; preds = %158
  store i16 0, ptr %14, align 2, !tbaa !10
  br label %162

162:                                              ; preds = %177, %161
  %163 = load i16, ptr %14, align 2, !tbaa !10
  %164 = icmp slt i16 %163, 3
  br i1 %164, label %165, label %180

165:                                              ; preds = %162
  call void @llvm.lifetime.start.p0(ptr %15) #4
  %166 = load ptr, ptr %12, align 2, !tbaa !23
  %167 = load i16, ptr %13, align 2, !tbaa !10
  %168 = getelementptr inbounds i32, ptr %166, i16 %167
  %169 = load volatile i32, ptr %168, align 4, !tbaa !25
  %170 = load ptr, ptr %12, align 2, !tbaa !23
  %171 = load i16, ptr %14, align 2, !tbaa !10
  %172 = getelementptr inbounds i32, ptr %170, i16 %171
  %173 = load volatile i32, ptr %172, align 4, !tbaa !25
  %174 = call i32 @llvm.smul.fix.sat.i32(i32 %169, i32 %173, i32 31)
  store i32 %174, ptr %15, align 4, !tbaa !25
  call void @llvm.lifetime.start.p0(ptr %16) #4
  %175 = load i32, ptr %15, align 4, !tbaa !25
  store i32 %175, ptr %16, align 1, !tbaa !13
  %176 = load i32, ptr %16, align 1, !tbaa !13
  call void @report(i32 noundef %176)
  call void @llvm.lifetime.end.p0(ptr %16) #4
  call void @llvm.lifetime.end.p0(ptr %15) #4
  br label %177

177:                                              ; preds = %165
  %178 = load i16, ptr %14, align 2, !tbaa !10
  %179 = add nsw i16 %178, 1
  store i16 %179, ptr %14, align 2, !tbaa !10
  br label %162, !llvm.loop !27

180:                                              ; preds = %162
  br label %181

181:                                              ; preds = %180
  %182 = load i16, ptr %13, align 2, !tbaa !10
  %183 = add nsw i16 %182, 1
  store i16 %183, ptr %13, align 2, !tbaa !10
  br label %158, !llvm.loop !28

184:                                              ; preds = %158
  call void @llvm.lifetime.end.p0(ptr %14) #4
  call void @llvm.lifetime.end.p0(ptr %13) #4
  call void @llvm.lifetime.end.p0(ptr %12) #4
  br label %185

185:                                              ; preds = %184
  br label %186

186:                                              ; preds = %185
  call void @llvm.lifetime.start.p0(ptr %17) #4
  store ptr @usf, ptr %17, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %18) #4
  call void @llvm.lifetime.start.p0(ptr %19) #4
  store i16 0, ptr %18, align 2, !tbaa !10
  br label %187

187:                                              ; preds = %211, %186
  %188 = load i16, ptr %18, align 2, !tbaa !10
  %189 = icmp slt i16 %188, 3
  br i1 %189, label %190, label %214

190:                                              ; preds = %187
  store i16 0, ptr %19, align 2, !tbaa !10
  br label %191

191:                                              ; preds = %207, %190
  %192 = load i16, ptr %19, align 2, !tbaa !10
  %193 = icmp slt i16 %192, 3
  br i1 %193, label %194, label %210

194:                                              ; preds = %191
  call void @llvm.lifetime.start.p0(ptr %20) #4
  %195 = load ptr, ptr %17, align 2, !tbaa !7
  %196 = load i16, ptr %18, align 2, !tbaa !10
  %197 = getelementptr inbounds i8, ptr %195, i16 %196
  %198 = load volatile i8, ptr %197, align 1, !tbaa !11
  %199 = load ptr, ptr %17, align 2, !tbaa !7
  %200 = load i16, ptr %19, align 2, !tbaa !10
  %201 = getelementptr inbounds i8, ptr %199, i16 %200
  %202 = load volatile i8, ptr %201, align 1, !tbaa !11
  %203 = call i8 @llvm.umul.fix.sat.i8(i8 %198, i8 %202, i32 8)
  store i8 %203, ptr %20, align 1, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %21) #4
  %204 = load i8, ptr %20, align 1, !tbaa !11
  store i8 %204, ptr %21, align 1, !tbaa !13
  %205 = load i8, ptr %21, align 1, !tbaa !13
  %206 = zext i8 %205 to i32
  call void @report(i32 noundef %206)
  call void @llvm.lifetime.end.p0(ptr %21) #4
  call void @llvm.lifetime.end.p0(ptr %20) #4
  br label %207

207:                                              ; preds = %194
  %208 = load i16, ptr %19, align 2, !tbaa !10
  %209 = add nsw i16 %208, 1
  store i16 %209, ptr %19, align 2, !tbaa !10
  br label %191, !llvm.loop !29

210:                                              ; preds = %191
  br label %211

211:                                              ; preds = %210
  %212 = load i16, ptr %18, align 2, !tbaa !10
  %213 = add nsw i16 %212, 1
  store i16 %213, ptr %18, align 2, !tbaa !10
  br label %187, !llvm.loop !30

214:                                              ; preds = %187
  call void @llvm.lifetime.end.p0(ptr %19) #4
  call void @llvm.lifetime.end.p0(ptr %18) #4
  call void @llvm.lifetime.end.p0(ptr %17) #4
  br label %215

215:                                              ; preds = %214
  br label %216

216:                                              ; preds = %215
  call void @llvm.lifetime.start.p0(ptr %22) #4
  store ptr @uf, ptr %22, align 2, !tbaa !17
  call void @llvm.lifetime.start.p0(ptr %23) #4
  call void @llvm.lifetime.start.p0(ptr %24) #4
  store i16 0, ptr %23, align 2, !tbaa !10
  br label %217

217:                                              ; preds = %241, %216
  %218 = load i16, ptr %23, align 2, !tbaa !10
  %219 = icmp slt i16 %218, 3
  br i1 %219, label %220, label %244

220:                                              ; preds = %217
  store i16 0, ptr %24, align 2, !tbaa !10
  br label %221

221:                                              ; preds = %237, %220
  %222 = load i16, ptr %24, align 2, !tbaa !10
  %223 = icmp slt i16 %222, 3
  br i1 %223, label %224, label %240

224:                                              ; preds = %221
  call void @llvm.lifetime.start.p0(ptr %25) #4
  %225 = load ptr, ptr %22, align 2, !tbaa !17
  %226 = load i16, ptr %23, align 2, !tbaa !10
  %227 = getelementptr inbounds i16, ptr %225, i16 %226
  %228 = load volatile i16, ptr %227, align 2, !tbaa !19
  %229 = load ptr, ptr %22, align 2, !tbaa !17
  %230 = load i16, ptr %24, align 2, !tbaa !10
  %231 = getelementptr inbounds i16, ptr %229, i16 %230
  %232 = load volatile i16, ptr %231, align 2, !tbaa !19
  %233 = call i16 @llvm.umul.fix.sat.i16(i16 %228, i16 %232, i32 16)
  store i16 %233, ptr %25, align 2, !tbaa !19
  call void @llvm.lifetime.start.p0(ptr %26) #4
  %234 = load i16, ptr %25, align 2, !tbaa !19
  store i16 %234, ptr %26, align 1, !tbaa !13
  %235 = load i16, ptr %26, align 1, !tbaa !13
  %236 = zext i16 %235 to i32
  call void @report(i32 noundef %236)
  call void @llvm.lifetime.end.p0(ptr %26) #4
  call void @llvm.lifetime.end.p0(ptr %25) #4
  br label %237

237:                                              ; preds = %224
  %238 = load i16, ptr %24, align 2, !tbaa !10
  %239 = add nsw i16 %238, 1
  store i16 %239, ptr %24, align 2, !tbaa !10
  br label %221, !llvm.loop !31

240:                                              ; preds = %221
  br label %241

241:                                              ; preds = %240
  %242 = load i16, ptr %23, align 2, !tbaa !10
  %243 = add nsw i16 %242, 1
  store i16 %243, ptr %23, align 2, !tbaa !10
  br label %217, !llvm.loop !32

244:                                              ; preds = %217
  call void @llvm.lifetime.end.p0(ptr %24) #4
  call void @llvm.lifetime.end.p0(ptr %23) #4
  call void @llvm.lifetime.end.p0(ptr %22) #4
  br label %245

245:                                              ; preds = %244
  br label %246

246:                                              ; preds = %245
  call void @llvm.lifetime.start.p0(ptr %27) #4
  store ptr @ulf, ptr %27, align 2, !tbaa !23
  call void @llvm.lifetime.start.p0(ptr %28) #4
  call void @llvm.lifetime.start.p0(ptr %29) #4
  store i16 0, ptr %28, align 2, !tbaa !10
  br label %247

247:                                              ; preds = %270, %246
  %248 = load i16, ptr %28, align 2, !tbaa !10
  %249 = icmp slt i16 %248, 3
  br i1 %249, label %250, label %273

250:                                              ; preds = %247
  store i16 0, ptr %29, align 2, !tbaa !10
  br label %251

251:                                              ; preds = %266, %250
  %252 = load i16, ptr %29, align 2, !tbaa !10
  %253 = icmp slt i16 %252, 3
  br i1 %253, label %254, label %269

254:                                              ; preds = %251
  call void @llvm.lifetime.start.p0(ptr %30) #4
  %255 = load ptr, ptr %27, align 2, !tbaa !23
  %256 = load i16, ptr %28, align 2, !tbaa !10
  %257 = getelementptr inbounds i32, ptr %255, i16 %256
  %258 = load volatile i32, ptr %257, align 4, !tbaa !25
  %259 = load ptr, ptr %27, align 2, !tbaa !23
  %260 = load i16, ptr %29, align 2, !tbaa !10
  %261 = getelementptr inbounds i32, ptr %259, i16 %260
  %262 = load volatile i32, ptr %261, align 4, !tbaa !25
  %263 = call i32 @llvm.umul.fix.sat.i32(i32 %258, i32 %262, i32 32)
  store i32 %263, ptr %30, align 4, !tbaa !25
  call void @llvm.lifetime.start.p0(ptr %31) #4
  %264 = load i32, ptr %30, align 4, !tbaa !25
  store i32 %264, ptr %31, align 1, !tbaa !13
  %265 = load i32, ptr %31, align 1, !tbaa !13
  call void @report(i32 noundef %265)
  call void @llvm.lifetime.end.p0(ptr %31) #4
  call void @llvm.lifetime.end.p0(ptr %30) #4
  br label %266

266:                                              ; preds = %254
  %267 = load i16, ptr %29, align 2, !tbaa !10
  %268 = add nsw i16 %267, 1
  store i16 %268, ptr %29, align 2, !tbaa !10
  br label %251, !llvm.loop !33

269:                                              ; preds = %251
  br label %270

270:                                              ; preds = %269
  %271 = load i16, ptr %28, align 2, !tbaa !10
  %272 = add nsw i16 %271, 1
  store i16 %272, ptr %28, align 2, !tbaa !10
  br label %247, !llvm.loop !34

273:                                              ; preds = %247
  call void @llvm.lifetime.end.p0(ptr %29) #4
  call void @llvm.lifetime.end.p0(ptr %28) #4
  call void @llvm.lifetime.end.p0(ptr %27) #4
  br label %274

274:                                              ; preds = %273
  br label %275

275:                                              ; preds = %274
  call void @llvm.lifetime.start.p0(ptr %32) #4
  store ptr @sa, ptr %32, align 2, !tbaa !35
  call void @llvm.lifetime.start.p0(ptr %33) #4
  call void @llvm.lifetime.start.p0(ptr %34) #4
  store i16 0, ptr %33, align 2, !tbaa !10
  br label %276

276:                                              ; preds = %300, %275
  %277 = load i16, ptr %33, align 2, !tbaa !10
  %278 = icmp slt i16 %277, 3
  br i1 %278, label %279, label %303

279:                                              ; preds = %276
  store i16 0, ptr %34, align 2, !tbaa !10
  br label %280

280:                                              ; preds = %296, %279
  %281 = load i16, ptr %34, align 2, !tbaa !10
  %282 = icmp slt i16 %281, 3
  br i1 %282, label %283, label %299

283:                                              ; preds = %280
  call void @llvm.lifetime.start.p0(ptr %35) #4
  %284 = load ptr, ptr %32, align 2, !tbaa !35
  %285 = load i16, ptr %33, align 2, !tbaa !10
  %286 = getelementptr inbounds i16, ptr %284, i16 %285
  %287 = load volatile i16, ptr %286, align 2, !tbaa !37
  %288 = load ptr, ptr %32, align 2, !tbaa !35
  %289 = load i16, ptr %34, align 2, !tbaa !10
  %290 = getelementptr inbounds i16, ptr %288, i16 %289
  %291 = load volatile i16, ptr %290, align 2, !tbaa !37
  %292 = call i16 @llvm.smul.fix.sat.i16(i16 %287, i16 %291, i32 7)
  store i16 %292, ptr %35, align 2, !tbaa !37
  call void @llvm.lifetime.start.p0(ptr %36) #4
  %293 = load i16, ptr %35, align 2, !tbaa !37
  store i16 %293, ptr %36, align 1, !tbaa !13
  %294 = load i16, ptr %36, align 1, !tbaa !13
  %295 = zext i16 %294 to i32
  call void @report(i32 noundef %295)
  call void @llvm.lifetime.end.p0(ptr %36) #4
  call void @llvm.lifetime.end.p0(ptr %35) #4
  br label %296

296:                                              ; preds = %283
  %297 = load i16, ptr %34, align 2, !tbaa !10
  %298 = add nsw i16 %297, 1
  store i16 %298, ptr %34, align 2, !tbaa !10
  br label %280, !llvm.loop !39

299:                                              ; preds = %280
  br label %300

300:                                              ; preds = %299
  %301 = load i16, ptr %33, align 2, !tbaa !10
  %302 = add nsw i16 %301, 1
  store i16 %302, ptr %33, align 2, !tbaa !10
  br label %276, !llvm.loop !40

303:                                              ; preds = %276
  call void @llvm.lifetime.end.p0(ptr %34) #4
  call void @llvm.lifetime.end.p0(ptr %33) #4
  call void @llvm.lifetime.end.p0(ptr %32) #4
  br label %304

304:                                              ; preds = %303
  br label %305

305:                                              ; preds = %304
  call void @llvm.lifetime.start.p0(ptr %37) #4
  store ptr @a, ptr %37, align 2, !tbaa !41
  call void @llvm.lifetime.start.p0(ptr %38) #4
  call void @llvm.lifetime.start.p0(ptr %39) #4
  store i16 0, ptr %38, align 2, !tbaa !10
  br label %306

306:                                              ; preds = %329, %305
  %307 = load i16, ptr %38, align 2, !tbaa !10
  %308 = icmp slt i16 %307, 3
  br i1 %308, label %309, label %332

309:                                              ; preds = %306
  store i16 0, ptr %39, align 2, !tbaa !10
  br label %310

310:                                              ; preds = %325, %309
  %311 = load i16, ptr %39, align 2, !tbaa !10
  %312 = icmp slt i16 %311, 3
  br i1 %312, label %313, label %328

313:                                              ; preds = %310
  call void @llvm.lifetime.start.p0(ptr %40) #4
  %314 = load ptr, ptr %37, align 2, !tbaa !41
  %315 = load i16, ptr %38, align 2, !tbaa !10
  %316 = getelementptr inbounds i32, ptr %314, i16 %315
  %317 = load volatile i32, ptr %316, align 4, !tbaa !43
  %318 = load ptr, ptr %37, align 2, !tbaa !41
  %319 = load i16, ptr %39, align 2, !tbaa !10
  %320 = getelementptr inbounds i32, ptr %318, i16 %319
  %321 = load volatile i32, ptr %320, align 4, !tbaa !43
  %322 = call i32 @llvm.smul.fix.sat.i32(i32 %317, i32 %321, i32 15)
  store i32 %322, ptr %40, align 4, !tbaa !43
  call void @llvm.lifetime.start.p0(ptr %41) #4
  %323 = load i32, ptr %40, align 4, !tbaa !43
  store i32 %323, ptr %41, align 1, !tbaa !13
  %324 = load i32, ptr %41, align 1, !tbaa !13
  call void @report(i32 noundef %324)
  call void @llvm.lifetime.end.p0(ptr %41) #4
  call void @llvm.lifetime.end.p0(ptr %40) #4
  br label %325

325:                                              ; preds = %313
  %326 = load i16, ptr %39, align 2, !tbaa !10
  %327 = add nsw i16 %326, 1
  store i16 %327, ptr %39, align 2, !tbaa !10
  br label %310, !llvm.loop !45

328:                                              ; preds = %310
  br label %329

329:                                              ; preds = %328
  %330 = load i16, ptr %38, align 2, !tbaa !10
  %331 = add nsw i16 %330, 1
  store i16 %331, ptr %38, align 2, !tbaa !10
  br label %306, !llvm.loop !46

332:                                              ; preds = %306
  call void @llvm.lifetime.end.p0(ptr %39) #4
  call void @llvm.lifetime.end.p0(ptr %38) #4
  call void @llvm.lifetime.end.p0(ptr %37) #4
  br label %333

333:                                              ; preds = %332
  br label %334

334:                                              ; preds = %333
  call void @llvm.lifetime.start.p0(ptr %42) #4
  store ptr @la, ptr %42, align 2, !tbaa !47
  call void @llvm.lifetime.start.p0(ptr %43) #4
  call void @llvm.lifetime.start.p0(ptr %44) #4
  store i16 0, ptr %43, align 2, !tbaa !10
  br label %335

335:                                              ; preds = %359, %334
  %336 = load i16, ptr %43, align 2, !tbaa !10
  %337 = icmp slt i16 %336, 3
  br i1 %337, label %338, label %362

338:                                              ; preds = %335
  store i16 0, ptr %44, align 2, !tbaa !10
  br label %339

339:                                              ; preds = %355, %338
  %340 = load i16, ptr %44, align 2, !tbaa !10
  %341 = icmp slt i16 %340, 3
  br i1 %341, label %342, label %358

342:                                              ; preds = %339
  call void @llvm.lifetime.start.p0(ptr %45) #4
  %343 = load ptr, ptr %42, align 2, !tbaa !47
  %344 = load i16, ptr %43, align 2, !tbaa !10
  %345 = getelementptr inbounds i64, ptr %343, i16 %344
  %346 = load volatile i64, ptr %345, align 8, !tbaa !49
  %347 = load ptr, ptr %42, align 2, !tbaa !47
  %348 = load i16, ptr %44, align 2, !tbaa !10
  %349 = getelementptr inbounds i64, ptr %347, i16 %348
  %350 = load volatile i64, ptr %349, align 8, !tbaa !49
  %351 = call i64 @llvm.smul.fix.sat.i64(i64 %346, i64 %350, i32 31)
  store i64 %351, ptr %45, align 8, !tbaa !49
  call void @llvm.lifetime.start.p0(ptr %46) #4
  %352 = load i64, ptr %45, align 8, !tbaa !49
  store i64 %352, ptr %46, align 1, !tbaa !13
  %353 = load i64, ptr %46, align 1, !tbaa !13
  %354 = trunc i64 %353 to i32
  call void @report(i32 noundef %354)
  call void @llvm.lifetime.end.p0(ptr %46) #4
  call void @llvm.lifetime.end.p0(ptr %45) #4
  br label %355

355:                                              ; preds = %342
  %356 = load i16, ptr %44, align 2, !tbaa !10
  %357 = add nsw i16 %356, 1
  store i16 %357, ptr %44, align 2, !tbaa !10
  br label %339, !llvm.loop !51

358:                                              ; preds = %339
  br label %359

359:                                              ; preds = %358
  %360 = load i16, ptr %43, align 2, !tbaa !10
  %361 = add nsw i16 %360, 1
  store i16 %361, ptr %43, align 2, !tbaa !10
  br label %335, !llvm.loop !52

362:                                              ; preds = %335
  call void @llvm.lifetime.end.p0(ptr %44) #4
  call void @llvm.lifetime.end.p0(ptr %43) #4
  call void @llvm.lifetime.end.p0(ptr %42) #4
  br label %363

363:                                              ; preds = %362
  br label %364

364:                                              ; preds = %363
  call void @llvm.lifetime.start.p0(ptr %47) #4
  store ptr @usa, ptr %47, align 2, !tbaa !35
  call void @llvm.lifetime.start.p0(ptr %48) #4
  call void @llvm.lifetime.start.p0(ptr %49) #4
  store i16 0, ptr %48, align 2, !tbaa !10
  br label %365

365:                                              ; preds = %389, %364
  %366 = load i16, ptr %48, align 2, !tbaa !10
  %367 = icmp slt i16 %366, 3
  br i1 %367, label %368, label %392

368:                                              ; preds = %365
  store i16 0, ptr %49, align 2, !tbaa !10
  br label %369

369:                                              ; preds = %385, %368
  %370 = load i16, ptr %49, align 2, !tbaa !10
  %371 = icmp slt i16 %370, 3
  br i1 %371, label %372, label %388

372:                                              ; preds = %369
  call void @llvm.lifetime.start.p0(ptr %50) #4
  %373 = load ptr, ptr %47, align 2, !tbaa !35
  %374 = load i16, ptr %48, align 2, !tbaa !10
  %375 = getelementptr inbounds i16, ptr %373, i16 %374
  %376 = load volatile i16, ptr %375, align 2, !tbaa !37
  %377 = load ptr, ptr %47, align 2, !tbaa !35
  %378 = load i16, ptr %49, align 2, !tbaa !10
  %379 = getelementptr inbounds i16, ptr %377, i16 %378
  %380 = load volatile i16, ptr %379, align 2, !tbaa !37
  %381 = call i16 @llvm.umul.fix.sat.i16(i16 %376, i16 %380, i32 8)
  store i16 %381, ptr %50, align 2, !tbaa !37
  call void @llvm.lifetime.start.p0(ptr %51) #4
  %382 = load i16, ptr %50, align 2, !tbaa !37
  store i16 %382, ptr %51, align 1, !tbaa !13
  %383 = load i16, ptr %51, align 1, !tbaa !13
  %384 = zext i16 %383 to i32
  call void @report(i32 noundef %384)
  call void @llvm.lifetime.end.p0(ptr %51) #4
  call void @llvm.lifetime.end.p0(ptr %50) #4
  br label %385

385:                                              ; preds = %372
  %386 = load i16, ptr %49, align 2, !tbaa !10
  %387 = add nsw i16 %386, 1
  store i16 %387, ptr %49, align 2, !tbaa !10
  br label %369, !llvm.loop !53

388:                                              ; preds = %369
  br label %389

389:                                              ; preds = %388
  %390 = load i16, ptr %48, align 2, !tbaa !10
  %391 = add nsw i16 %390, 1
  store i16 %391, ptr %48, align 2, !tbaa !10
  br label %365, !llvm.loop !54

392:                                              ; preds = %365
  call void @llvm.lifetime.end.p0(ptr %49) #4
  call void @llvm.lifetime.end.p0(ptr %48) #4
  call void @llvm.lifetime.end.p0(ptr %47) #4
  br label %393

393:                                              ; preds = %392
  br label %394

394:                                              ; preds = %393
  call void @llvm.lifetime.start.p0(ptr %52) #4
  store ptr @ua, ptr %52, align 2, !tbaa !41
  call void @llvm.lifetime.start.p0(ptr %53) #4
  call void @llvm.lifetime.start.p0(ptr %54) #4
  store i16 0, ptr %53, align 2, !tbaa !10
  br label %395

395:                                              ; preds = %418, %394
  %396 = load i16, ptr %53, align 2, !tbaa !10
  %397 = icmp slt i16 %396, 3
  br i1 %397, label %398, label %421

398:                                              ; preds = %395
  store i16 0, ptr %54, align 2, !tbaa !10
  br label %399

399:                                              ; preds = %414, %398
  %400 = load i16, ptr %54, align 2, !tbaa !10
  %401 = icmp slt i16 %400, 3
  br i1 %401, label %402, label %417

402:                                              ; preds = %399
  call void @llvm.lifetime.start.p0(ptr %55) #4
  %403 = load ptr, ptr %52, align 2, !tbaa !41
  %404 = load i16, ptr %53, align 2, !tbaa !10
  %405 = getelementptr inbounds i32, ptr %403, i16 %404
  %406 = load volatile i32, ptr %405, align 4, !tbaa !43
  %407 = load ptr, ptr %52, align 2, !tbaa !41
  %408 = load i16, ptr %54, align 2, !tbaa !10
  %409 = getelementptr inbounds i32, ptr %407, i16 %408
  %410 = load volatile i32, ptr %409, align 4, !tbaa !43
  %411 = call i32 @llvm.umul.fix.sat.i32(i32 %406, i32 %410, i32 16)
  store i32 %411, ptr %55, align 4, !tbaa !43
  call void @llvm.lifetime.start.p0(ptr %56) #4
  %412 = load i32, ptr %55, align 4, !tbaa !43
  store i32 %412, ptr %56, align 1, !tbaa !13
  %413 = load i32, ptr %56, align 1, !tbaa !13
  call void @report(i32 noundef %413)
  call void @llvm.lifetime.end.p0(ptr %56) #4
  call void @llvm.lifetime.end.p0(ptr %55) #4
  br label %414

414:                                              ; preds = %402
  %415 = load i16, ptr %54, align 2, !tbaa !10
  %416 = add nsw i16 %415, 1
  store i16 %416, ptr %54, align 2, !tbaa !10
  br label %399, !llvm.loop !55

417:                                              ; preds = %399
  br label %418

418:                                              ; preds = %417
  %419 = load i16, ptr %53, align 2, !tbaa !10
  %420 = add nsw i16 %419, 1
  store i16 %420, ptr %53, align 2, !tbaa !10
  br label %395, !llvm.loop !56

421:                                              ; preds = %395
  call void @llvm.lifetime.end.p0(ptr %54) #4
  call void @llvm.lifetime.end.p0(ptr %53) #4
  call void @llvm.lifetime.end.p0(ptr %52) #4
  br label %422

422:                                              ; preds = %421
  br label %423

423:                                              ; preds = %422
  call void @llvm.lifetime.start.p0(ptr %57) #4
  store ptr @ula, ptr %57, align 2, !tbaa !47
  call void @llvm.lifetime.start.p0(ptr %58) #4
  call void @llvm.lifetime.start.p0(ptr %59) #4
  store i16 0, ptr %58, align 2, !tbaa !10
  br label %424

424:                                              ; preds = %448, %423
  %425 = load i16, ptr %58, align 2, !tbaa !10
  %426 = icmp slt i16 %425, 3
  br i1 %426, label %427, label %451

427:                                              ; preds = %424
  store i16 0, ptr %59, align 2, !tbaa !10
  br label %428

428:                                              ; preds = %444, %427
  %429 = load i16, ptr %59, align 2, !tbaa !10
  %430 = icmp slt i16 %429, 3
  br i1 %430, label %431, label %447

431:                                              ; preds = %428
  call void @llvm.lifetime.start.p0(ptr %60) #4
  %432 = load ptr, ptr %57, align 2, !tbaa !47
  %433 = load i16, ptr %58, align 2, !tbaa !10
  %434 = getelementptr inbounds i64, ptr %432, i16 %433
  %435 = load volatile i64, ptr %434, align 8, !tbaa !49
  %436 = load ptr, ptr %57, align 2, !tbaa !47
  %437 = load i16, ptr %59, align 2, !tbaa !10
  %438 = getelementptr inbounds i64, ptr %436, i16 %437
  %439 = load volatile i64, ptr %438, align 8, !tbaa !49
  %440 = call i64 @llvm.umul.fix.sat.i64(i64 %435, i64 %439, i32 32)
  store i64 %440, ptr %60, align 8, !tbaa !49
  call void @llvm.lifetime.start.p0(ptr %61) #4
  %441 = load i64, ptr %60, align 8, !tbaa !49
  store i64 %441, ptr %61, align 1, !tbaa !13
  %442 = load i64, ptr %61, align 1, !tbaa !13
  %443 = trunc i64 %442 to i32
  call void @report(i32 noundef %443)
  call void @llvm.lifetime.end.p0(ptr %61) #4
  call void @llvm.lifetime.end.p0(ptr %60) #4
  br label %444

444:                                              ; preds = %431
  %445 = load i16, ptr %59, align 2, !tbaa !10
  %446 = add nsw i16 %445, 1
  store i16 %446, ptr %59, align 2, !tbaa !10
  br label %428, !llvm.loop !57

447:                                              ; preds = %428
  br label %448

448:                                              ; preds = %447
  %449 = load i16, ptr %58, align 2, !tbaa !10
  %450 = add nsw i16 %449, 1
  store i16 %450, ptr %58, align 2, !tbaa !10
  br label %424, !llvm.loop !58

451:                                              ; preds = %424
  call void @llvm.lifetime.end.p0(ptr %59) #4
  call void @llvm.lifetime.end.p0(ptr %58) #4
  call void @llvm.lifetime.end.p0(ptr %57) #4
  br label %452

452:                                              ; preds = %451
  br label %453

453:                                              ; preds = %452
  call void @llvm.lifetime.start.p0(ptr %62) #4
  store ptr @sf, ptr %62, align 2, !tbaa !7
  call void @llvm.lifetime.start.p0(ptr %63) #4
  store volatile i16 3, ptr %63, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %64) #4
  store i16 0, ptr %64, align 2, !tbaa !10
  br label %454

454:                                              ; preds = %468, %453
  %455 = load i16, ptr %64, align 2, !tbaa !10
  %456 = icmp slt i16 %455, 3
  br i1 %456, label %457, label %471

457:                                              ; preds = %454
  call void @llvm.lifetime.start.p0(ptr %65) #4
  %458 = load ptr, ptr %62, align 2, !tbaa !7
  %459 = load i16, ptr %64, align 2, !tbaa !10
  %460 = getelementptr inbounds i8, ptr %458, i16 %459
  %461 = load volatile i8, ptr %460, align 1, !tbaa !11
  %462 = load volatile i16, ptr %63, align 2, !tbaa !10
  %463 = trunc i16 %462 to i8
  %464 = call i8 @llvm.sshl.sat.i8(i8 %461, i8 %463)
  store i8 %464, ptr %65, align 1, !tbaa !11
  call void @llvm.lifetime.start.p0(ptr %66) #4
  %465 = load i8, ptr %65, align 1, !tbaa !11
  store i8 %465, ptr %66, align 1, !tbaa !13
  %466 = load i8, ptr %66, align 1, !tbaa !13
  %467 = zext i8 %466 to i32
  call void @report(i32 noundef %467)
  call void @llvm.lifetime.end.p0(ptr %66) #4
  call void @llvm.lifetime.end.p0(ptr %65) #4
  br label %468

468:                                              ; preds = %457
  %469 = load i16, ptr %64, align 2, !tbaa !10
  %470 = add nsw i16 %469, 1
  store i16 %470, ptr %64, align 2, !tbaa !10
  br label %454, !llvm.loop !59

471:                                              ; preds = %454
  call void @llvm.lifetime.end.p0(ptr %64) #4
  call void @llvm.lifetime.end.p0(ptr %63) #4
  call void @llvm.lifetime.end.p0(ptr %62) #4
  br label %472

472:                                              ; preds = %471
  br label %473

473:                                              ; preds = %472
  call void @llvm.lifetime.start.p0(ptr %67) #4
  store ptr @f, ptr %67, align 2, !tbaa !17
  call void @llvm.lifetime.start.p0(ptr %68) #4
  store volatile i16 3, ptr %68, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %69) #4
  store i16 0, ptr %69, align 2, !tbaa !10
  br label %474

474:                                              ; preds = %487, %473
  %475 = load i16, ptr %69, align 2, !tbaa !10
  %476 = icmp slt i16 %475, 3
  br i1 %476, label %477, label %490

477:                                              ; preds = %474
  call void @llvm.lifetime.start.p0(ptr %70) #4
  %478 = load ptr, ptr %67, align 2, !tbaa !17
  %479 = load i16, ptr %69, align 2, !tbaa !10
  %480 = getelementptr inbounds i16, ptr %478, i16 %479
  %481 = load volatile i16, ptr %480, align 2, !tbaa !19
  %482 = load volatile i16, ptr %68, align 2, !tbaa !10
  %483 = call i16 @llvm.sshl.sat.i16(i16 %481, i16 %482)
  store i16 %483, ptr %70, align 2, !tbaa !19
  call void @llvm.lifetime.start.p0(ptr %71) #4
  %484 = load i16, ptr %70, align 2, !tbaa !19
  store i16 %484, ptr %71, align 1, !tbaa !13
  %485 = load i16, ptr %71, align 1, !tbaa !13
  %486 = zext i16 %485 to i32
  call void @report(i32 noundef %486)
  call void @llvm.lifetime.end.p0(ptr %71) #4
  call void @llvm.lifetime.end.p0(ptr %70) #4
  br label %487

487:                                              ; preds = %477
  %488 = load i16, ptr %69, align 2, !tbaa !10
  %489 = add nsw i16 %488, 1
  store i16 %489, ptr %69, align 2, !tbaa !10
  br label %474, !llvm.loop !60

490:                                              ; preds = %474
  call void @llvm.lifetime.end.p0(ptr %69) #4
  call void @llvm.lifetime.end.p0(ptr %68) #4
  call void @llvm.lifetime.end.p0(ptr %67) #4
  br label %491

491:                                              ; preds = %490
  br label %492

492:                                              ; preds = %491
  call void @llvm.lifetime.start.p0(ptr %72) #4
  store ptr @lf, ptr %72, align 2, !tbaa !23
  call void @llvm.lifetime.start.p0(ptr %73) #4
  store volatile i16 3, ptr %73, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %74) #4
  store i16 0, ptr %74, align 2, !tbaa !10
  br label %493

493:                                              ; preds = %506, %492
  %494 = load i16, ptr %74, align 2, !tbaa !10
  %495 = icmp slt i16 %494, 3
  br i1 %495, label %496, label %509

496:                                              ; preds = %493
  call void @llvm.lifetime.start.p0(ptr %75) #4
  %497 = load ptr, ptr %72, align 2, !tbaa !23
  %498 = load i16, ptr %74, align 2, !tbaa !10
  %499 = getelementptr inbounds i32, ptr %497, i16 %498
  %500 = load volatile i32, ptr %499, align 4, !tbaa !25
  %501 = load volatile i16, ptr %73, align 2, !tbaa !10
  %502 = zext i16 %501 to i32
  %503 = call i32 @llvm.sshl.sat.i32(i32 %500, i32 %502)
  store i32 %503, ptr %75, align 4, !tbaa !25
  call void @llvm.lifetime.start.p0(ptr %76) #4
  %504 = load i32, ptr %75, align 4, !tbaa !25
  store i32 %504, ptr %76, align 1, !tbaa !13
  %505 = load i32, ptr %76, align 1, !tbaa !13
  call void @report(i32 noundef %505)
  call void @llvm.lifetime.end.p0(ptr %76) #4
  call void @llvm.lifetime.end.p0(ptr %75) #4
  br label %506

506:                                              ; preds = %496
  %507 = load i16, ptr %74, align 2, !tbaa !10
  %508 = add nsw i16 %507, 1
  store i16 %508, ptr %74, align 2, !tbaa !10
  br label %493, !llvm.loop !61

509:                                              ; preds = %493
  call void @llvm.lifetime.end.p0(ptr %74) #4
  call void @llvm.lifetime.end.p0(ptr %73) #4
  call void @llvm.lifetime.end.p0(ptr %72) #4
  br label %510

510:                                              ; preds = %509
  br label %511

511:                                              ; preds = %510
  call void @llvm.lifetime.start.p0(ptr %77) #4
  store ptr @uf, ptr %77, align 2, !tbaa !17
  call void @llvm.lifetime.start.p0(ptr %78) #4
  store volatile i16 3, ptr %78, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %79) #4
  store i16 0, ptr %79, align 2, !tbaa !10
  br label %512

512:                                              ; preds = %525, %511
  %513 = load i16, ptr %79, align 2, !tbaa !10
  %514 = icmp slt i16 %513, 3
  br i1 %514, label %515, label %528

515:                                              ; preds = %512
  call void @llvm.lifetime.start.p0(ptr %80) #4
  %516 = load ptr, ptr %77, align 2, !tbaa !17
  %517 = load i16, ptr %79, align 2, !tbaa !10
  %518 = getelementptr inbounds i16, ptr %516, i16 %517
  %519 = load volatile i16, ptr %518, align 2, !tbaa !19
  %520 = load volatile i16, ptr %78, align 2, !tbaa !10
  %521 = call i16 @llvm.ushl.sat.i16(i16 %519, i16 %520)
  store i16 %521, ptr %80, align 2, !tbaa !19
  call void @llvm.lifetime.start.p0(ptr %81) #4
  %522 = load i16, ptr %80, align 2, !tbaa !19
  store i16 %522, ptr %81, align 1, !tbaa !13
  %523 = load i16, ptr %81, align 1, !tbaa !13
  %524 = zext i16 %523 to i32
  call void @report(i32 noundef %524)
  call void @llvm.lifetime.end.p0(ptr %81) #4
  call void @llvm.lifetime.end.p0(ptr %80) #4
  br label %525

525:                                              ; preds = %515
  %526 = load i16, ptr %79, align 2, !tbaa !10
  %527 = add nsw i16 %526, 1
  store i16 %527, ptr %79, align 2, !tbaa !10
  br label %512, !llvm.loop !62

528:                                              ; preds = %512
  call void @llvm.lifetime.end.p0(ptr %79) #4
  call void @llvm.lifetime.end.p0(ptr %78) #4
  call void @llvm.lifetime.end.p0(ptr %77) #4
  br label %529

529:                                              ; preds = %528
  br label %530

530:                                              ; preds = %529
  call void @llvm.lifetime.start.p0(ptr %82) #4
  store ptr @sa, ptr %82, align 2, !tbaa !35
  call void @llvm.lifetime.start.p0(ptr %83) #4
  store volatile i16 3, ptr %83, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %84) #4
  store i16 0, ptr %84, align 2, !tbaa !10
  br label %531

531:                                              ; preds = %544, %530
  %532 = load i16, ptr %84, align 2, !tbaa !10
  %533 = icmp slt i16 %532, 3
  br i1 %533, label %534, label %547

534:                                              ; preds = %531
  call void @llvm.lifetime.start.p0(ptr %85) #4
  %535 = load ptr, ptr %82, align 2, !tbaa !35
  %536 = load i16, ptr %84, align 2, !tbaa !10
  %537 = getelementptr inbounds i16, ptr %535, i16 %536
  %538 = load volatile i16, ptr %537, align 2, !tbaa !37
  %539 = load volatile i16, ptr %83, align 2, !tbaa !10
  %540 = call i16 @llvm.sshl.sat.i16(i16 %538, i16 %539)
  store i16 %540, ptr %85, align 2, !tbaa !37
  call void @llvm.lifetime.start.p0(ptr %86) #4
  %541 = load i16, ptr %85, align 2, !tbaa !37
  store i16 %541, ptr %86, align 1, !tbaa !13
  %542 = load i16, ptr %86, align 1, !tbaa !13
  %543 = zext i16 %542 to i32
  call void @report(i32 noundef %543)
  call void @llvm.lifetime.end.p0(ptr %86) #4
  call void @llvm.lifetime.end.p0(ptr %85) #4
  br label %544

544:                                              ; preds = %534
  %545 = load i16, ptr %84, align 2, !tbaa !10
  %546 = add nsw i16 %545, 1
  store i16 %546, ptr %84, align 2, !tbaa !10
  br label %531, !llvm.loop !63

547:                                              ; preds = %531
  call void @llvm.lifetime.end.p0(ptr %84) #4
  call void @llvm.lifetime.end.p0(ptr %83) #4
  call void @llvm.lifetime.end.p0(ptr %82) #4
  br label %548

548:                                              ; preds = %547
  br label %549

549:                                              ; preds = %548
  call void @llvm.lifetime.start.p0(ptr %87) #4
  store ptr @la, ptr %87, align 2, !tbaa !47
  call void @llvm.lifetime.start.p0(ptr %88) #4
  store volatile i16 3, ptr %88, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %89) #4
  store i16 0, ptr %89, align 2, !tbaa !10
  br label %550

550:                                              ; preds = %564, %549
  %551 = load i16, ptr %89, align 2, !tbaa !10
  %552 = icmp slt i16 %551, 3
  br i1 %552, label %553, label %567

553:                                              ; preds = %550
  call void @llvm.lifetime.start.p0(ptr %90) #4
  %554 = load ptr, ptr %87, align 2, !tbaa !47
  %555 = load i16, ptr %89, align 2, !tbaa !10
  %556 = getelementptr inbounds i64, ptr %554, i16 %555
  %557 = load volatile i64, ptr %556, align 8, !tbaa !49
  %558 = load volatile i16, ptr %88, align 2, !tbaa !10
  %559 = zext i16 %558 to i64
  %560 = call i64 @llvm.sshl.sat.i64(i64 %557, i64 %559)
  store i64 %560, ptr %90, align 8, !tbaa !49
  call void @llvm.lifetime.start.p0(ptr %91) #4
  %561 = load i64, ptr %90, align 8, !tbaa !49
  store i64 %561, ptr %91, align 1, !tbaa !13
  %562 = load i64, ptr %91, align 1, !tbaa !13
  %563 = trunc i64 %562 to i32
  call void @report(i32 noundef %563)
  call void @llvm.lifetime.end.p0(ptr %91) #4
  call void @llvm.lifetime.end.p0(ptr %90) #4
  br label %564

564:                                              ; preds = %553
  %565 = load i16, ptr %89, align 2, !tbaa !10
  %566 = add nsw i16 %565, 1
  store i16 %566, ptr %89, align 2, !tbaa !10
  br label %550, !llvm.loop !64

567:                                              ; preds = %550
  call void @llvm.lifetime.end.p0(ptr %89) #4
  call void @llvm.lifetime.end.p0(ptr %88) #4
  call void @llvm.lifetime.end.p0(ptr %87) #4
  br label %568

568:                                              ; preds = %567
  br label %569

569:                                              ; preds = %568
  call void @llvm.lifetime.start.p0(ptr %92) #4
  store ptr @ula, ptr %92, align 2, !tbaa !47
  call void @llvm.lifetime.start.p0(ptr %93) #4
  store volatile i16 3, ptr %93, align 2, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %94) #4
  store i16 0, ptr %94, align 2, !tbaa !10
  br label %570

570:                                              ; preds = %584, %569
  %571 = load i16, ptr %94, align 2, !tbaa !10
  %572 = icmp slt i16 %571, 3
  br i1 %572, label %573, label %587

573:                                              ; preds = %570
  call void @llvm.lifetime.start.p0(ptr %95) #4
  %574 = load ptr, ptr %92, align 2, !tbaa !47
  %575 = load i16, ptr %94, align 2, !tbaa !10
  %576 = getelementptr inbounds i64, ptr %574, i16 %575
  %577 = load volatile i64, ptr %576, align 8, !tbaa !49
  %578 = load volatile i16, ptr %93, align 2, !tbaa !10
  %579 = zext i16 %578 to i64
  %580 = call i64 @llvm.ushl.sat.i64(i64 %577, i64 %579)
  store i64 %580, ptr %95, align 8, !tbaa !49
  call void @llvm.lifetime.start.p0(ptr %96) #4
  %581 = load i64, ptr %95, align 8, !tbaa !49
  store i64 %581, ptr %96, align 1, !tbaa !13
  %582 = load i64, ptr %96, align 1, !tbaa !13
  %583 = trunc i64 %582 to i32
  call void @report(i32 noundef %583)
  call void @llvm.lifetime.end.p0(ptr %96) #4
  call void @llvm.lifetime.end.p0(ptr %95) #4
  br label %584

584:                                              ; preds = %573
  %585 = load i16, ptr %94, align 2, !tbaa !10
  %586 = add nsw i16 %585, 1
  store i16 %586, ptr %94, align 2, !tbaa !10
  br label %570, !llvm.loop !65

587:                                              ; preds = %570
  call void @llvm.lifetime.end.p0(ptr %94) #4
  call void @llvm.lifetime.end.p0(ptr %93) #4
  call void @llvm.lifetime.end.p0(ptr %92) #4
  br label %588

588:                                              ; preds = %587
  ret i16 0
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i8 @llvm.smul.fix.sat.i8(i8, i8, i32 immarg) #2

declare dso_local void @report(i32 noundef) #3

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.smul.fix.sat.i16(i16, i16, i32 immarg) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.smul.fix.sat.i32(i32, i32, i32 immarg) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i8 @llvm.umul.fix.sat.i8(i8, i8, i32 immarg) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.umul.fix.sat.i16(i16, i16, i32 immarg) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.umul.fix.sat.i32(i32, i32, i32 immarg) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.smul.fix.sat.i64(i64, i64, i32 immarg) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.umul.fix.sat.i64(i64, i64, i32 immarg) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i8 @llvm.sshl.sat.i8(i8, i8) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.sshl.sat.i16(i16, i16) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.sshl.sat.i32(i32, i32) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i16 @llvm.ushl.sat.i16(i16, i16) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.sshl.sat.i64(i64, i64) #2

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare i64 @llvm.ushl.sat.i64(i64, i64) #2

attributes #0 = { nounwind "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
attributes #3 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" }
attributes #4 = { nounwind }

!llvm.module.flags = !{!0}
!llvm.ident = !{!1}
!llvm.errno.tbaa = !{!2}

!0 = !{i32 1, !"wchar_size", i32 2}
!1 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!2 = !{!3, !4, i64 0}
!3 = !{!"__libc_errno", !4, i64 0}
!4 = !{!"int", !5, i64 0}
!5 = !{!"omnipotent char", !6, i64 0}
!6 = !{!"Simple C/C++ TBAA"}
!7 = !{!8, !8, i64 0}
!8 = !{!"p1 _Sat short _Fract", !9, i64 0}
!9 = !{!"any pointer", !5, i64 0}
!10 = !{!4, !4, i64 0}
!11 = !{!12, !12, i64 0}
!12 = !{!"_Sat short _Fract", !5, i64 0}
!13 = !{!5, !5, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
!16 = distinct !{!16, !15}
!17 = !{!18, !18, i64 0}
!18 = !{!"p1 _Sat _Fract", !9, i64 0}
!19 = !{!20, !20, i64 0}
!20 = !{!"_Sat _Fract", !5, i64 0}
!21 = distinct !{!21, !15}
!22 = distinct !{!22, !15}
!23 = !{!24, !24, i64 0}
!24 = !{!"p1 _Sat long _Fract", !9, i64 0}
!25 = !{!26, !26, i64 0}
!26 = !{!"_Sat long _Fract", !5, i64 0}
!27 = distinct !{!27, !15}
!28 = distinct !{!28, !15}
!29 = distinct !{!29, !15}
!30 = distinct !{!30, !15}
!31 = distinct !{!31, !15}
!32 = distinct !{!32, !15}
!33 = distinct !{!33, !15}
!34 = distinct !{!34, !15}
!35 = !{!36, !36, i64 0}
!36 = !{!"p1 _Sat short _Accum", !9, i64 0}
!37 = !{!38, !38, i64 0}
!38 = !{!"_Sat short _Accum", !5, i64 0}
!39 = distinct !{!39, !15}
!40 = distinct !{!40, !15}
!41 = !{!42, !42, i64 0}
!42 = !{!"p1 _Sat _Accum", !9, i64 0}
!43 = !{!44, !44, i64 0}
!44 = !{!"_Sat _Accum", !5, i64 0}
!45 = distinct !{!45, !15}
!46 = distinct !{!46, !15}
!47 = !{!48, !48, i64 0}
!48 = !{!"p1 _Sat long _Accum", !9, i64 0}
!49 = !{!50, !50, i64 0}
!50 = !{!"_Sat long _Accum", !5, i64 0}
!51 = distinct !{!51, !15}
!52 = distinct !{!52, !15}
!53 = distinct !{!53, !15}
!54 = distinct !{!54, !15}
!55 = distinct !{!55, !15}
!56 = distinct !{!56, !15}
!57 = distinct !{!57, !15}
!58 = distinct !{!58, !15}
!59 = distinct !{!59, !15}
!60 = distinct !{!60, !15}
!61 = distinct !{!61, !15}
!62 = distinct !{!62, !15}
!63 = distinct !{!63, !15}
!64 = distinct !{!64, !15}
!65 = distinct !{!65, !15}
