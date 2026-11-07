; ModuleID = '/tmp/w15nest/src/stft_fixed.c'
source_filename = "/tmp/w15nest/src/stft_fixed.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@tw_re = internal global [128 x i64] zeroinitializer, align 8
@tw_im = internal global [128 x i64] zeroinitializer, align 8
@window = internal global [256 x i64] zeroinitializer, align 8
@rev = internal global [256 x i8] zeroinitializer, align 1
@.str = private unnamed_addr constant [7 x i8] c"SIGNAL\00", align 1
@.str.1 = private unnamed_addr constant [8 x i8] c"SPECTRA\00", align 1
@block = internal global [2 x [128 x i16]] zeroinitializer, align 2
@re = internal global [256 x i64] zeroinitializer, align 8
@im = internal global [256 x i64] zeroinitializer, align 8
@row = internal global [129 x i16] zeroinitializer, align 2

; Function Attrs: noinline nounwind
define dso_local i64 @bench_stft_fixed() #0 {
  %1 = alloca i64, align 8
  %2 = alloca i64, align 8
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  %5 = alloca i64, align 8
  %6 = alloca i64, align 8
  %7 = alloca i64, align 8
  %8 = alloca i64, align 8
  %9 = alloca i64, align 8
  %10 = alloca i64, align 8
  %11 = alloca i64, align 8
  %12 = alloca i64, align 8
  %13 = alloca i64, align 8
  %14 = alloca i64, align 8
  %15 = alloca i64, align 8
  %16 = alloca i32, align 4
  %17 = alloca i32, align 4
  %18 = alloca i32, align 4
  %19 = alloca i32, align 4
  %20 = alloca i32, align 4
  %21 = alloca i32, align 4
  %22 = alloca i32, align 4
  %23 = alloca i32, align 4
  %24 = alloca i32, align 4
  %25 = alloca i32, align 4
  %26 = alloca i32, align 4
  %27 = alloca i32, align 4
  %28 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(ptr %2) #4
  store i64 32768, ptr %2, align 8, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %3) #4
  store i64 0, ptr %3, align 8, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %4) #4
  call void @llvm.lifetime.start.p0(ptr %5) #4
  call void @llvm.lifetime.start.p0(ptr %6) #4
  call void @llvm.lifetime.start.p0(ptr %7) #4
  call void @llvm.lifetime.start.p0(ptr %8) #4
  call void @llvm.lifetime.start.p0(ptr %9) #4
  call void @llvm.lifetime.start.p0(ptr %10) #4
  call void @llvm.lifetime.start.p0(ptr %11) #4
  call void @llvm.lifetime.start.p0(ptr %12) #4
  call void @llvm.lifetime.start.p0(ptr %13) #4
  store i64 0, ptr %13, align 8, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %14) #4
  store i64 0, ptr %14, align 8, !tbaa !15
  call void @llvm.lifetime.start.p0(ptr %15) #4
  call void @llvm.lifetime.start.p0(ptr %16) #4
  call void @llvm.lifetime.start.p0(ptr %17) #4
  call void @llvm.lifetime.start.p0(ptr %18) #4
  call void @llvm.lifetime.start.p0(ptr %19) #4
  call void @llvm.lifetime.start.p0(ptr %20) #4
  call void @llvm.lifetime.start.p0(ptr %21) #4
  call void @llvm.lifetime.start.p0(ptr %22) #4
  call void @llvm.lifetime.start.p0(ptr %23) #4
  call void @llvm.lifetime.start.p0(ptr %24) #4
  call void @llvm.lifetime.start.p0(ptr %25) #4
  call void @llvm.lifetime.start.p0(ptr %26) #4
  call void @llvm.lifetime.start.p0(ptr %27) #4
  store i32 0, ptr %27, align 4, !tbaa !17
  store i32 0, ptr %19, align 4, !tbaa !17
  br label %29

29:                                               ; preds = %59, %0
  %30 = load i32, ptr %19, align 4, !tbaa !17
  %31 = icmp slt i32 %30, 128
  br i1 %31, label %32, label %62

