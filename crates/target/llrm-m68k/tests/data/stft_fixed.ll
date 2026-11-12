; ModuleID = 'bench/stft_fixed/stft_fixed.c'
source_filename = "bench/stft_fixed/stft_fixed.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@tw_re = internal global [128 x i32] zeroinitializer, align 4
@tw_im = internal global [128 x i32] zeroinitializer, align 4
@window = internal global [256 x i32] zeroinitializer, align 4
@rev = internal global [256 x i8] zeroinitializer, align 1
@.str = private unnamed_addr constant [7 x i8] c"SIGNAL\00", align 1
@.str.1 = private unnamed_addr constant [8 x i8] c"SPECTRA\00", align 1
@block = internal global [2 x [128 x i16]] zeroinitializer, align 2
@re = internal global [256 x i32] zeroinitializer, align 4
@im = internal global [256 x i32] zeroinitializer, align 4
@row = internal global [129 x i16] zeroinitializer, align 2

; Function Attrs: nounwind uwtable
define dso_local i32 @bench_stft_fixed() #0 {
  %1 = alloca i32, align 4
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  %11 = alloca i32, align 4
  %12 = alloca i32, align 4
  %13 = alloca i32, align 4
  %14 = alloca i32, align 4
  %15 = alloca i32, align 4
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
  call void @llvm.lifetime.start.p0(ptr %2) #3
  store i32 32768, ptr %2, align 4, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 4, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  call void @llvm.lifetime.start.p0(ptr %6) #3
  call void @llvm.lifetime.start.p0(ptr %7) #3
  call void @llvm.lifetime.start.p0(ptr %8) #3
  call void @llvm.lifetime.start.p0(ptr %9) #3
  call void @llvm.lifetime.start.p0(ptr %10) #3
  call void @llvm.lifetime.start.p0(ptr %11) #3
  call void @llvm.lifetime.start.p0(ptr %12) #3
  call void @llvm.lifetime.start.p0(ptr %13) #3
  store i32 0, ptr %13, align 4, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %14) #3
  store i32 0, ptr %14, align 4, !tbaa !10
  call void @llvm.lifetime.start.p0(ptr %15) #3
  call void @llvm.lifetime.start.p0(ptr %16) #3
  call void @llvm.lifetime.start.p0(ptr %17) #3
  call void @llvm.lifetime.start.p0(ptr %18) #3
  call void @llvm.lifetime.start.p0(ptr %19) #3
  call void @llvm.lifetime.start.p0(ptr %20) #3
  call void @llvm.lifetime.start.p0(ptr %21) #3
  call void @llvm.lifetime.start.p0(ptr %22) #3
  call void @llvm.lifetime.start.p0(ptr %23) #3
  call void @llvm.lifetime.start.p0(ptr %24) #3
  call void @llvm.lifetime.start.p0(ptr %25) #3
  call void @llvm.lifetime.start.p0(ptr %26) #3
  call void @llvm.lifetime.start.p0(ptr %27) #3
  store i32 0, ptr %27, align 4, !tbaa !12
  store i32 0, ptr %19, align 4, !tbaa !12
  br label %29

29:                                               ; preds = %57, %0
  %30 = load i32, ptr %19, align 4, !tbaa !12
  %31 = icmp slt i32 %30, 128
  br i1 %31, label %32, label %60

32:                                               ; preds = %29
  %33 = load i32, ptr %2, align 4, !tbaa !10
  %34 = sdiv i32 %33, 64
  %35 = load i32, ptr %19, align 4, !tbaa !12
  %36 = getelementptr inbounds [128 x i32], ptr @tw_re, i32 0, i32 %35
  store i32 %34, ptr %36, align 4, !tbaa !10
  %37 = load i32, ptr %3, align 4, !tbaa !10
  %38 = sdiv i32 %37, 64
  %39 = sub nsw i32 0, %38
  %40 = load i32, ptr %19, align 4, !tbaa !12
  %41 = getelementptr inbounds [128 x i32], ptr @tw_im, i32 0, i32 %40
  store i32 %39, ptr %41, align 4, !tbaa !10
  %42 = load i32, ptr %2, align 4, !tbaa !10
  %43 = mul nsw i32 %42, 32758
  %44 = ashr i32 %43, 15
  %45 = load i32, ptr %3, align 4, !tbaa !10
  %46 = mul nsw i32 %45, 804
  %47 = ashr i32 %46, 15
  %48 = sub nsw i32 %44, %47
  store i32 %48, ptr %4, align 4, !tbaa !10
  %49 = load i32, ptr %3, align 4, !tbaa !10
  %50 = mul nsw i32 %49, 32758
  %51 = ashr i32 %50, 15
  %52 = load i32, ptr %2, align 4, !tbaa !10
  %53 = mul nsw i32 %52, 804
  %54 = ashr i32 %53, 15
  %55 = add nsw i32 %51, %54
  store i32 %55, ptr %3, align 4, !tbaa !10
  %56 = load i32, ptr %4, align 4, !tbaa !10
  store i32 %56, ptr %2, align 4, !tbaa !10
  br label %57

57:                                               ; preds = %32
  %58 = load i32, ptr %19, align 4, !tbaa !12
  %59 = add nsw i32 %58, 1
  store i32 %59, ptr %19, align 4, !tbaa !12
  br label %29, !llvm.loop !13

60:                                               ; preds = %29
  store i32 0, ptr %19, align 4, !tbaa !12
  br label %61

61:                                               ; preds = %79, %60
  %62 = load i32, ptr %19, align 4, !tbaa !12
  %63 = icmp slt i32 %62, 128
  br i1 %63, label %64, label %82

64:                                               ; preds = %61
  %65 = load i32, ptr %19, align 4, !tbaa !12
  %66 = getelementptr inbounds [128 x i32], ptr @tw_re, i32 0, i32 %65
  %67 = load i32, ptr %66, align 4, !tbaa !10
  %68 = sub nsw i32 512, %67
  %69 = sdiv i32 %68, 2
  %70 = load i32, ptr %19, align 4, !tbaa !12
  %71 = getelementptr inbounds [256 x i32], ptr @window, i32 0, i32 %70
  store i32 %69, ptr %71, align 4, !tbaa !10
  %72 = load i32, ptr %19, align 4, !tbaa !12
  %73 = getelementptr inbounds [256 x i32], ptr @window, i32 0, i32 %72
  %74 = load i32, ptr %73, align 4, !tbaa !10
  %75 = sub nsw i32 512, %74
  %76 = load i32, ptr %19, align 4, !tbaa !12
  %77 = add nsw i32 %76, 128
  %78 = getelementptr inbounds [256 x i32], ptr @window, i32 0, i32 %77
  store i32 %75, ptr %78, align 4, !tbaa !10
  br label %79

79:                                               ; preds = %64
  %80 = load i32, ptr %19, align 4, !tbaa !12
  %81 = add nsw i32 %80, 1
  store i32 %81, ptr %19, align 4, !tbaa !12
  br label %61, !llvm.loop !15

82:                                               ; preds = %61
  store i32 0, ptr %20, align 4, !tbaa !12
  br label %83

83:                                               ; preds = %107, %82
  %84 = load i32, ptr %20, align 4, !tbaa !12
  %85 = icmp slt i32 %84, 256
  br i1 %85, label %86, label %110

86:                                               ; preds = %83
  store i32 0, ptr %25, align 4, !tbaa !12
  %87 = load i32, ptr %20, align 4, !tbaa !12
  store i32 %87, ptr %26, align 4, !tbaa !12
  store i32 0, ptr %21, align 4, !tbaa !12
  br label %88

88:                                               ; preds = %97, %86
  %89 = load i32, ptr %21, align 4, !tbaa !12
  %90 = icmp slt i32 %89, 8
  br i1 %90, label %91, label %102

91:                                               ; preds = %88
  %92 = load i32, ptr %25, align 4, !tbaa !12
  %93 = mul nsw i32 %92, 2
  %94 = load i32, ptr %26, align 4, !tbaa !12
  %95 = and i32 %94, 1
  %96 = add nsw i32 %93, %95
  store i32 %96, ptr %25, align 4, !tbaa !12
  br label %97