32:                                               ; preds = %29
  %33 = load i64, ptr %2, align 8, !tbaa !15
  %34 = sdiv i64 %33, 64
  %35 = load i32, ptr %19, align 4, !tbaa !17
  %36 = sext i32 %35 to i64
  %37 = getelementptr inbounds [128 x i64], ptr @tw_re, i64 0, i64 %36
  store i64 %34, ptr %37, align 8, !tbaa !15
  %38 = load i64, ptr %3, align 8, !tbaa !15
  %39 = sdiv i64 %38, 64
  %40 = sub nsw i64 0, %39
  %41 = load i32, ptr %19, align 4, !tbaa !17
  %42 = sext i32 %41 to i64
  %43 = getelementptr inbounds [128 x i64], ptr @tw_im, i64 0, i64 %42
  store i64 %40, ptr %43, align 8, !tbaa !15
  %44 = load i64, ptr %2, align 8, !tbaa !15
  %45 = mul nsw i64 %44, 32758
  %46 = ashr i64 %45, 15
  %47 = load i64, ptr %3, align 8, !tbaa !15
  %48 = mul nsw i64 %47, 804
  %49 = ashr i64 %48, 15
  %50 = sub nsw i64 %46, %49
  store i64 %50, ptr %4, align 8, !tbaa !15
  %51 = load i64, ptr %3, align 8, !tbaa !15
  %52 = mul nsw i64 %51, 32758
  %53 = ashr i64 %52, 15
  %54 = load i64, ptr %2, align 8, !tbaa !15
  %55 = mul nsw i64 %54, 804
  %56 = ashr i64 %55, 15
  %57 = add nsw i64 %53, %56
  store i64 %57, ptr %3, align 8, !tbaa !15
  %58 = load i64, ptr %4, align 8, !tbaa !15
  store i64 %58, ptr %2, align 8, !tbaa !15
  br label %59

59:                                               ; preds = %32
  %60 = load i32, ptr %19, align 4, !tbaa !17
  %61 = add nsw i32 %60, 1
  store i32 %61, ptr %19, align 4, !tbaa !17
  br label %29, !llvm.loop !18

62:                                               ; preds = %29
  store i32 0, ptr %19, align 4, !tbaa !17
  br label %63

63:                                               ; preds = %85, %62
  %64 = load i32, ptr %19, align 4, !tbaa !17
  %65 = icmp slt i32 %64, 128
  br i1 %65, label %66, label %88

66:                                               ; preds = %63
  %67 = load i32, ptr %19, align 4, !tbaa !17
  %68 = sext i32 %67 to i64
  %69 = getelementptr inbounds [128 x i64], ptr @tw_re, i64 0, i64 %68
  %70 = load i64, ptr %69, align 8, !tbaa !15
  %71 = sub nsw i64 512, %70
  %72 = sdiv i64 %71, 2
  %73 = load i32, ptr %19, align 4, !tbaa !17
  %74 = sext i32 %73 to i64
  %75 = getelementptr inbounds [256 x i64], ptr @window, i64 0, i64 %74
  store i64 %72, ptr %75, align 8, !tbaa !15
  %76 = load i32, ptr %19, align 4, !tbaa !17
  %77 = sext i32 %76 to i64
  %78 = getelementptr inbounds [256 x i64], ptr @window, i64 0, i64 %77
  %79 = load i64, ptr %78, align 8, !tbaa !15
  %80 = sub nsw i64 512, %79
  %81 = load i32, ptr %19, align 4, !tbaa !17
  %82 = add nsw i32 %81, 128
  %83 = sext i32 %82 to i64
  %84 = getelementptr inbounds [256 x i64], ptr @window, i64 0, i64 %83
  store i64 %80, ptr %84, align 8, !tbaa !15
  br label %85

85:                                               ; preds = %66
  %86 = load i32, ptr %19, align 4, !tbaa !17
  %87 = add nsw i32 %86, 1
  store i32 %87, ptr %19, align 4, !tbaa !17
  br label %63, !llvm.loop !20

88:                                               ; preds = %63
  store i32 0, ptr %20, align 4, !tbaa !17
  br label %89

89:                                               ; preds = %114, %88
  %90 = load i32, ptr %20, align 4, !tbaa !17
  %91 = icmp slt i32 %90, 256
  br i1 %91, label %92, label %117

92:                                               ; preds = %89
  store i32 0, ptr %25, align 4, !tbaa !17
  %93 = load i32, ptr %20, align 4, !tbaa !17
  store i32 %93, ptr %26, align 4, !tbaa !17
  store i32 0, ptr %21, align 4, !tbaa !17
  br label %94