97:                                               ; preds = %91
  %98 = load i32, ptr %21, align 4, !tbaa !12
  %99 = add nsw i32 %98, 1
  store i32 %99, ptr %21, align 4, !tbaa !12
  %100 = load i32, ptr %26, align 4, !tbaa !12
  %101 = ashr i32 %100, 1
  store i32 %101, ptr %26, align 4, !tbaa !12
  br label %88, !llvm.loop !16

102:                                              ; preds = %88
  %103 = load i32, ptr %25, align 4, !tbaa !12
  %104 = trunc i32 %103 to i8
  %105 = load i32, ptr %20, align 4, !tbaa !12
  %106 = getelementptr inbounds [256 x i8], ptr @rev, i32 0, i32 %105
  store i8 %104, ptr %106, align 1, !tbaa !17
  br label %107

107:                                              ; preds = %102
  %108 = load i32, ptr %20, align 4, !tbaa !12
  %109 = add nsw i32 %108, 1
  store i32 %109, ptr %20, align 4, !tbaa !12
  br label %83, !llvm.loop !18

110:                                              ; preds = %83
  %111 = call i32 @llrm_os_open(ptr noundef @.str, i8 noundef zeroext 0)
  store i32 %111, ptr %16, align 4, !tbaa !12
  %112 = call i32 @llrm_os_create(ptr noundef @.str.1)
  store i32 %112, ptr %17, align 4, !tbaa !12
  %113 = load i32, ptr %16, align 4, !tbaa !12
  %114 = icmp slt i32 %113, 0
  br i1 %114, label %118, label %115

115:                                              ; preds = %110
  %116 = load i32, ptr %17, align 4, !tbaa !12
  %117 = icmp slt i32 %116, 0
  br i1 %117, label %118, label %119

118:                                              ; preds = %115, %110
  store i32 -1, ptr %1, align 4
  store i32 1, ptr %28, align 4
  br label %369

119:                                              ; preds = %115
  %120 = load i32, ptr %16, align 4, !tbaa !12
  %121 = load i32, ptr %27, align 4, !tbaa !12
  %122 = getelementptr inbounds [2 x [128 x i16]], ptr @block, i32 0, i32 %121
  %123 = getelementptr inbounds [128 x i16], ptr %122, i32 0, i32 0
  %124 = call i32 @llrm_os_read(i32 noundef %120, ptr noundef %123, i32 noundef 256)
  %125 = icmp ne i32 %124, 256
  br i1 %125, label %126, label %127

126:                                              ; preds = %119
  store i32 -1, ptr %1, align 4
  store i32 1, ptr %28, align 4
  br label %369

127:                                              ; preds = %119
  br label %128

128:                                              ; preds = %360, %127
  %129 = load i32, ptr %16, align 4, !tbaa !12
  %130 = load i32, ptr %27, align 4, !tbaa !12
  %131 = sub nsw i32 1, %130
  %132 = getelementptr inbounds [2 x [128 x i16]], ptr @block, i32 0, i32 %131
  %133 = getelementptr inbounds [128 x i16], ptr %132, i32 0, i32 0
  %134 = call i32 @llrm_os_read(i32 noundef %129, ptr noundef %133, i32 noundef 256)
  %135 = icmp eq i32 %134, 256
  br i1 %135, label %136, label %363

136:                                              ; preds = %128
  store i32 0, ptr %18, align 4, !tbaa !12
  br label %137

137:                                              ; preds = %187, %136
  %138 = load i32, ptr %18, align 4, !tbaa !12
  %139 = icmp slt i32 %138, 128
  br i1 %139, label %140, label %190

140:                                              ; preds = %137
  %141 = load i32, ptr %27, align 4, !tbaa !12
  %142 = getelementptr inbounds [2 x [128 x i16]], ptr @block, i32 0, i32 %141
  %143 = load i32, ptr %18, align 4, !tbaa !12
  %144 = getelementptr inbounds [128 x i16], ptr %142, i32 0, i32 %143
  %145 = load i16, ptr %144, align 2, !tbaa !19
  %146 = sext i16 %145 to i32
  %147 = sdiv i32 %146, 4
  %148 = load i32, ptr %18, align 4, !tbaa !12
  %149 = getelementptr inbounds [256 x i32], ptr @window, i32 0, i32 %148
  %150 = load i32, ptr %149, align 4, !tbaa !10
  %151 = mul nsw i32 %147, %150
  %152 = load i32, ptr %18, align 4, !tbaa !12
  %153 = getelementptr inbounds [256 x i8], ptr @rev, i32 0, i32 %152
  %154 = load i8, ptr %153, align 1, !tbaa !17
  %155 = zext i8 %154 to i32
  %156 = getelementptr inbounds nuw [256 x i32], ptr @re, i32 0, i32 %155
  store i32 %151, ptr %156, align 4, !tbaa !10
  %157 = load i32, ptr %27, align 4, !tbaa !12
  %158 = sub nsw i32 1, %157
  %159 = getelementptr inbounds [2 x [128 x i16]], ptr @block, i32 0, i32 %158
  %160 = load i32, ptr %18, align 4, !tbaa !12
  %161 = getelementptr inbounds [128 x i16], ptr %159, i32 0, i32 %160
  %162 = load i16, ptr %161, align 2, !tbaa !19
  %163 = sext i16 %162 to i32
  %164 = sdiv i32 %163, 4
  %165 = load i32, ptr %18, align 4, !tbaa !12
  %166 = add nsw i32 %165, 128
  %167 = getelementptr inbounds [256 x i32], ptr @window, i32 0, i32 %166
  %168 = load i32, ptr %167, align 4, !tbaa !10
  %169 = mul nsw i32 %164, %168
  %170 = load i32, ptr %18, align 4, !tbaa !12
  %171 = add nsw i32 %170, 128
  %172 = getelementptr inbounds [256 x i8], ptr @rev, i32 0, i32 %171
  %173 = load i8, ptr %172, align 1, !tbaa !17
  %174 = zext i8 %173 to i32
  %175 = getelementptr inbounds nuw [256 x i32], ptr @re, i32 0, i32 %174
  store i32 %169, ptr %175, align 4, !tbaa !10
  %176 = load i32, ptr %18, align 4, !tbaa !12
  %177 = getelementptr inbounds [256 x i8], ptr @rev, i32 0, i32 %176
  %178 = load i8, ptr %177, align 1, !tbaa !17
  %179 = zext i8 %178 to i32
  %180 = getelementptr inbounds nuw [256 x i32], ptr @im, i32 0, i32 %179
  store i32 0, ptr %180, align 4, !tbaa !10
  %181 = load i32, ptr %18, align 4, !tbaa !12
  %182 = add nsw i32 %181, 128
  %183 = getelementptr inbounds [256 x i8], ptr @rev, i32 0, i32 %182
  %184 = load i8, ptr %183, align 1, !tbaa !17
  %185 = zext i8 %184 to i32
  %186 = getelementptr inbounds nuw [256 x i32], ptr @im, i32 0, i32 %185
  store i32 0, ptr %186, align 4, !tbaa !10
  br label %187

187:                                              ; preds = %140
  %188 = load i32, ptr %18, align 4, !tbaa !12
  %189 = add nsw i32 %188, 1
  store i32 %189, ptr %18, align 4, !tbaa !12
  br label %137, !llvm.loop !21

190:                                              ; preds = %137
  store i32 2, ptr %22, align 4, !tbaa !12
  br label %191

191:                                              ; preds = %311, %190
  %192 = load i32, ptr %22, align 4, !tbaa !12
  %193 = icmp sle i32 %192, 256
  br i1 %193, label %194, label %314

194:                                              ; preds = %191
  %195 = load i32, ptr %22, align 4, !tbaa !12
  %196 = sdiv i32 %195, 2
  store i32 %196, ptr %23, align 4, !tbaa !12
  %197 = load i32, ptr %22, align 4, !tbaa !12
  %198 = sdiv i32 256, %197
  store i32 %198, ptr %24, align 4, !tbaa !12
  store i32 0, ptr %20, align 4, !tbaa !12
  br label %199