94:                                               ; preds = %103, %92
  %95 = load i32, ptr %21, align 4, !tbaa !17
  %96 = icmp slt i32 %95, 8
  br i1 %96, label %97, label %108

97:                                               ; preds = %94
  %98 = load i32, ptr %25, align 4, !tbaa !17
  %99 = mul nsw i32 %98, 2
  %100 = load i32, ptr %26, align 4, !tbaa !17
  %101 = and i32 %100, 1
  %102 = add nsw i32 %99, %101
  store i32 %102, ptr %25, align 4, !tbaa !17
  br label %103

103:                                              ; preds = %97
  %104 = load i32, ptr %21, align 4, !tbaa !17
  %105 = add nsw i32 %104, 1
  store i32 %105, ptr %21, align 4, !tbaa !17
  %106 = load i32, ptr %26, align 4, !tbaa !17
  %107 = ashr i32 %106, 1
  store i32 %107, ptr %26, align 4, !tbaa !17
  br label %94, !llvm.loop !21

108:                                              ; preds = %94
  %109 = load i32, ptr %25, align 4, !tbaa !17
  %110 = trunc i32 %109 to i8
  %111 = load i32, ptr %20, align 4, !tbaa !17
  %112 = sext i32 %111 to i64
  %113 = getelementptr inbounds [256 x i8], ptr @rev, i64 0, i64 %112
  store i8 %110, ptr %113, align 1, !tbaa !22
  br label %114

114:                                              ; preds = %108
  %115 = load i32, ptr %20, align 4, !tbaa !17
  %116 = add nsw i32 %115, 1
  store i32 %116, ptr %20, align 4, !tbaa !17
  br label %89, !llvm.loop !23

117:                                              ; preds = %89
  %118 = call i32 @llrm_os_open(ptr noundef @.str, i8 noundef 0) #5
  store i32 %118, ptr %16, align 4, !tbaa !17
  %119 = call i32 @llrm_os_create(ptr noundef @.str.1) #5
  store i32 %119, ptr %17, align 4, !tbaa !17
  %120 = load i32, ptr %16, align 4, !tbaa !17
  %121 = icmp slt i32 %120, 0
  br i1 %121, label %125, label %122

122:                                              ; preds = %117
  %123 = load i32, ptr %17, align 4, !tbaa !17
  %124 = icmp slt i32 %123, 0
  br i1 %124, label %125, label %126

125:                                              ; preds = %122, %117
  store i64 -1, ptr %1, align 8
  store i32 1, ptr %28, align 4
  br label %403

126:                                              ; preds = %122
  %127 = load i32, ptr %16, align 4, !tbaa !17
  %128 = load i32, ptr %27, align 4, !tbaa !17
  %129 = sext i32 %128 to i64
  %130 = getelementptr inbounds [2 x [128 x i16]], ptr @block, i64 0, i64 %129
  %131 = getelementptr inbounds [128 x i16], ptr %130, i64 0, i64 0
  %132 = call i32 @llrm_os_read(i32 noundef %127, ptr noundef %131, i64 noundef 256) #5
  %133 = icmp ne i32 %132, 256
  br i1 %133, label %134, label %135

134:                                              ; preds = %126
  store i64 -1, ptr %1, align 8
  store i32 1, ptr %28, align 4
  br label %403

135:                                              ; preds = %126
  br label %136

136:                                              ; preds = %394, %135
  %137 = load i32, ptr %16, align 4, !tbaa !17
  %138 = load i32, ptr %27, align 4, !tbaa !17
  %139 = sub nsw i32 1, %138
  %140 = sext i32 %139 to i64
  %141 = getelementptr inbounds [2 x [128 x i16]], ptr @block, i64 0, i64 %140
  %142 = getelementptr inbounds [128 x i16], ptr %141, i64 0, i64 0
  %143 = call i32 @llrm_os_read(i32 noundef %137, ptr noundef %142, i64 noundef 256) #5
  %144 = icmp eq i32 %143, 256
  br i1 %144, label %145, label %397

145:                                              ; preds = %136
  store i32 0, ptr %18, align 4, !tbaa !17
  br label %146

146:                                              ; preds = %206, %145
  %147 = load i32, ptr %18, align 4, !tbaa !17
  %148 = icmp slt i32 %147, 128
  br i1 %148, label %149, label %209