199:                                              ; preds = %306, %194
  %200 = load i32, ptr %20, align 4, !tbaa !12
  %201 = icmp slt i32 %200, 256
  br i1 %201, label %202, label %310

202:                                              ; preds = %199
  store i32 0, ptr %21, align 4, !tbaa !12
  br label %203

203:                                              ; preds = %302, %202
  %204 = load i32, ptr %21, align 4, !tbaa !12
  %205 = load i32, ptr %23, align 4, !tbaa !12
  %206 = icmp slt i32 %204, %205
  br i1 %206, label %207, label %305

207:                                              ; preds = %203
  %208 = load i32, ptr %20, align 4, !tbaa !12
  %209 = load i32, ptr %21, align 4, !tbaa !12
  %210 = add nsw i32 %208, %209
  %211 = getelementptr inbounds [256 x i32], ptr @re, i32 0, i32 %210
  %212 = load i32, ptr %211, align 4, !tbaa !10
  store i32 %212, ptr %5, align 4, !tbaa !10
  %213 = load i32, ptr %20, align 4, !tbaa !12
  %214 = load i32, ptr %21, align 4, !tbaa !12
  %215 = add nsw i32 %213, %214
  %216 = getelementptr inbounds [256 x i32], ptr @im, i32 0, i32 %215
  %217 = load i32, ptr %216, align 4, !tbaa !10
  store i32 %217, ptr %6, align 4, !tbaa !10
  %218 = load i32, ptr %20, align 4, !tbaa !12
  %219 = load i32, ptr %21, align 4, !tbaa !12
  %220 = add nsw i32 %218, %219
  %221 = load i32, ptr %23, align 4, !tbaa !12
  %222 = add nsw i32 %220, %221
  %223 = getelementptr inbounds [256 x i32], ptr @re, i32 0, i32 %222
  %224 = load i32, ptr %223, align 4, !tbaa !10
  store i32 %224, ptr %7, align 4, !tbaa !10
  %225 = load i32, ptr %20, align 4, !tbaa !12
  %226 = load i32, ptr %21, align 4, !tbaa !12
  %227 = add nsw i32 %225, %226
  %228 = load i32, ptr %23, align 4, !tbaa !12
  %229 = add nsw i32 %227, %228
  %230 = getelementptr inbounds [256 x i32], ptr @im, i32 0, i32 %229
  %231 = load i32, ptr %230, align 4, !tbaa !10
  store i32 %231, ptr %8, align 4, !tbaa !10
  %232 = load i32, ptr %21, align 4, !tbaa !12
  %233 = load i32, ptr %24, align 4, !tbaa !12
  %234 = mul nsw i32 %232, %233
  %235 = getelementptr inbounds [128 x i32], ptr @tw_re, i32 0, i32 %234
  %236 = load i32, ptr %235, align 4, !tbaa !10
  %237 = load i32, ptr %7, align 4, !tbaa !10
  %238 = mul nsw i32 %236, %237
  %239 = ashr i32 %238, 9
  %240 = load i32, ptr %21, align 4, !tbaa !12
  %241 = load i32, ptr %24, align 4, !tbaa !12
  %242 = mul nsw i32 %240, %241
  %243 = getelementptr inbounds [128 x i32], ptr @tw_im, i32 0, i32 %242
  %244 = load i32, ptr %243, align 4, !tbaa !10
  %245 = load i32, ptr %8, align 4, !tbaa !10
  %246 = mul nsw i32 %244, %245
  %247 = ashr i32 %246, 9
  %248 = sub nsw i32 %239, %247
  store i32 %248, ptr %9, align 4, !tbaa !10
  %249 = load i32, ptr %21, align 4, !tbaa !12
  %250 = load i32, ptr %24, align 4, !tbaa !12
  %251 = mul nsw i32 %249, %250
  %252 = getelementptr inbounds [128 x i32], ptr @tw_re, i32 0, i32 %251
  %253 = load i32, ptr %252, align 4, !tbaa !10
  %254 = load i32, ptr %8, align 4, !tbaa !10
  %255 = mul nsw i32 %253, %254
  %256 = ashr i32 %255, 9
  %257 = load i32, ptr %21, align 4, !tbaa !12
  %258 = load i32, ptr %24, align 4, !tbaa !12
  %259 = mul nsw i32 %257, %258
  %260 = getelementptr inbounds [128 x i32], ptr @tw_im, i32 0, i32 %259
  %261 = load i32, ptr %260, align 4, !tbaa !10
  %262 = load i32, ptr %7, align 4, !tbaa !10
  %263 = mul nsw i32 %261, %262
  %264 = ashr i32 %263, 9
  %265 = add nsw i32 %256, %264
  store i32 %265, ptr %10, align 4, !tbaa !10
  %266 = load i32, ptr %5, align 4, !tbaa !10
  %267 = load i32, ptr %9, align 4, !tbaa !10
  %268 = add nsw i32 %266, %267
  %269 = sdiv i32 %268, 2
  %270 = load i32, ptr %20, align 4, !tbaa !12
  %271 = load i32, ptr %21, align 4, !tbaa !12
  %272 = add nsw i32 %270, %271
  %273 = getelementptr inbounds [256 x i32], ptr @re, i32 0, i32 %272
  store i32 %269, ptr %273, align 4, !tbaa !10
  %274 = load i32, ptr %6, align 4, !tbaa !10
  %275 = load i32, ptr %10, align 4, !tbaa !10
  %276 = add nsw i32 %274, %275
  %277 = sdiv i32 %276, 2
  %278 = load i32, ptr %20, align 4, !tbaa !12
  %279 = load i32, ptr %21, align 4, !tbaa !12
  %280 = add nsw i32 %278, %279
  %281 = getelementptr inbounds [256 x i32], ptr @im, i32 0, i32 %280
  store i32 %277, ptr %281, align 4, !tbaa !10
  %282 = load i32, ptr %5, align 4, !tbaa !10
  %283 = load i32, ptr %9, align 4, !tbaa !10
  %284 = sub nsw i32 %282, %283
  %285 = sdiv i32 %284, 2
  %286 = load i32, ptr %20, align 4, !tbaa !12
  %287 = load i32, ptr %21, align 4, !tbaa !12
  %288 = add nsw i32 %286, %287
  %289 = load i32, ptr %23, align 4, !tbaa !12
  %290 = add nsw i32 %288, %289
  %291 = getelementptr inbounds [256 x i32], ptr @re, i32 0, i32 %290
  store i32 %285, ptr %291, align 4, !tbaa !10
  %292 = load i32, ptr %6, align 4, !tbaa !10
  %293 = load i32, ptr %10, align 4, !tbaa !10
  %294 = sub nsw i32 %292, %293
  %295 = sdiv i32 %294, 2
  %296 = load i32, ptr %20, align 4, !tbaa !12
  %297 = load i32, ptr %21, align 4, !tbaa !12
  %298 = add nsw i32 %296, %297
  %299 = load i32, ptr %23, align 4, !tbaa !12
  %300 = add nsw i32 %298, %299
  %301 = getelementptr inbounds [256 x i32], ptr @im, i32 0, i32 %300
  store i32 %295, ptr %301, align 4, !tbaa !10
  br label %302

302:                                              ; preds = %207
  %303 = load i32, ptr %21, align 4, !tbaa !12
  %304 = add nsw i32 %303, 1
  store i32 %304, ptr %21, align 4, !tbaa !12
  br label %203, !llvm.loop !22

305:                                              ; preds = %203
  br label %306

306:                                              ; preds = %305
  %307 = load i32, ptr %22, align 4, !tbaa !12
  %308 = load i32, ptr %20, align 4, !tbaa !12
  %309 = add nsw i32 %308, %307
  store i32 %309, ptr %20, align 4, !tbaa !12
  br label %199, !llvm.loop !23

310:                                              ; preds = %199
  br label %311

311:                                              ; preds = %310
  %312 = load i32, ptr %22, align 4, !tbaa !12
  %313 = mul nsw i32 %312, 2
  store i32 %313, ptr %22, align 4, !tbaa !12
  br label %191, !llvm.loop !24

314:                                              ; preds = %191
  store i32 0, ptr %19, align 4, !tbaa !12
  br label %315

315:                                              ; preds = %352, %314
  %316 = load i32, ptr %19, align 4, !tbaa !12
  %317 = icmp slt i32 %316, 129
  br i1 %317, label %318, label %355

318:                                              ; preds = %315
  %319 = load i32, ptr %19, align 4, !tbaa !12
  %320 = getelementptr inbounds [256 x i32], ptr @re, i32 0, i32 %319
  %321 = load i32, ptr %320, align 4, !tbaa !10
  %322 = sdiv i32 %321, 128
  store i32 %322, ptr %11, align 4, !tbaa !10
  %323 = load i32, ptr %19, align 4, !tbaa !12
  %324 = getelementptr inbounds [256 x i32], ptr @im, i32 0, i32 %323
  %325 = load i32, ptr %324, align 4, !tbaa !10
  %326 = sdiv i32 %325, 128
  store i32 %326, ptr %12, align 4, !tbaa !10
  %327 = load i32, ptr %11, align 4, !tbaa !10
  %328 = load i32, ptr %11, align 4, !tbaa !10
  %329 = mul nsw i32 %327, %328
  %330 = load i32, ptr %12, align 4, !tbaa !10
  %331 = load i32, ptr %12, align 4, !tbaa !10
  %332 = mul nsw i32 %330, %331
  %333 = add nsw i32 %329, %332
  %334 = sdiv i32 %333, 256
  store i32 %334, ptr %15, align 4, !tbaa !10
  %335 = load i32, ptr %15, align 4, !tbaa !10
  %336 = icmp sgt i32 %335, 65535
  br i1 %336, label %337, label %338