149:                                              ; preds = %146
  %150 = load i32, ptr %27, align 4, !tbaa !17
  %151 = sext i32 %150 to i64
  %152 = getelementptr inbounds [2 x [128 x i16]], ptr @block, i64 0, i64 %151
  %153 = load i32, ptr %18, align 4, !tbaa !17
  %154 = sext i32 %153 to i64
  %155 = getelementptr inbounds [128 x i16], ptr %152, i64 0, i64 %154
  %156 = load i16, ptr %155, align 2, !tbaa !24
  %157 = sext i16 %156 to i64
  %158 = sdiv i64 %157, 4
  %159 = load i32, ptr %18, align 4, !tbaa !17
  %160 = sext i32 %159 to i64
  %161 = getelementptr inbounds [256 x i64], ptr @window, i64 0, i64 %160
  %162 = load i64, ptr %161, align 8, !tbaa !15
  %163 = mul nsw i64 %158, %162
  %164 = load i32, ptr %18, align 4, !tbaa !17
  %165 = sext i32 %164 to i64
  %166 = getelementptr inbounds [256 x i8], ptr @rev, i64 0, i64 %165
  %167 = load i8, ptr %166, align 1, !tbaa !22
  %168 = zext i8 %167 to i64
  %169 = getelementptr inbounds nuw [256 x i64], ptr @re, i64 0, i64 %168
  store i64 %163, ptr %169, align 8, !tbaa !15
  %170 = load i32, ptr %27, align 4, !tbaa !17
  %171 = sub nsw i32 1, %170
  %172 = sext i32 %171 to i64
  %173 = getelementptr inbounds [2 x [128 x i16]], ptr @block, i64 0, i64 %172
  %174 = load i32, ptr %18, align 4, !tbaa !17
  %175 = sext i32 %174 to i64
  %176 = getelementptr inbounds [128 x i16], ptr %173, i64 0, i64 %175
  %177 = load i16, ptr %176, align 2, !tbaa !24
  %178 = sext i16 %177 to i64
  %179 = sdiv i64 %178, 4
  %180 = load i32, ptr %18, align 4, !tbaa !17
  %181 = add nsw i32 %180, 128
  %182 = sext i32 %181 to i64
  %183 = getelementptr inbounds [256 x i64], ptr @window, i64 0, i64 %182
  %184 = load i64, ptr %183, align 8, !tbaa !15
  %185 = mul nsw i64 %179, %184
  %186 = load i32, ptr %18, align 4, !tbaa !17
  %187 = add nsw i32 %186, 128
  %188 = sext i32 %187 to i64
  %189 = getelementptr inbounds [256 x i8], ptr @rev, i64 0, i64 %188
  %190 = load i8, ptr %189, align 1, !tbaa !22
  %191 = zext i8 %190 to i64
  %192 = getelementptr inbounds nuw [256 x i64], ptr @re, i64 0, i64 %191
  store i64 %185, ptr %192, align 8, !tbaa !15
  %193 = load i32, ptr %18, align 4, !tbaa !17
  %194 = sext i32 %193 to i64
  %195 = getelementptr inbounds [256 x i8], ptr @rev, i64 0, i64 %194
  %196 = load i8, ptr %195, align 1, !tbaa !22
  %197 = zext i8 %196 to i64
  %198 = getelementptr inbounds nuw [256 x i64], ptr @im, i64 0, i64 %197
  store i64 0, ptr %198, align 8, !tbaa !15
  %199 = load i32, ptr %18, align 4, !tbaa !17
  %200 = add nsw i32 %199, 128
  %201 = sext i32 %200 to i64
  %202 = getelementptr inbounds [256 x i8], ptr @rev, i64 0, i64 %201
  %203 = load i8, ptr %202, align 1, !tbaa !22
  %204 = zext i8 %203 to i64
  %205 = getelementptr inbounds nuw [256 x i64], ptr @im, i64 0, i64 %204
  store i64 0, ptr %205, align 8, !tbaa !15
  br label %206

206:                                              ; preds = %149
  %207 = load i32, ptr %18, align 4, !tbaa !17
  %208 = add nsw i32 %207, 1
  store i32 %208, ptr %18, align 4, !tbaa !17
  br label %146, !llvm.loop !26

209:                                              ; preds = %146
  store i32 2, ptr %22, align 4, !tbaa !17
  br label %210

210:                                              ; preds = %342, %209
  %211 = load i32, ptr %22, align 4, !tbaa !17
  %212 = icmp sle i32 %211, 256
  br i1 %212, label %213, label %345

213:                                              ; preds = %210
  %214 = load i32, ptr %22, align 4, !tbaa !17
  %215 = sdiv i32 %214, 2
  store i32 %215, ptr %23, align 4, !tbaa !17
  %216 = load i32, ptr %22, align 4, !tbaa !17
  %217 = sdiv i32 256, %216
  store i32 %217, ptr %24, align 4, !tbaa !17
  store i32 0, ptr %20, align 4, !tbaa !17
  br label %218

218:                                              ; preds = %337, %213
  %219 = load i32, ptr %20, align 4, !tbaa !17
  %220 = icmp slt i32 %219, 256
  br i1 %220, label %221, label %341

221:                                              ; preds = %218
  store i32 0, ptr %21, align 4, !tbaa !17
  br label %222

222:                                              ; preds = %333, %221
  %223 = load i32, ptr %21, align 4, !tbaa !17
  %224 = load i32, ptr %23, align 4, !tbaa !17
  %225 = icmp slt i32 %223, %224
  br i1 %225, label %226, label %336

226:                                              ; preds = %222
  %227 = load i32, ptr %20, align 4, !tbaa !17
  %228 = load i32, ptr %21, align 4, !tbaa !17
  %229 = add nsw i32 %227, %228
  %230 = sext i32 %229 to i64
  %231 = getelementptr inbounds [256 x i64], ptr @re, i64 0, i64 %230
  %232 = load i64, ptr %231, align 8, !tbaa !15
  store i64 %232, ptr %5, align 8, !tbaa !15
  %233 = load i32, ptr %20, align 4, !tbaa !17
  %234 = load i32, ptr %21, align 4, !tbaa !17
  %235 = add nsw i32 %233, %234
  %236 = sext i32 %235 to i64
  %237 = getelementptr inbounds [256 x i64], ptr @im, i64 0, i64 %236
  %238 = load i64, ptr %237, align 8, !tbaa !15
  store i64 %238, ptr %6, align 8, !tbaa !15
  %239 = load i32, ptr %20, align 4, !tbaa !17
  %240 = load i32, ptr %21, align 4, !tbaa !17
  %241 = add nsw i32 %239, %240
  %242 = load i32, ptr %23, align 4, !tbaa !17
  %243 = add nsw i32 %241, %242
  %244 = sext i32 %243 to i64
  %245 = getelementptr inbounds [256 x i64], ptr @re, i64 0, i64 %244
  %246 = load i64, ptr %245, align 8, !tbaa !15
  store i64 %246, ptr %7, align 8, !tbaa !15
  %247 = load i32, ptr %20, align 4, !tbaa !17
  %248 = load i32, ptr %21, align 4, !tbaa !17
  %249 = add nsw i32 %247, %248
  %250 = load i32, ptr %23, align 4, !tbaa !17
  %251 = add nsw i32 %249, %250
  %252 = sext i32 %251 to i64
  %253 = getelementptr inbounds [256 x i64], ptr @im, i64 0, i64 %252
  %254 = load i64, ptr %253, align 8, !tbaa !15
  store i64 %254, ptr %8, align 8, !tbaa !15
  %255 = load i32, ptr %21, align 4, !tbaa !17
  %256 = load i32, ptr %24, align 4, !tbaa !17
  %257 = mul nsw i32 %255, %256
  %258 = sext i32 %257 to i64
  %259 = getelementptr inbounds [128 x i64], ptr @tw_re, i64 0, i64 %258
  %260 = load i64, ptr %259, align 8, !tbaa !15
  %261 = load i64, ptr %7, align 8, !tbaa !15
  %262 = mul nsw i64 %260, %261
  %263 = ashr i64 %262, 9
  %264 = load i32, ptr %21, align 4, !tbaa !17
  %265 = load i32, ptr %24, align 4, !tbaa !17
  %266 = mul nsw i32 %264, %265
  %267 = sext i32 %266 to i64
  %268 = getelementptr inbounds [128 x i64], ptr @tw_im, i64 0, i64 %267
  %269 = load i64, ptr %268, align 8, !tbaa !15
  %270 = load i64, ptr %8, align 8, !tbaa !15
  %271 = mul nsw i64 %269, %270
  %272 = ashr i64 %271, 9
  %273 = sub nsw i64 %263, %272
  store i64 %273, ptr %9, align 8, !tbaa !15
  %274 = load i32, ptr %21, align 4, !tbaa !17
  %275 = load i32, ptr %24, align 4, !tbaa !17
  %276 = mul nsw i32 %274, %275
  %277 = sext i32 %276 to i64
  %278 = getelementptr inbounds [128 x i64], ptr @tw_re, i64 0, i64 %277
  %279 = load i64, ptr %278, align 8, !tbaa !15
  %280 = load i64, ptr %8, align 8, !tbaa !15
  %281 = mul nsw i64 %279, %280
  %282 = ashr i64 %281, 9
  %283 = load i32, ptr %21, align 4, !tbaa !17
  %284 = load i32, ptr %24, align 4, !tbaa !17
  %285 = mul nsw i32 %283, %284
  %286 = sext i32 %285 to i64
  %287 = getelementptr inbounds [128 x i64], ptr @tw_im, i64 0, i64 %286
  %288 = load i64, ptr %287, align 8, !tbaa !15
  %289 = load i64, ptr %7, align 8, !tbaa !15
  %290 = mul nsw i64 %288, %289
  %291 = ashr i64 %290, 9
  %292 = add nsw i64 %282, %291
  store i64 %292, ptr %10, align 8, !tbaa !15
  %293 = load i64, ptr %5, align 8, !tbaa !15
  %294 = load i64, ptr %9, align 8, !tbaa !15
  %295 = add nsw i64 %293, %294
  %296 = sdiv i64 %295, 2
  %297 = load i32, ptr %20, align 4, !tbaa !17
  %298 = load i32, ptr %21, align 4, !tbaa !17
  %299 = add nsw i32 %297, %298
  %300 = sext i32 %299 to i64
  %301 = getelementptr inbounds [256 x i64], ptr @re, i64 0, i64 %300
  store i64 %296, ptr %301, align 8, !tbaa !15
  %302 = load i64, ptr %6, align 8, !tbaa !15
  %303 = load i64, ptr %10, align 8, !tbaa !15
  %304 = add nsw i64 %302, %303
  %305 = sdiv i64 %304, 2
  %306 = load i32, ptr %20, align 4, !tbaa !17
  %307 = load i32, ptr %21, align 4, !tbaa !17
  %308 = add nsw i32 %306, %307
  %309 = sext i32 %308 to i64
  %310 = getelementptr inbounds [256 x i64], ptr @im, i64 0, i64 %309
  store i64 %305, ptr %310, align 8, !tbaa !15
  %311 = load i64, ptr %5, align 8, !tbaa !15
  %312 = load i64, ptr %9, align 8, !tbaa !15
  %313 = sub nsw i64 %311, %312
  %314 = sdiv i64 %313, 2
  %315 = load i32, ptr %20, align 4, !tbaa !17
  %316 = load i32, ptr %21, align 4, !tbaa !17
  %317 = add nsw i32 %315, %316
  %318 = load i32, ptr %23, align 4, !tbaa !17
  %319 = add nsw i32 %317, %318
  %320 = sext i32 %319 to i64
  %321 = getelementptr inbounds [256 x i64], ptr @re, i64 0, i64 %320
  store i64 %314, ptr %321, align 8, !tbaa !15
  %322 = load i64, ptr %6, align 8, !tbaa !15
  %323 = load i64, ptr %10, align 8, !tbaa !15
  %324 = sub nsw i64 %322, %323
  %325 = sdiv i64 %324, 2
  %326 = load i32, ptr %20, align 4, !tbaa !17
  %327 = load i32, ptr %21, align 4, !tbaa !17
  %328 = add nsw i32 %326, %327
  %329 = load i32, ptr %23, align 4, !tbaa !17
  %330 = add nsw i32 %328, %329
  %331 = sext i32 %330 to i64
  %332 = getelementptr inbounds [256 x i64], ptr @im, i64 0, i64 %331
  store i64 %325, ptr %332, align 8, !tbaa !15
  br label %333