337:                                              ; preds = %318
  store i32 65535, ptr %15, align 4, !tbaa !10
  br label %338

338:                                              ; preds = %337, %318
  %339 = load i32, ptr %15, align 4, !tbaa !10
  %340 = trunc i32 %339 to i16
  %341 = load i32, ptr %19, align 4, !tbaa !12
  %342 = getelementptr inbounds [129 x i16], ptr @row, i32 0, i32 %341
  store i16 %340, ptr %342, align 2, !tbaa !19
  %343 = load i32, ptr %13, align 4, !tbaa !10
  %344 = mul nsw i32 %343, 3
  %345 = load i32, ptr %15, align 4, !tbaa !10
  %346 = add nsw i32 %344, %345
  %347 = load i32, ptr %14, align 4, !tbaa !10
  %348 = add nsw i32 %346, %347
  %349 = and i32 %348, 536870911
  store i32 %349, ptr %13, align 4, !tbaa !10
  %350 = load i32, ptr %14, align 4, !tbaa !10
  %351 = add nsw i32 %350, 1
  store i32 %351, ptr %14, align 4, !tbaa !10
  br label %352

352:                                              ; preds = %338
  %353 = load i32, ptr %19, align 4, !tbaa !12
  %354 = add nsw i32 %353, 1
  store i32 %354, ptr %19, align 4, !tbaa !12
  br label %315, !llvm.loop !25

355:                                              ; preds = %315
  %356 = load i32, ptr %17, align 4, !tbaa !12
  %357 = call i32 @llrm_os_write_file(i32 noundef %356, ptr noundef @row, i32 noundef 258)
  %358 = icmp ne i32 %357, 258
  br i1 %358, label %359, label %360

359:                                              ; preds = %355
  store i32 -1, ptr %1, align 4
  store i32 1, ptr %28, align 4
  br label %369

360:                                              ; preds = %355
  %361 = load i32, ptr %27, align 4, !tbaa !12
  %362 = sub nsw i32 1, %361
  store i32 %362, ptr %27, align 4, !tbaa !12
  br label %128, !llvm.loop !26