333:                                              ; preds = %226
  %334 = load i32, ptr %21, align 4, !tbaa !17
  %335 = add nsw i32 %334, 1
  store i32 %335, ptr %21, align 4, !tbaa !17
  br label %222, !llvm.loop !27

336:                                              ; preds = %222
  br label %337

337:                                              ; preds = %336
  %338 = load i32, ptr %22, align 4, !tbaa !17
  %339 = load i32, ptr %20, align 4, !tbaa !17
  %340 = add nsw i32 %339, %338
  store i32 %340, ptr %20, align 4, !tbaa !17
  br label %218, !llvm.loop !28

341:                                              ; preds = %218
  br label %342

342:                                              ; preds = %341
  %343 = load i32, ptr %22, align 4, !tbaa !17
  %344 = mul nsw i32 %343, 2
  store i32 %344, ptr %22, align 4, !tbaa !17
  br label %210, !llvm.loop !29

345:                                              ; preds = %210
  store i32 0, ptr %19, align 4, !tbaa !17
  br label %346

346:                                              ; preds = %386, %345
  %347 = load i32, ptr %19, align 4, !tbaa !17
  %348 = icmp slt i32 %347, 129
  br i1 %348, label %349, label %389

349:                                              ; preds = %346
  %350 = load i32, ptr %19, align 4, !tbaa !17
  %351 = sext i32 %350 to i64
  %352 = getelementptr inbounds [256 x i64], ptr @re, i64 0, i64 %351
  %353 = load i64, ptr %352, align 8, !tbaa !15
  %354 = sdiv i64 %353, 128
  store i64 %354, ptr %11, align 8, !tbaa !15
  %355 = load i32, ptr %19, align 4, !tbaa !17
  %356 = sext i32 %355 to i64
  %357 = getelementptr inbounds [256 x i64], ptr @im, i64 0, i64 %356
  %358 = load i64, ptr %357, align 8, !tbaa !15
  %359 = sdiv i64 %358, 128
  store i64 %359, ptr %12, align 8, !tbaa !15
  %360 = load i64, ptr %11, align 8, !tbaa !15
  %361 = load i64, ptr %11, align 8, !tbaa !15
  %362 = mul nsw i64 %360, %361
  %363 = load i64, ptr %12, align 8, !tbaa !15
  %364 = load i64, ptr %12, align 8, !tbaa !15
  %365 = mul nsw i64 %363, %364
  %366 = add nsw i64 %362, %365
  %367 = sdiv i64 %366, 256
  store i64 %367, ptr %15, align 8, !tbaa !15
  %368 = load i64, ptr %15, align 8, !tbaa !15
  %369 = icmp sgt i64 %368, 65535
  br i1 %369, label %370, label %371

370:                                              ; preds = %349
  store i64 65535, ptr %15, align 8, !tbaa !15
  br label %371

371:                                              ; preds = %370, %349
  %372 = load i64, ptr %15, align 8, !tbaa !15
  %373 = trunc i64 %372 to i16
  %374 = load i32, ptr %19, align 4, !tbaa !17
  %375 = sext i32 %374 to i64
  %376 = getelementptr inbounds [129 x i16], ptr @row, i64 0, i64 %375
  store i16 %373, ptr %376, align 2, !tbaa !24
  %377 = load i64, ptr %13, align 8, !tbaa !15
  %378 = mul nsw i64 %377, 3
  %379 = load i64, ptr %15, align 8, !tbaa !15
  %380 = add nsw i64 %378, %379
  %381 = load i64, ptr %14, align 8, !tbaa !15
  %382 = add nsw i64 %380, %381
  %383 = and i64 %382, 536870911
  store i64 %383, ptr %13, align 8, !tbaa !15
  %384 = load i64, ptr %14, align 8, !tbaa !15
  %385 = add nsw i64 %384, 1
  store i64 %385, ptr %14, align 8, !tbaa !15
  br label %386

386:                                              ; preds = %371
  %387 = load i32, ptr %19, align 4, !tbaa !17
  %388 = add nsw i32 %387, 1
  store i32 %388, ptr %19, align 4, !tbaa !17
  br label %346, !llvm.loop !30

389:                                              ; preds = %346
  %390 = load i32, ptr %17, align 4, !tbaa !17
  %391 = call i32 @llrm_os_write_file(i32 noundef %390, ptr noundef @row, i64 noundef 258) #5
  %392 = icmp ne i32 %391, 258
  br i1 %392, label %393, label %394

393:                                              ; preds = %389
  store i64 -1, ptr %1, align 8
  store i32 1, ptr %28, align 4
  br label %403