363:                                              ; preds = %128
  %364 = load i32, ptr %16, align 4, !tbaa !12
  %365 = call i32 @llrm_os_close(i32 noundef %364)
  %366 = load i32, ptr %17, align 4, !tbaa !12
  %367 = call i32 @llrm_os_close(i32 noundef %366)
  %368 = load i32, ptr %13, align 4, !tbaa !10
  store i32 %368, ptr %1, align 4
  store i32 1, ptr %28, align 4
  br label %369

369:                                              ; preds = %363, %359, %126, %118
  call void @llvm.lifetime.end.p0(ptr %27) #3
  call void @llvm.lifetime.end.p0(ptr %26) #3
  call void @llvm.lifetime.end.p0(ptr %25) #3
  call void @llvm.lifetime.end.p0(ptr %24) #3
  call void @llvm.lifetime.end.p0(ptr %23) #3
  call void @llvm.lifetime.end.p0(ptr %22) #3
  call void @llvm.lifetime.end.p0(ptr %21) #3
  call void @llvm.lifetime.end.p0(ptr %20) #3
  call void @llvm.lifetime.end.p0(ptr %19) #3
  call void @llvm.lifetime.end.p0(ptr %18) #3
  call void @llvm.lifetime.end.p0(ptr %17) #3
  call void @llvm.lifetime.end.p0(ptr %16) #3
  call void @llvm.lifetime.end.p0(ptr %15) #3
  call void @llvm.lifetime.end.p0(ptr %14) #3
  call void @llvm.lifetime.end.p0(ptr %13) #3
  call void @llvm.lifetime.end.p0(ptr %12) #3
  call void @llvm.lifetime.end.p0(ptr %11) #3
  call void @llvm.lifetime.end.p0(ptr %10) #3
  call void @llvm.lifetime.end.p0(ptr %9) #3
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  call void @llvm.lifetime.end.p0(ptr %6) #3
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  call void @llvm.lifetime.end.p0(ptr %3) #3
  call void @llvm.lifetime.end.p0(ptr %2) #3
  %370 = load i32, ptr %1, align 4
  ret i32 %370
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare i32 @llrm_os_open(ptr noundef, i8 noundef zeroext) #2

declare i32 @llrm_os_create(ptr noundef) #2

declare i32 @llrm_os_read(i32 noundef, ptr noundef, i32 noundef) #2

declare i32 @llrm_os_write_file(i32 noundef, ptr noundef, i32 noundef) #2

declare i32 @llrm_os_close(i32 noundef) #2

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind uwtable
define dso_local i32 @main() #0 {
  %1 = alloca i32, align 4
  store i32 0, ptr %1, align 4
  %2 = call i32 @bench_stft_fixed()
  call void @report(i32 noundef %2)
  ret i32 0
}

declare void @report(i32 noundef) #2

attributes #0 = { nounwind uwtable "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="pentium4" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #3 = { nounwind }

!llvm.module.flags = !{!0, !1, !2, !3}
!llvm.ident = !{!4}
!llvm.errno.tbaa = !{!5}

!0 = !{i32 1, !"NumRegisterParameters", i32 0}
!1 = !{i32 8, !"PIC Level", i32 2}
!2 = !{i32 7, !"PIE Level", i32 2}
!3 = !{i32 7, !"uwtable", i32 2}
!4 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!5 = !{!6, !7, i64 0}
!6 = !{!"__libc_errno", !7, i64 0}
!7 = !{!"int", !8, i64 0}
!8 = !{!"omnipotent char", !9, i64 0}
!9 = !{!"Simple C/C++ TBAA"}
!10 = !{!11, !11, i64 0}
!11 = !{!"long", !8, i64 0}
!12 = !{!7, !7, i64 0}
!13 = distinct !{!13, !14}
!14 = !{!"llvm.loop.mustprogress"}
!15 = distinct !{!15, !14}
!16 = distinct !{!16, !14}
!17 = !{!8, !8, i64 0}
!18 = distinct !{!18, !14}
!19 = !{!20, !20, i64 0}
!20 = !{!"short", !8, i64 0}
!21 = distinct !{!21, !14}
!22 = distinct !{!22, !14}
!23 = distinct !{!23, !14}
!24 = distinct !{!24, !14}
!25 = distinct !{!25, !14}
!26 = distinct !{!26, !14}