394:                                              ; preds = %389
  %395 = load i32, ptr %27, align 4, !tbaa !17
  %396 = sub nsw i32 1, %395
  store i32 %396, ptr %27, align 4, !tbaa !17
  br label %136, !llvm.loop !31

397:                                              ; preds = %136
  %398 = load i32, ptr %16, align 4, !tbaa !17
  %399 = call i64 @llrm_os_close(i32 noundef %398) #5
  %400 = load i32, ptr %17, align 4, !tbaa !17
  %401 = call i64 @llrm_os_close(i32 noundef %400) #5
  %402 = load i64, ptr %13, align 8, !tbaa !15
  store i64 %402, ptr %1, align 8
  store i32 1, ptr %28, align 4
  br label %403

403:                                              ; preds = %397, %393, %134, %125
  call void @llvm.lifetime.end.p0(ptr %27) #4
  call void @llvm.lifetime.end.p0(ptr %26) #4
  call void @llvm.lifetime.end.p0(ptr %25) #4
  call void @llvm.lifetime.end.p0(ptr %24) #4
  call void @llvm.lifetime.end.p0(ptr %23) #4
  call void @llvm.lifetime.end.p0(ptr %22) #4
  call void @llvm.lifetime.end.p0(ptr %21) #4
  call void @llvm.lifetime.end.p0(ptr %20) #4
  call void @llvm.lifetime.end.p0(ptr %19) #4
  call void @llvm.lifetime.end.p0(ptr %18) #4
  call void @llvm.lifetime.end.p0(ptr %17) #4
  call void @llvm.lifetime.end.p0(ptr %16) #4
  call void @llvm.lifetime.end.p0(ptr %15) #4
  call void @llvm.lifetime.end.p0(ptr %14) #4
  call void @llvm.lifetime.end.p0(ptr %13) #4
  call void @llvm.lifetime.end.p0(ptr %12) #4
  call void @llvm.lifetime.end.p0(ptr %11) #4
  call void @llvm.lifetime.end.p0(ptr %10) #4
  call void @llvm.lifetime.end.p0(ptr %9) #4
  call void @llvm.lifetime.end.p0(ptr %8) #4
  call void @llvm.lifetime.end.p0(ptr %7) #4
  call void @llvm.lifetime.end.p0(ptr %6) #4
  call void @llvm.lifetime.end.p0(ptr %5) #4
  call void @llvm.lifetime.end.p0(ptr %4) #4
  call void @llvm.lifetime.end.p0(ptr %3) #4
  call void @llvm.lifetime.end.p0(ptr %2) #4
  %404 = load i64, ptr %1, align 8
  ret i64 %404
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare i32 @llrm_os_open(ptr noundef, i8 noundef) #2

declare i32 @llrm_os_create(ptr noundef) #2

declare i32 @llrm_os_read(i32 noundef, ptr noundef, i64 noundef) #2

declare i32 @llrm_os_write_file(i32 noundef, ptr noundef, i64 noundef) #2

declare i64 @llrm_os_close(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define dso_local i32 @main() #3 {
  %1 = call i64 @bench_stft_fixed() #5
  call void @report(i64 noundef %1) #5
  ret i32 0
}

declare void @report(i64 noundef) #2

attributes #0 = { noinline nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nounwind "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7, !8}
!llvm.ident = !{!9}
!llvm.errno.tbaa = !{!10}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{i32 7, !"frame-pointer", i32 4}
!9 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!10 = !{!11, !12, i64 0}
!11 = !{!"__libc_errno", !12, i64 0}
!12 = !{!"int", !13, i64 0}
!13 = !{!"omnipotent char", !14, i64 0}
!14 = !{!"Simple C/C++ TBAA"}
!15 = !{!16, !16, i64 0}
!16 = !{!"long", !13, i64 0}
!17 = !{!12, !12, i64 0}
!18 = distinct !{!18, !19}
!19 = !{!"llvm.loop.mustprogress"}
!20 = distinct !{!20, !19}
!21 = distinct !{!21, !19}
!22 = !{!13, !13, i64 0}
!23 = distinct !{!23, !19}
!24 = !{!25, !25, i64 0}
!25 = !{!"short", !13, i64 0}
!26 = distinct !{!26, !19}
!27 = distinct !{!27, !19}
!28 = distinct !{!28, !19}
!29 = distinct !{!29, !19}
!30 = distinct !{!30, !19}
!31 = distinct !{!31, !19}
