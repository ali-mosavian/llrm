; ModuleID = 'bench/jpeg_fixed/jpeg_fixed.c'
source_filename = "bench/jpeg_fixed/jpeg_fixed.c"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

@.str = private unnamed_addr constant [10 x i8] c"PHOTO.JPG\00", align 1
@infile = internal global i32 0, align 2
@.str.1 = private unnamed_addr constant [22 x i8] c"PHOTO.JPG not found\0D\0A\00", align 1
@.str.2 = private unnamed_addr constant [13 x i8] c"not a JPEG\0D\0A\00", align 1
@bad = internal global i32 0, align 2
@.str.3 = private unnamed_addr constant [43 x i8] c"16-bit quantization tables not supported\0D\0A\00", align 1
@quant = internal global [4 x [64 x i32]] zeroinitializer, align 2
@valptr = internal global [4 x [17 x i32]] zeroinitializer, align 2
@mincode = internal global [4 x [17 x i32]] zeroinitializer, align 2
@maxcode = internal global [4 x [18 x i32]] zeroinitializer, align 2
@huffval = internal global [4 x [256 x i8]] zeroinitializer, align 1
@.str.4 = private unnamed_addr constant [35 x i8] c"only 8-bit samples are supported\0D\0A\00", align 1
@.str.5 = private unnamed_addr constant [33 x i8] c"progressive JPEG not supported\0D\0A\00", align 1
@.str.6 = private unnamed_addr constant [38 x i8] c"arithmetic-coded JPEG not supported\0D\0A\00", align 1
@.str.7 = private unnamed_addr constant [34 x i8] c"only baseline JPEG is supported\0D\0A\00", align 1
@.str.8 = private unnamed_addr constant [19 x i8] c"unsupported JPEG\0D\0A\00", align 1
@bmp = internal global [54 x i8] zeroinitializer, align 1
@.str.9 = private unnamed_addr constant [8 x i8] c"OUT.BMP\00", align 1
@.str.10 = private unnamed_addr constant [23 x i8] c"cannot write OUT.BMP\0D\0A\00", align 1
@coef = internal global [64 x i32] zeroinitializer, align 2
@zigzag = internal constant [64 x i8] c"\00\01\08\10\09\02\03\0A\11\18 \19\12\0B\04\05\0C\13\1A!(0)\22\1B\14\0D\06\07\0E\15\1C#*1892+$\1D\16\0F\17\1E%,3:;4-&\1F'.5<=6/7>?", align 1
@line = internal global [771 x i8] zeroinitializer, align 1
@adler_b = internal global i32 0, align 2
@adler_a = internal global i32 1, align 2
@inpos = internal global i32 0, align 2
@inlen = internal global i32 0, align 2
@inbuf = internal global [512 x i8] zeroinitializer, align 1
@bitcnt = internal global i32 0, align 2
@bitbuf = internal global i32 0, align 2
@cosine = internal constant [8 x [8 x i32]] [[8 x i32] [i32 362, i32 502, i32 473, i32 426, i32 362, i32 284, i32 196, i32 100], [8 x i32] [i32 362, i32 426, i32 196, i32 -100, i32 -362, i32 -502, i32 -473, i32 -284], [8 x i32] [i32 362, i32 284, i32 -196, i32 -502, i32 -362, i32 100, i32 473, i32 426], [8 x i32] [i32 362, i32 100, i32 -473, i32 -284, i32 362, i32 426, i32 -196, i32 -502], [8 x i32] [i32 362, i32 -100, i32 -473, i32 284, i32 362, i32 -426, i32 -196, i32 502], [8 x i32] [i32 362, i32 -284, i32 -196, i32 502, i32 -362, i32 -100, i32 473, i32 -426], [8 x i32] [i32 362, i32 -426, i32 196, i32 100, i32 -362, i32 502, i32 -473, i32 284], [8 x i32] [i32 362, i32 -502, i32 473, i32 -426, i32 362, i32 -284, i32 196, i32 -100]], align 2
@planes = internal global [3 x [16 x [256 x i8]]] zeroinitializer, align 1

; Function Attrs: nounwind
define dso_local i32 @bench_jpeg_fixed() #0 {
  %1 = alloca i32, align 2
  %2 = alloca [3 x i32], align 2
  %3 = alloca [3 x i32], align 2
  %4 = alloca [3 x i32], align 2
  %5 = alloca [3 x i32], align 2
  %6 = alloca [3 x i32], align 2
  %7 = alloca [3 x i32], align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  %10 = alloca i32, align 2
  %11 = alloca i32, align 2
  %12 = alloca i32, align 2
  %13 = alloca i32, align 2
  %14 = alloca i32, align 2
  %15 = alloca i32, align 2
  %16 = alloca i32, align 2
  %17 = alloca i32, align 2
  %18 = alloca i32, align 2
  %19 = alloca i32, align 2
  %20 = alloca i32, align 2
  %21 = alloca i32, align 2
  %22 = alloca i32, align 2
  %23 = alloca i32, align 2
  %24 = alloca i32, align 2
  %25 = alloca i32, align 2
  %26 = alloca i32, align 2
  %27 = alloca i32, align 2
  %28 = alloca i32, align 2
  %29 = alloca i32, align 2
  %30 = alloca i32, align 2
  %31 = alloca i32, align 2
  %32 = alloca i32, align 2
  %33 = alloca i32, align 4
  %34 = alloca [17 x i32], align 2
  %35 = alloca i32, align 2
  %36 = alloca i32, align 2
  %37 = alloca i32, align 2
  %38 = alloca i32, align 2
  %39 = alloca i32, align 2
  %40 = alloca i32, align 2
  %41 = alloca i32, align 2
  %42 = alloca i32, align 2
  %43 = alloca i32, align 2
  %44 = alloca i32, align 2
  %45 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %2) #3
  call void @llvm.lifetime.start.p0(ptr %3) #3
  call void @llvm.lifetime.start.p0(ptr %4) #3
  call void @llvm.lifetime.start.p0(ptr %5) #3
  call void @llvm.lifetime.start.p0(ptr %6) #3
  call void @llvm.lifetime.start.p0(ptr %7) #3
  call void @llvm.lifetime.start.p0(ptr %8) #3
  store i32 0, ptr %8, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 0, ptr %9, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %10) #3
  store i32 0, ptr %10, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %11) #3
  store i32 0, ptr %11, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %12) #3
  store i32 0, ptr %12, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %13) #3
  call void @llvm.lifetime.start.p0(ptr %14) #3
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
  call void @llvm.lifetime.start.p0(ptr %28) #3
  call void @llvm.lifetime.start.p0(ptr %29) #3
  call void @llvm.lifetime.start.p0(ptr %30) #3
  call void @llvm.lifetime.start.p0(ptr %31) #3
  call void @llvm.lifetime.start.p0(ptr %32) #3
  %46 = call i32 @llrm_os_open(ptr noundef @.str, i8 noundef zeroext 0) #4
  store i32 %46, ptr @infile, align 2, !tbaa !8
  %47 = load i32, ptr @infile, align 2, !tbaa !8
  %48 = icmp slt i32 %47, 0
  br i1 %48, label %49, label %50

49:                                               ; preds = %0
  call void @message(ptr noundef @.str.1) #4
  store i32 0, ptr %1, align 2
  store i32 1, ptr %33, align 4
  br label %686

50:                                               ; preds = %0
  %51 = call i32 @next() #4
  %52 = icmp ne i32 %51, 255
  br i1 %52, label %56, label %53

53:                                               ; preds = %50
  %54 = call i32 @next() #4
  %55 = icmp ne i32 %54, 216
  br i1 %55, label %56, label %57

56:                                               ; preds = %53, %50
  call void @message(ptr noundef @.str.2) #4
  store i32 0, ptr %1, align 2
  store i32 1, ptr %33, align 4
  br label %686

57:                                               ; preds = %53
  br label %58

58:                                               ; preds = %354, %57
  %59 = load i32, ptr %11, align 2, !tbaa !8
  %60 = icmp ne i32 %59, 0
  br i1 %60, label %65, label %61

61:                                               ; preds = %58
  %62 = load i32, ptr @bad, align 2, !tbaa !8
  %63 = icmp ne i32 %62, 0
  %64 = xor i1 %63, true
  br label %65

65:                                               ; preds = %61, %58
  %66 = phi i1 [ false, %58 ], [ %64, %61 ]
  br i1 %66, label %67, label %355

67:                                               ; preds = %65
  br label %68

68:                                               ; preds = %77, %67
  %69 = call i32 @next() #4
  %70 = icmp ne i32 %69, 255
  br i1 %70, label %71, label %75

71:                                               ; preds = %68
  %72 = load i32, ptr @bad, align 2, !tbaa !8
  %73 = icmp ne i32 %72, 0
  %74 = xor i1 %73, true
  br label %75

75:                                               ; preds = %71, %68
  %76 = phi i1 [ false, %68 ], [ %74, %71 ]
  br i1 %76, label %77, label %78

77:                                               ; preds = %75
  br label %68, !llvm.loop !9

78:                                               ; preds = %75
  br label %79

79:                                               ; preds = %82, %78
  %80 = call i32 @next() #4
  store i32 %80, ptr %13, align 2, !tbaa !8
  %81 = icmp eq i32 %80, 255
  br i1 %81, label %82, label %83

82:                                               ; preds = %79
  br label %79, !llvm.loop !11

83:                                               ; preds = %79
  %84 = call i32 @next16() #4
  %85 = sub nsw i32 %84, 2
  store i32 %85, ptr %14, align 2, !tbaa !8
  %86 = load i32, ptr %13, align 2, !tbaa !8
  %87 = icmp eq i32 %86, 219
  br i1 %87, label %88, label %116

88:                                               ; preds = %83
  br label %89

89:                                               ; preds = %112, %88
  %90 = load i32, ptr %14, align 2, !tbaa !8
  %91 = icmp sgt i32 %90, 0
  br i1 %91, label %92, label %115

92:                                               ; preds = %89
  %93 = call i32 @next() #4
  store i32 %93, ptr %18, align 2, !tbaa !8
  %94 = load i32, ptr %18, align 2, !tbaa !8
  %95 = ashr i32 %94, 4
  %96 = icmp ne i32 %95, 0
  br i1 %96, label %97, label %98

97:                                               ; preds = %92
  call void @message(ptr noundef @.str.3) #4
  store i32 0, ptr %1, align 2
  store i32 1, ptr %33, align 4
  br label %686

98:                                               ; preds = %92
  store i32 0, ptr %16, align 2, !tbaa !8
  br label %99

99:                                               ; preds = %109, %98
  %100 = load i32, ptr %16, align 2, !tbaa !8
  %101 = icmp slt i32 %100, 64
  br i1 %101, label %102, label %112

102:                                              ; preds = %99
  %103 = call i32 @next() #4
  %104 = load i32, ptr %18, align 2, !tbaa !8
  %105 = and i32 %104, 3
  %106 = getelementptr inbounds [4 x [64 x i32]], ptr @quant, i32 0, i32 %105
  %107 = load i32, ptr %16, align 2, !tbaa !8
  %108 = getelementptr inbounds [64 x i32], ptr %106, i32 0, i32 %107
  store i32 %103, ptr %108, align 2, !tbaa !8
  br label %109

109:                                              ; preds = %102
  %110 = load i32, ptr %16, align 2, !tbaa !8
  %111 = add nsw i32 %110, 1
  store i32 %111, ptr %16, align 2, !tbaa !8
  br label %99, !llvm.loop !12

112:                                              ; preds = %99
  %113 = load i32, ptr %14, align 2, !tbaa !8
  %114 = sub nsw i32 %113, 65
  store i32 %114, ptr %14, align 2, !tbaa !8
  br label %89, !llvm.loop !13

115:                                              ; preds = %89
  br label %347

116:                                              ; preds = %83
  %117 = load i32, ptr %13, align 2, !tbaa !8
  %118 = icmp eq i32 %117, 196
  br i1 %118, label %119, label %214

119:                                              ; preds = %116
  br label %120

120:                                              ; preds = %208, %119
  %121 = load i32, ptr %14, align 2, !tbaa !8
  %122 = icmp sgt i32 %121, 0
  br i1 %122, label %123, label %127

123:                                              ; preds = %120
  %124 = load i32, ptr @bad, align 2, !tbaa !8
  %125 = icmp ne i32 %124, 0
  %126 = xor i1 %125, true
  br label %127

127:                                              ; preds = %123, %120
  %128 = phi i1 [ false, %120 ], [ %126, %123 ]
  br i1 %128, label %129, label %213

129:                                              ; preds = %127
  call void @llvm.lifetime.start.p0(ptr %34) #3
  call void @llvm.lifetime.start.p0(ptr %35) #3
  store i32 0, ptr %35, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %36) #3
  store i32 0, ptr %36, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %37) #3
  store i32 0, ptr %37, align 2, !tbaa !8
  %130 = call i32 @next() #4
  store i32 %130, ptr %18, align 2, !tbaa !8
  %131 = load i32, ptr %18, align 2, !tbaa !8
  %132 = ashr i32 %131, 4
  %133 = mul nsw i32 %132, 2
  %134 = load i32, ptr %18, align 2, !tbaa !8
  %135 = and i32 %134, 1
  %136 = add nsw i32 %133, %135
  store i32 %136, ptr %18, align 2, !tbaa !8
  store i32 1, ptr %16, align 2, !tbaa !8
  br label %137

137:                                              ; preds = %146, %129
  %138 = load i32, ptr %16, align 2, !tbaa !8
  %139 = icmp sle i32 %138, 16
  br i1 %139, label %140, label %149

140:                                              ; preds = %137
  %141 = call i32 @next() #4
  %142 = load i32, ptr %16, align 2, !tbaa !8
  %143 = getelementptr inbounds [17 x i32], ptr %34, i32 0, i32 %142
  store i32 %141, ptr %143, align 2, !tbaa !8
  %144 = load i32, ptr %37, align 2, !tbaa !8
  %145 = add nsw i32 %144, %141
  store i32 %145, ptr %37, align 2, !tbaa !8
  br label %146

146:                                              ; preds = %140
  %147 = load i32, ptr %16, align 2, !tbaa !8
  %148 = add nsw i32 %147, 1
  store i32 %148, ptr %16, align 2, !tbaa !8
  br label %137, !llvm.loop !14

149:                                              ; preds = %137
  store i32 1, ptr %16, align 2, !tbaa !8
  br label %150

150:                                              ; preds = %190, %149
  %151 = load i32, ptr %16, align 2, !tbaa !8
  %152 = icmp sle i32 %151, 16
  br i1 %152, label %153, label %193

153:                                              ; preds = %150
  %154 = load i32, ptr %36, align 2, !tbaa !8
  %155 = load i32, ptr %18, align 2, !tbaa !8
  %156 = getelementptr inbounds [4 x [17 x i32]], ptr @valptr, i32 0, i32 %155
  %157 = load i32, ptr %16, align 2, !tbaa !8
  %158 = getelementptr inbounds [17 x i32], ptr %156, i32 0, i32 %157
  store i32 %154, ptr %158, align 2, !tbaa !8
  %159 = load i32, ptr %35, align 2, !tbaa !8
  %160 = load i32, ptr %18, align 2, !tbaa !8
  %161 = getelementptr inbounds [4 x [17 x i32]], ptr @mincode, i32 0, i32 %160
  %162 = load i32, ptr %16, align 2, !tbaa !8
  %163 = getelementptr inbounds [17 x i32], ptr %161, i32 0, i32 %162
  store i32 %159, ptr %163, align 2, !tbaa !8
  %164 = load i32, ptr %16, align 2, !tbaa !8
  %165 = getelementptr inbounds [17 x i32], ptr %34, i32 0, i32 %164
  %166 = load i32, ptr %165, align 2, !tbaa !8
  %167 = load i32, ptr %35, align 2, !tbaa !8
  %168 = add nsw i32 %167, %166
  store i32 %168, ptr %35, align 2, !tbaa !8
  %169 = load i32, ptr %16, align 2, !tbaa !8
  %170 = getelementptr inbounds [17 x i32], ptr %34, i32 0, i32 %169
  %171 = load i32, ptr %170, align 2, !tbaa !8
  %172 = load i32, ptr %36, align 2, !tbaa !8
  %173 = add nsw i32 %172, %171
  store i32 %173, ptr %36, align 2, !tbaa !8
  %174 = load i32, ptr %16, align 2, !tbaa !8
  %175 = getelementptr inbounds [17 x i32], ptr %34, i32 0, i32 %174
  %176 = load i32, ptr %175, align 2, !tbaa !8
  %177 = icmp ne i32 %176, 0
  br i1 %177, label %178, label %181

178:                                              ; preds = %153
  %179 = load i32, ptr %35, align 2, !tbaa !8
  %180 = sub nsw i32 %179, 1
  br label %182

181:                                              ; preds = %153
  br label %182

182:                                              ; preds = %181, %178
  %183 = phi i32 [ %180, %178 ], [ -1, %181 ]
  %184 = load i32, ptr %18, align 2, !tbaa !8
  %185 = getelementptr inbounds [4 x [18 x i32]], ptr @maxcode, i32 0, i32 %184
  %186 = load i32, ptr %16, align 2, !tbaa !8
  %187 = getelementptr inbounds [18 x i32], ptr %185, i32 0, i32 %186
  store i32 %183, ptr %187, align 2, !tbaa !8
  %188 = load i32, ptr %35, align 2, !tbaa !8
  %189 = mul nsw i32 %188, 2
  store i32 %189, ptr %35, align 2, !tbaa !8
  br label %190

190:                                              ; preds = %182
  %191 = load i32, ptr %16, align 2, !tbaa !8
  %192 = add nsw i32 %191, 1
  store i32 %192, ptr %16, align 2, !tbaa !8
  br label %150, !llvm.loop !15

193:                                              ; preds = %150
  store i32 0, ptr %16, align 2, !tbaa !8
  br label %194

194:                                              ; preds = %205, %193
  %195 = load i32, ptr %16, align 2, !tbaa !8
  %196 = load i32, ptr %37, align 2, !tbaa !8
  %197 = icmp slt i32 %195, %196
  br i1 %197, label %198, label %208

198:                                              ; preds = %194
  %199 = call i32 @next() #4
  %200 = trunc i32 %199 to i8
  %201 = load i32, ptr %18, align 2, !tbaa !8
  %202 = getelementptr inbounds [4 x [256 x i8]], ptr @huffval, i32 0, i32 %201
  %203 = load i32, ptr %16, align 2, !tbaa !8
  %204 = getelementptr inbounds [256 x i8], ptr %202, i32 0, i32 %203
  store i8 %200, ptr %204, align 1, !tbaa !16
  br label %205

205:                                              ; preds = %198
  %206 = load i32, ptr %16, align 2, !tbaa !8
  %207 = add nsw i32 %206, 1
  store i32 %207, ptr %16, align 2, !tbaa !8
  br label %194, !llvm.loop !17

208:                                              ; preds = %194
  %209 = load i32, ptr %37, align 2, !tbaa !8
  %210 = add nsw i32 17, %209
  %211 = load i32, ptr %14, align 2, !tbaa !8
  %212 = sub nsw i32 %211, %210
  store i32 %212, ptr %14, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %37) #3
  call void @llvm.lifetime.end.p0(ptr %36) #3
  call void @llvm.lifetime.end.p0(ptr %35) #3
  call void @llvm.lifetime.end.p0(ptr %34) #3
  br label %120, !llvm.loop !18

213:                                              ; preds = %127
  br label %346

214:                                              ; preds = %116
  %215 = load i32, ptr %13, align 2, !tbaa !8
  %216 = icmp eq i32 %215, 192
  br i1 %216, label %217, label %260

217:                                              ; preds = %214
  %218 = call i32 @next() #4
  %219 = icmp ne i32 %218, 8
  br i1 %219, label %220, label %221

220:                                              ; preds = %217
  call void @message(ptr noundef @.str.4) #4
  store i32 0, ptr %1, align 2
  store i32 1, ptr %33, align 4
  br label %686

221:                                              ; preds = %217
  %222 = call i32 @next16() #4
  store i32 %222, ptr %9, align 2, !tbaa !8
  %223 = call i32 @next16() #4
  store i32 %223, ptr %8, align 2, !tbaa !8
  %224 = call i32 @next() #4
  store i32 %224, ptr %12, align 2, !tbaa !8
  store i32 0, ptr %15, align 2, !tbaa !8
  br label %225

225:                                              ; preds = %245, %221
  %226 = load i32, ptr %15, align 2, !tbaa !8
  %227 = load i32, ptr %12, align 2, !tbaa !8
  %228 = icmp slt i32 %226, %227
  br i1 %228, label %229, label %232

229:                                              ; preds = %225
  %230 = load i32, ptr %15, align 2, !tbaa !8
  %231 = icmp slt i32 %230, 3
  br label %232

232:                                              ; preds = %229, %225
  %233 = phi i1 [ false, %225 ], [ %231, %229 ]
  br i1 %233, label %234, label %248

234:                                              ; preds = %232
  %235 = call i32 @next() #4
  %236 = load i32, ptr %15, align 2, !tbaa !8
  %237 = getelementptr inbounds [3 x i32], ptr %2, i32 0, i32 %236
  store i32 %235, ptr %237, align 2, !tbaa !8
  %238 = call i32 @next() #4
  %239 = load i32, ptr %15, align 2, !tbaa !8
  %240 = getelementptr inbounds [3 x i32], ptr %3, i32 0, i32 %239
  store i32 %238, ptr %240, align 2, !tbaa !8
  %241 = call i32 @next() #4
  %242 = and i32 %241, 3
  %243 = load i32, ptr %15, align 2, !tbaa !8
  %244 = getelementptr inbounds [3 x i32], ptr %4, i32 0, i32 %243
  store i32 %242, ptr %244, align 2, !tbaa !8
  br label %245

245:                                              ; preds = %234
  %246 = load i32, ptr %15, align 2, !tbaa !8
  %247 = add nsw i32 %246, 1
  store i32 %247, ptr %15, align 2, !tbaa !8
  br label %225, !llvm.loop !19

248:                                              ; preds = %232
  %249 = load i32, ptr %12, align 2, !tbaa !8
  %250 = icmp slt i32 %249, 3
  br i1 %250, label %251, label %253

251:                                              ; preds = %248
  %252 = load i32, ptr %12, align 2, !tbaa !8
  br label %254

253:                                              ; preds = %248
  br label %254

254:                                              ; preds = %253, %251
  %255 = phi i32 [ %252, %251 ], [ 3, %253 ]
  %256 = mul nsw i32 3, %255
  %257 = add nsw i32 6, %256
  %258 = load i32, ptr %14, align 2, !tbaa !8
  %259 = sub nsw i32 %258, %257
  store i32 %259, ptr %14, align 2, !tbaa !8
  br label %345

260:                                              ; preds = %214
  %261 = load i32, ptr %13, align 2, !tbaa !8
  %262 = icmp eq i32 %261, 194
  br i1 %262, label %263, label %264

263:                                              ; preds = %260
  call void @message(ptr noundef @.str.5) #4
  store i32 0, ptr %1, align 2
  store i32 1, ptr %33, align 4
  br label %686

264:                                              ; preds = %260
  %265 = load i32, ptr %13, align 2, !tbaa !8
  %266 = icmp sge i32 %265, 201
  br i1 %266, label %267, label %274

267:                                              ; preds = %264
  %268 = load i32, ptr %13, align 2, !tbaa !8
  %269 = icmp sle i32 %268, 207
  br i1 %269, label %270, label %274

270:                                              ; preds = %267
  %271 = load i32, ptr %13, align 2, !tbaa !8
  %272 = icmp ne i32 %271, 204
  br i1 %272, label %273, label %274

273:                                              ; preds = %270
  call void @message(ptr noundef @.str.6) #4
  store i32 0, ptr %1, align 2
  store i32 1, ptr %33, align 4
  br label %686

274:                                              ; preds = %270, %267, %264
  %275 = load i32, ptr %13, align 2, !tbaa !8
  %276 = icmp sge i32 %275, 193
  br i1 %276, label %277, label %290

277:                                              ; preds = %274
  %278 = load i32, ptr %13, align 2, !tbaa !8
  %279 = icmp sle i32 %278, 207
  br i1 %279, label %280, label %290

280:                                              ; preds = %277
  %281 = load i32, ptr %13, align 2, !tbaa !8
  %282 = icmp ne i32 %281, 196
  br i1 %282, label %283, label %290

283:                                              ; preds = %280
  %284 = load i32, ptr %13, align 2, !tbaa !8
  %285 = icmp ne i32 %284, 200
  br i1 %285, label %286, label %290

286:                                              ; preds = %283
  %287 = load i32, ptr %13, align 2, !tbaa !8
  %288 = icmp ne i32 %287, 204
  br i1 %288, label %289, label %290

289:                                              ; preds = %286
  call void @message(ptr noundef @.str.7) #4
  store i32 0, ptr %1, align 2
  store i32 1, ptr %33, align 4
  br label %686

290:                                              ; preds = %286, %283, %280, %277, %274
  %291 = load i32, ptr %13, align 2, !tbaa !8
  %292 = icmp eq i32 %291, 221
  br i1 %292, label %293, label %297

293:                                              ; preds = %290
  %294 = call i32 @next16() #4
  store i32 %294, ptr %10, align 2, !tbaa !8
  %295 = load i32, ptr %14, align 2, !tbaa !8
  %296 = sub nsw i32 %295, 2
  store i32 %296, ptr %14, align 2, !tbaa !8
  br label %341

297:                                              ; preds = %290
  %298 = load i32, ptr %13, align 2, !tbaa !8
  %299 = icmp eq i32 %298, 218
  br i1 %299, label %300, label %340

300:                                              ; preds = %297
  call void @llvm.lifetime.start.p0(ptr %38) #3
  %301 = call i32 @next() #4
  store i32 %301, ptr %38, align 2, !tbaa !8
  store i32 0, ptr %15, align 2, !tbaa !8
  br label %302

302:                                              ; preds = %333, %300
  %303 = load i32, ptr %15, align 2, !tbaa !8
  %304 = load i32, ptr %38, align 2, !tbaa !8
  %305 = icmp slt i32 %303, %304
  br i1 %305, label %306, label %336

306:                                              ; preds = %302
  call void @llvm.lifetime.start.p0(ptr %39) #3
  %307 = call i32 @next() #4
  store i32 %307, ptr %39, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %40) #3
  %308 = call i32 @next() #4
  store i32 %308, ptr %40, align 2, !tbaa !8
  store i32 0, ptr %17, align 2, !tbaa !8
  br label %309

309:                                              ; preds = %329, %306
  %310 = load i32, ptr %17, align 2, !tbaa !8
  %311 = icmp slt i32 %310, 3
  br i1 %311, label %312, label %332

312:                                              ; preds = %309
  %313 = load i32, ptr %17, align 2, !tbaa !8
  %314 = getelementptr inbounds [3 x i32], ptr %2, i32 0, i32 %313
  %315 = load i32, ptr %314, align 2, !tbaa !8
  %316 = load i32, ptr %39, align 2, !tbaa !8
  %317 = icmp eq i32 %315, %316
  br i1 %317, label %318, label %328

318:                                              ; preds = %312
  %319 = load i32, ptr %40, align 2, !tbaa !8
  %320 = ashr i32 %319, 4
  %321 = load i32, ptr %17, align 2, !tbaa !8
  %322 = getelementptr inbounds [3 x i32], ptr %5, i32 0, i32 %321
  store i32 %320, ptr %322, align 2, !tbaa !8
  %323 = load i32, ptr %40, align 2, !tbaa !8
  %324 = and i32 %323, 1
  %325 = add nsw i32 2, %324
  %326 = load i32, ptr %17, align 2, !tbaa !8
  %327 = getelementptr inbounds [3 x i32], ptr %6, i32 0, i32 %326
  store i32 %325, ptr %327, align 2, !tbaa !8
  br label %328

328:                                              ; preds = %318, %312
  br label %329

329:                                              ; preds = %328
  %330 = load i32, ptr %17, align 2, !tbaa !8
  %331 = add nsw i32 %330, 1
  store i32 %331, ptr %17, align 2, !tbaa !8
  br label %309, !llvm.loop !20

332:                                              ; preds = %309
  call void @llvm.lifetime.end.p0(ptr %40) #3
  call void @llvm.lifetime.end.p0(ptr %39) #3
  br label %333

333:                                              ; preds = %332
  %334 = load i32, ptr %15, align 2, !tbaa !8
  %335 = add nsw i32 %334, 1
  store i32 %335, ptr %15, align 2, !tbaa !8
  br label %302, !llvm.loop !21

336:                                              ; preds = %302
  %337 = call i32 @next() #4
  %338 = call i32 @next() #4
  %339 = call i32 @next() #4
  store i32 1, ptr %11, align 2, !tbaa !8
  store i32 0, ptr %14, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %38) #3
  br label %340

340:                                              ; preds = %336, %297
  br label %341

341:                                              ; preds = %340, %293
  br label %342

342:                                              ; preds = %341
  br label %343

343:                                              ; preds = %342
  br label %344

344:                                              ; preds = %343
  br label %345

345:                                              ; preds = %344, %254
  br label %346

346:                                              ; preds = %345, %213
  br label %347

347:                                              ; preds = %346, %115
  br label %348

348:                                              ; preds = %352, %347
  %349 = load i32, ptr %14, align 2, !tbaa !8
  %350 = add nsw i32 %349, -1
  store i32 %350, ptr %14, align 2, !tbaa !8
  %351 = icmp sgt i32 %349, 0
  br i1 %351, label %352, label %354

352:                                              ; preds = %348
  %353 = call i32 @next() #4
  br label %348, !llvm.loop !22

354:                                              ; preds = %348
  br label %58, !llvm.loop !23

355:                                              ; preds = %65
  %356 = getelementptr inbounds [3 x i32], ptr %3, i32 0, i32 0
  %357 = load i32, ptr %356, align 2, !tbaa !8
  %358 = ashr i32 %357, 4
  store i32 %358, ptr %19, align 2, !tbaa !8
  %359 = getelementptr inbounds [3 x i32], ptr %3, i32 0, i32 0
  %360 = load i32, ptr %359, align 2, !tbaa !8
  %361 = and i32 %360, 15
  store i32 %361, ptr %20, align 2, !tbaa !8
  %362 = load i32, ptr @bad, align 2, !tbaa !8
  %363 = icmp ne i32 %362, 0
  br i1 %363, label %396, label %364

364:                                              ; preds = %355
  %365 = load i32, ptr %12, align 2, !tbaa !8
  %366 = icmp ne i32 %365, 3
  br i1 %366, label %396, label %367

367:                                              ; preds = %364
  %368 = load i32, ptr %19, align 2, !tbaa !8
  %369 = icmp slt i32 %368, 1
  br i1 %369, label %396, label %370

370:                                              ; preds = %367
  %371 = load i32, ptr %19, align 2, !tbaa !8
  %372 = icmp sgt i32 %371, 2
  br i1 %372, label %396, label %373

373:                                              ; preds = %370
  %374 = load i32, ptr %20, align 2, !tbaa !8
  %375 = icmp slt i32 %374, 1
  br i1 %375, label %396, label %376

376:                                              ; preds = %373
  %377 = load i32, ptr %20, align 2, !tbaa !8
  %378 = icmp sgt i32 %377, 2
  br i1 %378, label %396, label %379

379:                                              ; preds = %376
  %380 = getelementptr inbounds [3 x i32], ptr %3, i32 0, i32 1
  %381 = load i32, ptr %380, align 2, !tbaa !8
  %382 = icmp ne i32 %381, 17
  br i1 %382, label %396, label %383

383:                                              ; preds = %379
  %384 = getelementptr inbounds [3 x i32], ptr %3, i32 0, i32 2
  %385 = load i32, ptr %384, align 2, !tbaa !8
  %386 = icmp ne i32 %385, 17
  br i1 %386, label %396, label %387

387:                                              ; preds = %383
  %388 = load i32, ptr %8, align 2, !tbaa !8
  %389 = icmp slt i32 %388, 1
  br i1 %389, label %396, label %390

390:                                              ; preds = %387
  %391 = load i32, ptr %8, align 2, !tbaa !8
  %392 = icmp sgt i32 %391, 256
  br i1 %392, label %396, label %393

393:                                              ; preds = %390
  %394 = load i32, ptr %9, align 2, !tbaa !8
  %395 = icmp slt i32 %394, 1
  br i1 %395, label %396, label %397

396:                                              ; preds = %393, %390, %387, %383, %379, %376, %373, %370, %367, %364, %355
  call void @message(ptr noundef @.str.8) #4
  store i32 0, ptr %1, align 2
  store i32 1, ptr %33, align 4
  br label %686

397:                                              ; preds = %393
  %398 = load i32, ptr %19, align 2, !tbaa !8
  %399 = mul nsw i32 8, %398
  store i32 %399, ptr %27, align 2, !tbaa !8
  %400 = load i32, ptr %20, align 2, !tbaa !8
  %401 = mul nsw i32 8, %400
  store i32 %401, ptr %28, align 2, !tbaa !8
  %402 = load i32, ptr %8, align 2, !tbaa !8
  %403 = load i32, ptr %27, align 2, !tbaa !8
  %404 = add nsw i32 %402, %403
  %405 = sub nsw i32 %404, 1
  %406 = load i32, ptr %27, align 2, !tbaa !8
  %407 = sdiv i32 %405, %406
  store i32 %407, ptr %21, align 2, !tbaa !8
  %408 = load i32, ptr %9, align 2, !tbaa !8
  %409 = load i32, ptr %28, align 2, !tbaa !8
  %410 = add nsw i32 %408, %409
  %411 = sub nsw i32 %410, 1
  %412 = load i32, ptr %28, align 2, !tbaa !8
  %413 = sdiv i32 %411, %412
  store i32 %413, ptr %22, align 2, !tbaa !8
  %414 = load i32, ptr %8, align 2, !tbaa !8
  %415 = mul nsw i32 %414, 3
  %416 = add nsw i32 %415, 3
  %417 = and i32 %416, -4
  store i32 %417, ptr %29, align 2, !tbaa !8
  %418 = load i32, ptr %29, align 2, !tbaa !8
  %419 = load i32, ptr %9, align 2, !tbaa !8
  %420 = mul i32 %418, %419
  %421 = add i32 54, %420
  store i32 %421, ptr %32, align 2, !tbaa !24
  store i8 66, ptr @bmp, align 1, !tbaa !16
  store i8 77, ptr getelementptr inbounds nuw (i8, ptr @bmp, i32 1), align 1, !tbaa !16
  %422 = load i32, ptr %32, align 2, !tbaa !24
  call void @put32(i32 noundef 2, i32 noundef %422) #4
  call void @put32(i32 noundef 10, i32 noundef 54) #4
  call void @put32(i32 noundef 14, i32 noundef 40) #4
  %423 = load i32, ptr %8, align 2, !tbaa !8
  call void @put32(i32 noundef 18, i32 noundef %423) #4
  %424 = load i32, ptr %9, align 2, !tbaa !8
  %425 = sub nsw i32 0, %424
  call void @put32(i32 noundef 22, i32 noundef %425) #4
  store i8 1, ptr getelementptr inbounds nuw (i8, ptr @bmp, i32 26), align 1, !tbaa !16
  store i8 24, ptr getelementptr inbounds nuw (i8, ptr @bmp, i32 28), align 1, !tbaa !16
  %426 = load i32, ptr %32, align 2, !tbaa !24
  %427 = sub i32 %426, 54
  call void @put32(i32 noundef 34, i32 noundef %427) #4
  call void @put32(i32 noundef 38, i32 noundef 2835) #4
  call void @put32(i32 noundef 42, i32 noundef 2835) #4
  %428 = call i32 @llrm_os_create(ptr noundef @.str.9) #4
  store i32 %428, ptr %31, align 2, !tbaa !8
  %429 = load i32, ptr %31, align 2, !tbaa !8
  %430 = icmp slt i32 %429, 0
  br i1 %430, label %431, label %432

431:                                              ; preds = %397
  call void @message(ptr noundef @.str.10) #4
  store i32 0, ptr %1, align 2
  store i32 1, ptr %33, align 4
  br label %686

432:                                              ; preds = %397
  %433 = load i32, ptr %31, align 2, !tbaa !8
  call void @emit(i32 noundef %433, ptr noundef @bmp, i32 noundef 54) #4
  store i32 0, ptr %15, align 2, !tbaa !8
  br label %434

434:                                              ; preds = %440, %432
  %435 = load i32, ptr %15, align 2, !tbaa !8
  %436 = icmp slt i32 %435, 3
  br i1 %436, label %437, label %443

437:                                              ; preds = %434
  %438 = load i32, ptr %15, align 2, !tbaa !8
  %439 = getelementptr inbounds [3 x i32], ptr %7, i32 0, i32 %438
  store i32 0, ptr %439, align 2, !tbaa !8
  br label %440

440:                                              ; preds = %437
  %441 = load i32, ptr %15, align 2, !tbaa !8
  %442 = add nsw i32 %441, 1
  store i32 %442, ptr %15, align 2, !tbaa !8
  br label %434, !llvm.loop !26

443:                                              ; preds = %434
  %444 = load i32, ptr %10, align 2, !tbaa !8
  store i32 %444, ptr %30, align 2, !tbaa !8
  store i32 0, ptr %24, align 2, !tbaa !8
  br label %445

445:                                              ; preds = %674, %443
  %446 = load i32, ptr %24, align 2, !tbaa !8
  %447 = load i32, ptr %22, align 2, !tbaa !8
  %448 = icmp slt i32 %446, %447
  br i1 %448, label %449, label %453

449:                                              ; preds = %445
  %450 = load i32, ptr @bad, align 2, !tbaa !8
  %451 = icmp ne i32 %450, 0
  %452 = xor i1 %451, true
  br label %453

453:                                              ; preds = %449, %445
  %454 = phi i1 [ false, %445 ], [ %452, %449 ]
  br i1 %454, label %455, label %677

455:                                              ; preds = %453
  store i32 0, ptr %23, align 2, !tbaa !8
  br label %456

456:                                              ; preds = %630, %455
  %457 = load i32, ptr %23, align 2, !tbaa !8
  %458 = load i32, ptr %21, align 2, !tbaa !8
  %459 = icmp slt i32 %457, %458
  br i1 %459, label %460, label %464

460:                                              ; preds = %456
  %461 = load i32, ptr @bad, align 2, !tbaa !8
  %462 = icmp ne i32 %461, 0
  %463 = xor i1 %462, true
  br label %464

464:                                              ; preds = %460, %456
  %465 = phi i1 [ false, %456 ], [ %463, %460 ]
  br i1 %465, label %466, label %633

466:                                              ; preds = %464
  %467 = load i32, ptr %10, align 2, !tbaa !8
  %468 = icmp ne i32 %467, 0
  br i1 %468, label %469, label %484

469:                                              ; preds = %466
  %470 = load i32, ptr %30, align 2, !tbaa !8
  %471 = icmp eq i32 %470, 0
  br i1 %471, label %472, label %484

472:                                              ; preds = %469
  call void @restart() #4
  store i32 0, ptr %15, align 2, !tbaa !8
  br label %473

473:                                              ; preds = %479, %472
  %474 = load i32, ptr %15, align 2, !tbaa !8
  %475 = icmp slt i32 %474, 3
  br i1 %475, label %476, label %482

476:                                              ; preds = %473
  %477 = load i32, ptr %15, align 2, !tbaa !8
  %478 = getelementptr inbounds [3 x i32], ptr %7, i32 0, i32 %477
  store i32 0, ptr %478, align 2, !tbaa !8
  br label %479

479:                                              ; preds = %476
  %480 = load i32, ptr %15, align 2, !tbaa !8
  %481 = add nsw i32 %480, 1
  store i32 %481, ptr %15, align 2, !tbaa !8
  br label %473, !llvm.loop !27

482:                                              ; preds = %473
  %483 = load i32, ptr %10, align 2, !tbaa !8
  store i32 %483, ptr %30, align 2, !tbaa !8
  br label %484

484:                                              ; preds = %482, %469, %466
  %485 = load i32, ptr %30, align 2, !tbaa !8
  %486 = add nsw i32 %485, -1
  store i32 %486, ptr %30, align 2, !tbaa !8
  store i32 0, ptr %17, align 2, !tbaa !8
  br label %487

487:                                              ; preds = %626, %484
  %488 = load i32, ptr %17, align 2, !tbaa !8
  %489 = icmp slt i32 %488, 3
  br i1 %489, label %490, label %629

490:                                              ; preds = %487
  call void @llvm.lifetime.start.p0(ptr %41) #3
  %491 = load i32, ptr %17, align 2, !tbaa !8
  %492 = icmp ne i32 %491, 0
  br i1 %492, label %493, label %494

493:                                              ; preds = %490
  br label %496

494:                                              ; preds = %490
  %495 = load i32, ptr %19, align 2, !tbaa !8
  br label %496

496:                                              ; preds = %494, %493
  %497 = phi i32 [ 1, %493 ], [ %495, %494 ]
  store i32 %497, ptr %41, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %42) #3
  %498 = load i32, ptr %17, align 2, !tbaa !8
  %499 = icmp ne i32 %498, 0
  br i1 %499, label %500, label %501

500:                                              ; preds = %496
  br label %503

501:                                              ; preds = %496
  %502 = load i32, ptr %20, align 2, !tbaa !8
  br label %503

503:                                              ; preds = %501, %500
  %504 = phi i32 [ 1, %500 ], [ %502, %501 ]
  store i32 %504, ptr %42, align 2, !tbaa !8
  store i32 0, ptr %26, align 2, !tbaa !8
  br label %505

505:                                              ; preds = %622, %503
  %506 = load i32, ptr %26, align 2, !tbaa !8
  %507 = load i32, ptr %42, align 2, !tbaa !8
  %508 = icmp slt i32 %506, %507
  br i1 %508, label %509, label %625

509:                                              ; preds = %505
  store i32 0, ptr %25, align 2, !tbaa !8
  br label %510

510:                                              ; preds = %618, %509
  %511 = load i32, ptr %25, align 2, !tbaa !8
  %512 = load i32, ptr %41, align 2, !tbaa !8
  %513 = icmp slt i32 %511, %512
  br i1 %513, label %514, label %621

514:                                              ; preds = %510
  store i32 0, ptr %16, align 2, !tbaa !8
  br label %515

515:                                              ; preds = %521, %514
  %516 = load i32, ptr %16, align 2, !tbaa !8
  %517 = icmp slt i32 %516, 64
  br i1 %517, label %518, label %524

518:                                              ; preds = %515
  %519 = load i32, ptr %16, align 2, !tbaa !8
  %520 = getelementptr inbounds [64 x i32], ptr @coef, i32 0, i32 %519
  store i32 0, ptr %520, align 2, !tbaa !24
  br label %521

521:                                              ; preds = %518
  %522 = load i32, ptr %16, align 2, !tbaa !8
  %523 = add nsw i32 %522, 1
  store i32 %523, ptr %16, align 2, !tbaa !8
  br label %515, !llvm.loop !28

524:                                              ; preds = %515
  %525 = load i32, ptr %17, align 2, !tbaa !8
  %526 = getelementptr inbounds [3 x i32], ptr %5, i32 0, i32 %525
  %527 = load i32, ptr %526, align 2, !tbaa !8
  %528 = call i32 @decode(i32 noundef %527) #4
  store i32 %528, ptr %18, align 2, !tbaa !8
  %529 = load i32, ptr %18, align 2, !tbaa !8
  %530 = icmp ne i32 %529, 0
  br i1 %530, label %531, label %540

531:                                              ; preds = %524
  %532 = load i32, ptr %18, align 2, !tbaa !8
  %533 = call i32 @receive(i32 noundef %532) #4
  %534 = load i32, ptr %18, align 2, !tbaa !8
  %535 = call i32 @extend(i32 noundef %533, i32 noundef %534) #4
  %536 = load i32, ptr %17, align 2, !tbaa !8
  %537 = getelementptr inbounds [3 x i32], ptr %7, i32 0, i32 %536
  %538 = load i32, ptr %537, align 2, !tbaa !8
  %539 = add nsw i32 %538, %535
  store i32 %539, ptr %537, align 2, !tbaa !8
  br label %540

540:                                              ; preds = %531, %524
  %541 = load i32, ptr %17, align 2, !tbaa !8
  %542 = getelementptr inbounds [3 x i32], ptr %7, i32 0, i32 %541
  %543 = load i32, ptr %542, align 2, !tbaa !8
  %544 = load i32, ptr %17, align 2, !tbaa !8
  %545 = getelementptr inbounds [3 x i32], ptr %4, i32 0, i32 %544
  %546 = load i32, ptr %545, align 2, !tbaa !8
  %547 = getelementptr inbounds [4 x [64 x i32]], ptr @quant, i32 0, i32 %546
  %548 = getelementptr inbounds [64 x i32], ptr %547, i32 0, i32 0
  %549 = load i32, ptr %548, align 2, !tbaa !8
  %550 = mul nsw i32 %543, %549
  store i32 %550, ptr @coef, align 2, !tbaa !24
  store i32 1, ptr %16, align 2, !tbaa !8
  br label %551

551:                                              ; preds = %607, %605, %540
  %552 = load i32, ptr %16, align 2, !tbaa !8
  %553 = icmp slt i32 %552, 64
  br i1 %553, label %554, label %558

554:                                              ; preds = %551
  %555 = load i32, ptr @bad, align 2, !tbaa !8
  %556 = icmp ne i32 %555, 0
  %557 = xor i1 %556, true
  br label %558

558:                                              ; preds = %554, %551
  %559 = phi i1 [ false, %551 ], [ %557, %554 ]
  br i1 %559, label %560, label %608

560:                                              ; preds = %558
  call void @llvm.lifetime.start.p0(ptr %43) #3
  %561 = load i32, ptr %17, align 2, !tbaa !8
  %562 = getelementptr inbounds [3 x i32], ptr %6, i32 0, i32 %561
  %563 = load i32, ptr %562, align 2, !tbaa !8
  %564 = call i32 @decode(i32 noundef %563) #4
  store i32 %564, ptr %43, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %44) #3
  %565 = load i32, ptr %43, align 2, !tbaa !8
  %566 = ashr i32 %565, 4
  store i32 %566, ptr %44, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %45) #3
  %567 = load i32, ptr %43, align 2, !tbaa !8
  %568 = and i32 %567, 15
  store i32 %568, ptr %45, align 2, !tbaa !8
  %569 = load i32, ptr %45, align 2, !tbaa !8
  %570 = icmp eq i32 %569, 0
  br i1 %570, label %571, label %578

571:                                              ; preds = %560
  %572 = load i32, ptr %44, align 2, !tbaa !8
  %573 = icmp ne i32 %572, 15
  br i1 %573, label %574, label %575

574:                                              ; preds = %571
  store i32 60, ptr %33, align 4
  br label %605

575:                                              ; preds = %571
  %576 = load i32, ptr %16, align 2, !tbaa !8
  %577 = add nsw i32 %576, 16
  store i32 %577, ptr %16, align 2, !tbaa !8
  store i32 59, ptr %33, align 4
  br label %605, !llvm.loop !29

578:                                              ; preds = %560
  %579 = load i32, ptr %44, align 2, !tbaa !8
  %580 = load i32, ptr %16, align 2, !tbaa !8
  %581 = add nsw i32 %580, %579
  store i32 %581, ptr %16, align 2, !tbaa !8
  %582 = load i32, ptr %16, align 2, !tbaa !8
  %583 = icmp sgt i32 %582, 63
  br i1 %583, label %584, label %585

584:                                              ; preds = %578
  store i32 60, ptr %33, align 4
  br label %605

585:                                              ; preds = %578
  %586 = load i32, ptr %45, align 2, !tbaa !8
  %587 = call i32 @receive(i32 noundef %586) #4
  %588 = load i32, ptr %45, align 2, !tbaa !8
  %589 = call i32 @extend(i32 noundef %587, i32 noundef %588) #4
  %590 = load i32, ptr %17, align 2, !tbaa !8
  %591 = getelementptr inbounds [3 x i32], ptr %4, i32 0, i32 %590
  %592 = load i32, ptr %591, align 2, !tbaa !8
  %593 = getelementptr inbounds [4 x [64 x i32]], ptr @quant, i32 0, i32 %592
  %594 = load i32, ptr %16, align 2, !tbaa !8
  %595 = getelementptr inbounds [64 x i32], ptr %593, i32 0, i32 %594
  %596 = load i32, ptr %595, align 2, !tbaa !8
  %597 = mul nsw i32 %589, %596
  %598 = load i32, ptr %16, align 2, !tbaa !8
  %599 = getelementptr inbounds [64 x i8], ptr @zigzag, i32 0, i32 %598
  %600 = load i8, ptr %599, align 1, !tbaa !16
  %601 = zext i8 %600 to i32
  %602 = getelementptr inbounds nuw [64 x i32], ptr @coef, i32 0, i32 %601
  store i32 %597, ptr %602, align 2, !tbaa !24
  %603 = load i32, ptr %16, align 2, !tbaa !8
  %604 = add nsw i32 %603, 1
  store i32 %604, ptr %16, align 2, !tbaa !8
  store i32 0, ptr %33, align 4
  br label %605

605:                                              ; preds = %585, %584, %575, %574
  call void @llvm.lifetime.end.p0(ptr %45) #3
  call void @llvm.lifetime.end.p0(ptr %44) #3
  call void @llvm.lifetime.end.p0(ptr %43) #3
  %606 = load i32, ptr %33, align 4
  switch i32 %606, label %688 [
    i32 0, label %607
    i32 60, label %608
    i32 59, label %551
  ]

607:                                              ; preds = %605
  br label %551, !llvm.loop !29

608:                                              ; preds = %605, %558
  %609 = load i32, ptr %17, align 2, !tbaa !8
  %610 = load i32, ptr %23, align 2, !tbaa !8
  %611 = load i32, ptr %41, align 2, !tbaa !8
  %612 = mul nsw i32 %610, %611
  %613 = load i32, ptr %25, align 2, !tbaa !8
  %614 = add nsw i32 %612, %613
  %615 = mul nsw i32 %614, 8
  %616 = load i32, ptr %26, align 2, !tbaa !8
  %617 = mul nsw i32 %616, 8
  call void @idct(i32 noundef %609, i32 noundef %615, i32 noundef %617) #4
  br label %618

618:                                              ; preds = %608
  %619 = load i32, ptr %25, align 2, !tbaa !8
  %620 = add nsw i32 %619, 1
  store i32 %620, ptr %25, align 2, !tbaa !8
  br label %510, !llvm.loop !30

621:                                              ; preds = %510
  br label %622

622:                                              ; preds = %621
  %623 = load i32, ptr %26, align 2, !tbaa !8
  %624 = add nsw i32 %623, 1
  store i32 %624, ptr %26, align 2, !tbaa !8
  br label %505, !llvm.loop !31

625:                                              ; preds = %505
  call void @llvm.lifetime.end.p0(ptr %42) #3
  call void @llvm.lifetime.end.p0(ptr %41) #3
  br label %626

626:                                              ; preds = %625
  %627 = load i32, ptr %17, align 2, !tbaa !8
  %628 = add nsw i32 %627, 1
  store i32 %628, ptr %17, align 2, !tbaa !8
  br label %487, !llvm.loop !32

629:                                              ; preds = %487
  br label %630

630:                                              ; preds = %629
  %631 = load i32, ptr %23, align 2, !tbaa !8
  %632 = add nsw i32 %631, 1
  store i32 %632, ptr %23, align 2, !tbaa !8
  br label %456, !llvm.loop !33

633:                                              ; preds = %464
  store i32 0, ptr %15, align 2, !tbaa !8
  br label %634

634:                                              ; preds = %670, %633
  %635 = load i32, ptr %15, align 2, !tbaa !8
  %636 = load i32, ptr %28, align 2, !tbaa !8
  %637 = icmp slt i32 %635, %636
  br i1 %637, label %638, label %646

638:                                              ; preds = %634
  %639 = load i32, ptr %24, align 2, !tbaa !8
  %640 = load i32, ptr %28, align 2, !tbaa !8
  %641 = mul nsw i32 %639, %640
  %642 = load i32, ptr %15, align 2, !tbaa !8
  %643 = add nsw i32 %641, %642
  %644 = load i32, ptr %9, align 2, !tbaa !8
  %645 = icmp slt i32 %643, %644
  br label %646

646:                                              ; preds = %638, %634
  %647 = phi i1 [ false, %634 ], [ %645, %638 ]
  br i1 %647, label %648, label %673

648:                                              ; preds = %646
  %649 = load i32, ptr %15, align 2, !tbaa !8
  %650 = load i32, ptr %8, align 2, !tbaa !8
  %651 = load i32, ptr %19, align 2, !tbaa !8
  %652 = sub nsw i32 %651, 1
  %653 = load i32, ptr %20, align 2, !tbaa !8
  %654 = sub nsw i32 %653, 1
  call void @convert(i32 noundef %649, i32 noundef %650, i32 noundef %652, i32 noundef %654) #4
  %655 = load i32, ptr %8, align 2, !tbaa !8
  %656 = mul nsw i32 %655, 3
  store i32 %656, ptr %16, align 2, !tbaa !8
  br label %657

657:                                              ; preds = %664, %648
  %658 = load i32, ptr %16, align 2, !tbaa !8
  %659 = load i32, ptr %29, align 2, !tbaa !8
  %660 = icmp slt i32 %658, %659
  br i1 %660, label %661, label %667

661:                                              ; preds = %657
  %662 = load i32, ptr %16, align 2, !tbaa !8
  %663 = getelementptr inbounds [771 x i8], ptr @line, i32 0, i32 %662
  store i8 0, ptr %663, align 1, !tbaa !16
  br label %664

664:                                              ; preds = %661
  %665 = load i32, ptr %16, align 2, !tbaa !8
  %666 = add nsw i32 %665, 1
  store i32 %666, ptr %16, align 2, !tbaa !8
  br label %657, !llvm.loop !34

667:                                              ; preds = %657
  %668 = load i32, ptr %31, align 2, !tbaa !8
  %669 = load i32, ptr %29, align 2, !tbaa !8
  call void @emit(i32 noundef %668, ptr noundef @line, i32 noundef %669) #4
  br label %670

670:                                              ; preds = %667
  %671 = load i32, ptr %15, align 2, !tbaa !8
  %672 = add nsw i32 %671, 1
  store i32 %672, ptr %15, align 2, !tbaa !8
  br label %634, !llvm.loop !35

673:                                              ; preds = %646
  br label %674

674:                                              ; preds = %673
  %675 = load i32, ptr %24, align 2, !tbaa !8
  %676 = add nsw i32 %675, 1
  store i32 %676, ptr %24, align 2, !tbaa !8
  br label %445, !llvm.loop !36

677:                                              ; preds = %453
  %678 = load i32, ptr %31, align 2, !tbaa !8
  %679 = call i32 @llrm_os_close(i32 noundef %678) #4
  %680 = load i32, ptr @infile, align 2, !tbaa !8
  %681 = call i32 @llrm_os_close(i32 noundef %680) #4
  %682 = load i32, ptr @adler_b, align 2, !tbaa !24
  %683 = shl i32 %682, 16
  %684 = load i32, ptr @adler_a, align 2, !tbaa !24
  %685 = or i32 %683, %684
  store i32 %685, ptr %1, align 2
  store i32 1, ptr %33, align 4
  br label %686

686:                                              ; preds = %677, %431, %396, %289, %273, %263, %220, %97, %56, %49
  call void @llvm.lifetime.end.p0(ptr %32) #3
  call void @llvm.lifetime.end.p0(ptr %31) #3
  call void @llvm.lifetime.end.p0(ptr %30) #3
  call void @llvm.lifetime.end.p0(ptr %29) #3
  call void @llvm.lifetime.end.p0(ptr %28) #3
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
  %687 = load i32, ptr %1, align 2
  ret i32 %687

688:                                              ; preds = %605
  unreachable
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

declare i32 @llrm_os_open(ptr noundef, i8 noundef zeroext) #2

; Function Attrs: nounwind
define internal void @message(ptr noundef %0) #0 {
  %2 = alloca ptr, align 2
  %3 = alloca i32, align 2
  store ptr %0, ptr %2, align 2, !tbaa !37
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !8
  br label %4

4:                                                ; preds = %10, %1
  %5 = load ptr, ptr %2, align 2, !tbaa !37
  %6 = load i32, ptr %3, align 2, !tbaa !8
  %7 = getelementptr inbounds i8, ptr %5, i32 %6
  %8 = load i8, ptr %7, align 1, !tbaa !16
  %9 = icmp ne i8 %8, 0
  br i1 %9, label %10, label %13

10:                                               ; preds = %4
  %11 = load i32, ptr %3, align 2, !tbaa !8
  %12 = add nsw i32 %11, 1
  store i32 %12, ptr %3, align 2, !tbaa !8
  br label %4, !llvm.loop !40

13:                                               ; preds = %4
  %14 = load ptr, ptr %2, align 2, !tbaa !37
  %15 = load i32, ptr %3, align 2, !tbaa !8
  %16 = call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef %14, i32 noundef %15) #4
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret void
}

; Function Attrs: nounwind
define internal i32 @next() #0 {
  %1 = alloca i32, align 2
  %2 = load i32, ptr @inpos, align 2, !tbaa !8
  %3 = load i32, ptr @inlen, align 2, !tbaa !8
  %4 = icmp eq i32 %2, %3
  br i1 %4, label %5, label %12

5:                                                ; preds = %0
  %6 = load i32, ptr @infile, align 2, !tbaa !8
  %7 = call i32 @llrm_os_read(i32 noundef %6, ptr noundef @inbuf, i32 noundef 512) #4
  store i32 %7, ptr @inlen, align 2, !tbaa !8
  store i32 0, ptr @inpos, align 2, !tbaa !8
  %8 = load i32, ptr @inlen, align 2, !tbaa !8
  %9 = icmp sle i32 %8, 0
  br i1 %9, label %10, label %11

10:                                               ; preds = %5
  store i32 0, ptr @inlen, align 2, !tbaa !8
  store i32 1, ptr @bad, align 2, !tbaa !8
  store i32 0, ptr %1, align 2
  br label %18

11:                                               ; preds = %5
  br label %12

12:                                               ; preds = %11, %0
  %13 = load i32, ptr @inpos, align 2, !tbaa !8
  %14 = add nsw i32 %13, 1
  store i32 %14, ptr @inpos, align 2, !tbaa !8
  %15 = getelementptr inbounds [512 x i8], ptr @inbuf, i32 0, i32 %13
  %16 = load i8, ptr %15, align 1, !tbaa !16
  %17 = zext i8 %16 to i32
  store i32 %17, ptr %1, align 2
  br label %18

18:                                               ; preds = %12, %10
  %19 = load i32, ptr %1, align 2
  ret i32 %19
}

; Function Attrs: nounwind
define internal i32 @next16() #0 {
  %1 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  %2 = call i32 @next() #4
  store i32 %2, ptr %1, align 2, !tbaa !8
  %3 = load i32, ptr %1, align 2, !tbaa !8
  %4 = mul nsw i32 %3, 256
  %5 = call i32 @next() #4
  %6 = add nsw i32 %4, %5
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret i32 %6
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: nounwind
define internal void @put32(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !24
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 0, ptr %5, align 2, !tbaa !8
  br label %6

6:                                                ; preds = %19, %2
  %7 = load i32, ptr %5, align 2, !tbaa !8
  %8 = icmp slt i32 %7, 4
  br i1 %8, label %9, label %22

9:                                                ; preds = %6
  %10 = load i32, ptr %4, align 2, !tbaa !24
  %11 = load i32, ptr %5, align 2, !tbaa !8
  %12 = mul nsw i32 8, %11
  %13 = lshr i32 %10, %12
  %14 = trunc i32 %13 to i8
  %15 = load i32, ptr %3, align 2, !tbaa !8
  %16 = load i32, ptr %5, align 2, !tbaa !8
  %17 = add nsw i32 %15, %16
  %18 = getelementptr inbounds [54 x i8], ptr @bmp, i32 0, i32 %17
  store i8 %14, ptr %18, align 1, !tbaa !16
  br label %19

19:                                               ; preds = %9
  %20 = load i32, ptr %5, align 2, !tbaa !8
  %21 = add nsw i32 %20, 1
  store i32 %21, ptr %5, align 2, !tbaa !8
  br label %6, !llvm.loop !41

22:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(ptr %5) #3
  ret void
}

declare i32 @llrm_os_create(ptr noundef) #2

; Function Attrs: nounwind
define internal void @emit(i32 noundef %0, ptr noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 2
  %5 = alloca ptr, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !8
  store ptr %1, ptr %5, align 2, !tbaa !37
  store i32 %2, ptr %6, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %7) #3
  %8 = load i32, ptr %4, align 2, !tbaa !8
  %9 = load ptr, ptr %5, align 2, !tbaa !37
  %10 = load i32, ptr %6, align 2, !tbaa !8
  %11 = call i32 @llrm_os_write_file(i32 noundef %8, ptr noundef %9, i32 noundef %10) #4
  store i32 0, ptr %7, align 2, !tbaa !8
  br label %12

12:                                               ; preds = %39, %3
  %13 = load i32, ptr %7, align 2, !tbaa !8
  %14 = load i32, ptr %6, align 2, !tbaa !8
  %15 = icmp slt i32 %13, %14
  br i1 %15, label %16, label %42

16:                                               ; preds = %12
  %17 = load ptr, ptr %5, align 2, !tbaa !37
  %18 = load i32, ptr %7, align 2, !tbaa !8
  %19 = getelementptr inbounds i8, ptr %17, i32 %18
  %20 = load i8, ptr %19, align 1, !tbaa !16
  %21 = zext i8 %20 to i32
  %22 = load i32, ptr @adler_a, align 2, !tbaa !24
  %23 = add i32 %22, %21
  store i32 %23, ptr @adler_a, align 2, !tbaa !24
  %24 = load i32, ptr @adler_a, align 2, !tbaa !24
  %25 = icmp uge i32 %24, 65521
  br i1 %25, label %26, label %29

26:                                               ; preds = %16
  %27 = load i32, ptr @adler_a, align 2, !tbaa !24
  %28 = sub i32 %27, 65521
  store i32 %28, ptr @adler_a, align 2, !tbaa !24
  br label %29

29:                                               ; preds = %26, %16
  %30 = load i32, ptr @adler_a, align 2, !tbaa !24
  %31 = load i32, ptr @adler_b, align 2, !tbaa !24
  %32 = add i32 %31, %30
  store i32 %32, ptr @adler_b, align 2, !tbaa !24
  %33 = load i32, ptr @adler_b, align 2, !tbaa !24
  %34 = icmp uge i32 %33, 65521
  br i1 %34, label %35, label %38

35:                                               ; preds = %29
  %36 = load i32, ptr @adler_b, align 2, !tbaa !24
  %37 = sub i32 %36, 65521
  store i32 %37, ptr @adler_b, align 2, !tbaa !24
  br label %38

38:                                               ; preds = %35, %29
  br label %39

39:                                               ; preds = %38
  %40 = load i32, ptr %7, align 2, !tbaa !8
  %41 = add nsw i32 %40, 1
  store i32 %41, ptr %7, align 2, !tbaa !8
  br label %12, !llvm.loop !42

42:                                               ; preds = %12
  call void @llvm.lifetime.end.p0(ptr %7) #3
  ret void
}

; Function Attrs: nounwind
define internal void @restart() #0 {
  %1 = alloca i32, align 2
  call void @llvm.lifetime.start.p0(ptr %1) #3
  store i32 0, ptr @bitcnt, align 2, !tbaa !8
  br label %2

2:                                                ; preds = %26, %0
  br label %3

3:                                                ; preds = %12, %2
  %4 = call i32 @next() #4
  %5 = icmp ne i32 %4, 255
  br i1 %5, label %6, label %10

6:                                                ; preds = %3
  %7 = load i32, ptr @bad, align 2, !tbaa !8
  %8 = icmp ne i32 %7, 0
  %9 = xor i1 %8, true
  br label %10

10:                                               ; preds = %6, %3
  %11 = phi i1 [ false, %3 ], [ %9, %6 ]
  br i1 %11, label %12, label %13

12:                                               ; preds = %10
  br label %3, !llvm.loop !43

13:                                               ; preds = %10
  %14 = call i32 @next() #4
  store i32 %14, ptr %1, align 2, !tbaa !8
  br label %15

15:                                               ; preds = %13
  %16 = load i32, ptr @bad, align 2, !tbaa !8
  %17 = icmp ne i32 %16, 0
  br i1 %17, label %26, label %18

18:                                               ; preds = %15
  %19 = load i32, ptr %1, align 2, !tbaa !8
  %20 = icmp slt i32 %19, 208
  br i1 %20, label %24, label %21

21:                                               ; preds = %18
  %22 = load i32, ptr %1, align 2, !tbaa !8
  %23 = icmp sgt i32 %22, 215
  br label %24

24:                                               ; preds = %21, %18
  %25 = phi i1 [ true, %18 ], [ %23, %21 ]
  br label %26

26:                                               ; preds = %24, %15
  %27 = phi i1 [ false, %15 ], [ %25, %24 ]
  br i1 %27, label %2, label %28, !llvm.loop !44

28:                                               ; preds = %26
  call void @llvm.lifetime.end.p0(ptr %1) #3
  ret void
}

; Function Attrs: nounwind
define internal i32 @decode(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 4
  store i32 %0, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %4) #3
  %7 = call i32 @getbit() #4
  store i32 %7, ptr %4, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %5) #3
  store i32 1, ptr %5, align 2, !tbaa !8
  br label %8

8:                                                ; preds = %25, %1
  %9 = load i32, ptr %4, align 2, !tbaa !8
  %10 = load i32, ptr %3, align 2, !tbaa !8
  %11 = getelementptr inbounds [4 x [18 x i32]], ptr @maxcode, i32 0, i32 %10
  %12 = load i32, ptr %5, align 2, !tbaa !8
  %13 = getelementptr inbounds [18 x i32], ptr %11, i32 0, i32 %12
  %14 = load i32, ptr %13, align 2, !tbaa !8
  %15 = icmp sgt i32 %9, %14
  br i1 %15, label %16, label %26

16:                                               ; preds = %8
  %17 = load i32, ptr %4, align 2, !tbaa !8
  %18 = mul nsw i32 %17, 2
  %19 = call i32 @getbit() #4
  %20 = add nsw i32 %18, %19
  store i32 %20, ptr %4, align 2, !tbaa !8
  %21 = load i32, ptr %5, align 2, !tbaa !8
  %22 = add nsw i32 %21, 1
  store i32 %22, ptr %5, align 2, !tbaa !8
  %23 = icmp sgt i32 %22, 16
  br i1 %23, label %24, label %25

24:                                               ; preds = %16
  store i32 1, ptr @bad, align 2, !tbaa !8
  store i32 0, ptr %2, align 2
  store i32 1, ptr %6, align 4
  br label %45

25:                                               ; preds = %16
  br label %8, !llvm.loop !45

26:                                               ; preds = %8
  %27 = load i32, ptr %3, align 2, !tbaa !8
  %28 = getelementptr inbounds [4 x [256 x i8]], ptr @huffval, i32 0, i32 %27
  %29 = load i32, ptr %3, align 2, !tbaa !8
  %30 = getelementptr inbounds [4 x [17 x i32]], ptr @valptr, i32 0, i32 %29
  %31 = load i32, ptr %5, align 2, !tbaa !8
  %32 = getelementptr inbounds [17 x i32], ptr %30, i32 0, i32 %31
  %33 = load i32, ptr %32, align 2, !tbaa !8
  %34 = load i32, ptr %4, align 2, !tbaa !8
  %35 = add nsw i32 %33, %34
  %36 = load i32, ptr %3, align 2, !tbaa !8
  %37 = getelementptr inbounds [4 x [17 x i32]], ptr @mincode, i32 0, i32 %36
  %38 = load i32, ptr %5, align 2, !tbaa !8
  %39 = getelementptr inbounds [17 x i32], ptr %37, i32 0, i32 %38
  %40 = load i32, ptr %39, align 2, !tbaa !8
  %41 = sub nsw i32 %35, %40
  %42 = getelementptr inbounds [256 x i8], ptr %28, i32 0, i32 %41
  %43 = load i8, ptr %42, align 1, !tbaa !16
  %44 = zext i8 %43 to i32
  store i32 %44, ptr %2, align 2
  store i32 1, ptr %6, align 4
  br label %45

45:                                               ; preds = %26, %24
  call void @llvm.lifetime.end.p0(ptr %5) #3
  call void @llvm.lifetime.end.p0(ptr %4) #3
  %46 = load i32, ptr %2, align 2
  ret i32 %46
}

; Function Attrs: nounwind
define internal i32 @extend(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !8
  store i32 %1, ptr %4, align 2, !tbaa !8
  %5 = load i32, ptr %3, align 2, !tbaa !8
  %6 = load i32, ptr %4, align 2, !tbaa !8
  %7 = sub nsw i32 %6, 1
  %8 = shl i32 1, %7
  %9 = icmp sge i32 %5, %8
  br i1 %9, label %10, label %12

10:                                               ; preds = %2
  %11 = load i32, ptr %3, align 2, !tbaa !8
  br label %18

12:                                               ; preds = %2
  %13 = load i32, ptr %3, align 2, !tbaa !8
  %14 = load i32, ptr %4, align 2, !tbaa !8
  %15 = shl i32 1, %14
  %16 = sub nsw i32 %13, %15
  %17 = add nsw i32 %16, 1
  br label %18

18:                                               ; preds = %12, %10
  %19 = phi i32 [ %11, %10 ], [ %17, %12 ]
  ret i32 %19
}

; Function Attrs: nounwind
define internal i32 @receive(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  %3 = alloca i32, align 2
  store i32 %0, ptr %2, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %3) #3
  store i32 0, ptr %3, align 2, !tbaa !8
  br label %4

4:                                                ; preds = %8, %1
  %5 = load i32, ptr %2, align 2, !tbaa !8
  %6 = add nsw i32 %5, -1
  store i32 %6, ptr %2, align 2, !tbaa !8
  %7 = icmp ne i32 %5, 0
  br i1 %7, label %8, label %13

8:                                                ; preds = %4
  %9 = load i32, ptr %3, align 2, !tbaa !8
  %10 = mul nsw i32 %9, 2
  %11 = call i32 @getbit() #4
  %12 = add nsw i32 %10, %11
  store i32 %12, ptr %3, align 2, !tbaa !8
  br label %4, !llvm.loop !46

13:                                               ; preds = %4
  %14 = load i32, ptr %3, align 2, !tbaa !8
  call void @llvm.lifetime.end.p0(ptr %3) #3
  ret i32 %14
}

; Function Attrs: nounwind
define internal void @idct(i32 noundef %0, i32 noundef %1, i32 noundef %2) #0 {
  %4 = alloca i32, align 2
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca [64 x i32], align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  %10 = alloca i32, align 2
  %11 = alloca i32, align 2
  %12 = alloca i32, align 2
  %13 = alloca i32, align 2
  store i32 %0, ptr %4, align 2, !tbaa !8
  store i32 %1, ptr %5, align 2, !tbaa !8
  store i32 %2, ptr %6, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %7) #3
  call void @llvm.lifetime.start.p0(ptr %8) #3
  call void @llvm.lifetime.start.p0(ptr %9) #3
  call void @llvm.lifetime.start.p0(ptr %10) #3
  call void @llvm.lifetime.start.p0(ptr %11) #3
  call void @llvm.lifetime.start.p0(ptr %12) #3
  call void @llvm.lifetime.start.p0(ptr %13) #3
  store i32 0, ptr %9, align 2, !tbaa !8
  br label %14

14:                                               ; preds = %55, %3
  %15 = load i32, ptr %9, align 2, !tbaa !8
  %16 = icmp slt i32 %15, 8
  br i1 %16, label %17, label %58

17:                                               ; preds = %14
  store i32 0, ptr %10, align 2, !tbaa !8
  br label %18

18:                                               ; preds = %51, %17
  %19 = load i32, ptr %10, align 2, !tbaa !8
  %20 = icmp slt i32 %19, 8
  br i1 %20, label %21, label %54

21:                                               ; preds = %18
  store i32 0, ptr %8, align 2, !tbaa !24
  store i32 0, ptr %11, align 2, !tbaa !8
  br label %22

22:                                               ; preds = %41, %21
  %23 = load i32, ptr %11, align 2, !tbaa !8
  %24 = icmp slt i32 %23, 8
  br i1 %24, label %25, label %44

25:                                               ; preds = %22
  %26 = load i32, ptr %10, align 2, !tbaa !8
  %27 = getelementptr inbounds [8 x [8 x i32]], ptr @cosine, i32 0, i32 %26
  %28 = load i32, ptr %11, align 2, !tbaa !8
  %29 = getelementptr inbounds [8 x i32], ptr %27, i32 0, i32 %28
  %30 = load i32, ptr %29, align 2, !tbaa !8
  %31 = load i32, ptr %9, align 2, !tbaa !8
  %32 = mul nsw i32 %31, 8
  %33 = load i32, ptr %11, align 2, !tbaa !8
  %34 = add nsw i32 %32, %33
  %35 = getelementptr inbounds [64 x i32], ptr @coef, i32 0, i32 %34
  %36 = load i32, ptr %35, align 2, !tbaa !24
  %37 = mul nsw i32 %36, 1024
  %38 = call i32 @tmul(i32 noundef %30, i32 noundef %37) #4
  %39 = load i32, ptr %8, align 2, !tbaa !24
  %40 = add nsw i32 %39, %38
  store i32 %40, ptr %8, align 2, !tbaa !24
  br label %41

41:                                               ; preds = %25
  %42 = load i32, ptr %11, align 2, !tbaa !8
  %43 = add nsw i32 %42, 1
  store i32 %43, ptr %11, align 2, !tbaa !8
  br label %22, !llvm.loop !47

44:                                               ; preds = %22
  %45 = load i32, ptr %8, align 2, !tbaa !24
  %46 = load i32, ptr %9, align 2, !tbaa !8
  %47 = mul nsw i32 %46, 8
  %48 = load i32, ptr %10, align 2, !tbaa !8
  %49 = add nsw i32 %47, %48
  %50 = getelementptr inbounds [64 x i32], ptr %7, i32 0, i32 %49
  store i32 %45, ptr %50, align 2, !tbaa !24
  br label %51

51:                                               ; preds = %44
  %52 = load i32, ptr %10, align 2, !tbaa !8
  %53 = add nsw i32 %52, 1
  store i32 %53, ptr %10, align 2, !tbaa !8
  br label %18, !llvm.loop !48

54:                                               ; preds = %18
  br label %55

55:                                               ; preds = %54
  %56 = load i32, ptr %9, align 2, !tbaa !8
  %57 = add nsw i32 %56, 1
  store i32 %57, ptr %9, align 2, !tbaa !8
  br label %14, !llvm.loop !49

58:                                               ; preds = %14
  store i32 0, ptr %12, align 2, !tbaa !8
  br label %59

59:                                               ; preds = %119, %58
  %60 = load i32, ptr %12, align 2, !tbaa !8
  %61 = icmp slt i32 %60, 8
  br i1 %61, label %62, label %122

62:                                               ; preds = %59
  store i32 0, ptr %10, align 2, !tbaa !8
  br label %63

63:                                               ; preds = %115, %62
  %64 = load i32, ptr %10, align 2, !tbaa !8
  %65 = icmp slt i32 %64, 8
  br i1 %65, label %66, label %118

66:                                               ; preds = %63
  store i32 131584, ptr %8, align 2, !tbaa !24
  store i32 0, ptr %9, align 2, !tbaa !8
  br label %67

67:                                               ; preds = %85, %66
  %68 = load i32, ptr %9, align 2, !tbaa !8
  %69 = icmp slt i32 %68, 8
  br i1 %69, label %70, label %88

70:                                               ; preds = %67
  %71 = load i32, ptr %12, align 2, !tbaa !8
  %72 = getelementptr inbounds [8 x [8 x i32]], ptr @cosine, i32 0, i32 %71
  %73 = load i32, ptr %9, align 2, !tbaa !8
  %74 = getelementptr inbounds [8 x i32], ptr %72, i32 0, i32 %73
  %75 = load i32, ptr %74, align 2, !tbaa !8
  %76 = load i32, ptr %9, align 2, !tbaa !8
  %77 = mul nsw i32 %76, 8
  %78 = load i32, ptr %10, align 2, !tbaa !8
  %79 = add nsw i32 %77, %78
  %80 = getelementptr inbounds [64 x i32], ptr %7, i32 0, i32 %79
  %81 = load i32, ptr %80, align 2, !tbaa !24
  %82 = call i32 @tmul(i32 noundef %75, i32 noundef %81) #4
  %83 = load i32, ptr %8, align 2, !tbaa !24
  %84 = add nsw i32 %83, %82
  store i32 %84, ptr %8, align 2, !tbaa !24
  br label %85

85:                                               ; preds = %70
  %86 = load i32, ptr %9, align 2, !tbaa !8
  %87 = add nsw i32 %86, 1
  store i32 %87, ptr %9, align 2, !tbaa !8
  br label %67, !llvm.loop !50

88:                                               ; preds = %67
  %89 = load i32, ptr %8, align 2, !tbaa !24
  %90 = sdiv i32 %89, 1024
  store i32 %90, ptr %13, align 2, !tbaa !8
  %91 = load i32, ptr %13, align 2, !tbaa !8
  %92 = icmp slt i32 %91, 0
  br i1 %92, label %93, label %94

93:                                               ; preds = %88
  br label %102

94:                                               ; preds = %88
  %95 = load i32, ptr %13, align 2, !tbaa !8
  %96 = icmp sgt i32 %95, 255
  br i1 %96, label %97, label %98

97:                                               ; preds = %94
  br label %100

98:                                               ; preds = %94
  %99 = load i32, ptr %13, align 2, !tbaa !8
  br label %100

100:                                              ; preds = %98, %97
  %101 = phi i32 [ 255, %97 ], [ %99, %98 ]
  br label %102

102:                                              ; preds = %100, %93
  %103 = phi i32 [ 0, %93 ], [ %101, %100 ]
  %104 = trunc i32 %103 to i8
  %105 = load i32, ptr %4, align 2, !tbaa !8
  %106 = getelementptr inbounds [3 x [16 x [256 x i8]]], ptr @planes, i32 0, i32 %105
  %107 = load i32, ptr %6, align 2, !tbaa !8
  %108 = load i32, ptr %12, align 2, !tbaa !8
  %109 = add nsw i32 %107, %108
  %110 = getelementptr inbounds [16 x [256 x i8]], ptr %106, i32 0, i32 %109
  %111 = load i32, ptr %5, align 2, !tbaa !8
  %112 = load i32, ptr %10, align 2, !tbaa !8
  %113 = add nsw i32 %111, %112
  %114 = getelementptr inbounds [256 x i8], ptr %110, i32 0, i32 %113
  store i8 %104, ptr %114, align 1, !tbaa !16
  br label %115

115:                                              ; preds = %102
  %116 = load i32, ptr %10, align 2, !tbaa !8
  %117 = add nsw i32 %116, 1
  store i32 %117, ptr %10, align 2, !tbaa !8
  br label %63, !llvm.loop !51

118:                                              ; preds = %63
  br label %119

119:                                              ; preds = %118
  %120 = load i32, ptr %12, align 2, !tbaa !8
  %121 = add nsw i32 %120, 1
  store i32 %121, ptr %12, align 2, !tbaa !8
  br label %59, !llvm.loop !52

122:                                              ; preds = %59
  call void @llvm.lifetime.end.p0(ptr %13) #3
  call void @llvm.lifetime.end.p0(ptr %12) #3
  call void @llvm.lifetime.end.p0(ptr %11) #3
  call void @llvm.lifetime.end.p0(ptr %10) #3
  call void @llvm.lifetime.end.p0(ptr %9) #3
  call void @llvm.lifetime.end.p0(ptr %8) #3
  call void @llvm.lifetime.end.p0(ptr %7) #3
  ret void
}

; Function Attrs: nounwind
define internal void @convert(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3) #0 {
  %5 = alloca i32, align 2
  %6 = alloca i32, align 2
  %7 = alloca i32, align 2
  %8 = alloca i32, align 2
  %9 = alloca i32, align 2
  %10 = alloca i32, align 2
  %11 = alloca i32, align 2
  %12 = alloca i32, align 2
  store i32 %0, ptr %5, align 2, !tbaa !8
  store i32 %1, ptr %6, align 2, !tbaa !8
  store i32 %2, ptr %7, align 2, !tbaa !8
  store i32 %3, ptr %8, align 2, !tbaa !8
  call void @llvm.lifetime.start.p0(ptr %9) #3
  store i32 0, ptr %9, align 2, !tbaa !8
  br label %13

13:                                               ; preds = %87, %4
  %14 = load i32, ptr %9, align 2, !tbaa !8
  %15 = load i32, ptr %6, align 2, !tbaa !8
  %16 = icmp slt i32 %14, %15
  br i1 %16, label %17, label %90

17:                                               ; preds = %13
  call void @llvm.lifetime.start.p0(ptr %10) #3
  %18 = load i32, ptr %5, align 2, !tbaa !8
  %19 = getelementptr inbounds [16 x [256 x i8]], ptr @planes, i32 0, i32 %18
  %20 = load i32, ptr %9, align 2, !tbaa !8
  %21 = getelementptr inbounds [256 x i8], ptr %19, i32 0, i32 %20
  %22 = load i8, ptr %21, align 1, !tbaa !16
  %23 = zext i8 %22 to i32
  %24 = mul nsw i32 %23, 1024
  store i32 %24, ptr %10, align 2, !tbaa !24
  call void @llvm.lifetime.start.p0(ptr %11) #3
  %25 = load i32, ptr %5, align 2, !tbaa !8
  %26 = load i32, ptr %8, align 2, !tbaa !8
  %27 = ashr i32 %25, %26
  %28 = getelementptr inbounds [16 x [256 x i8]], ptr getelementptr inbounds nuw (i8, ptr @planes, i32 4096), i32 0, i32 %27
  %29 = load i32, ptr %9, align 2, !tbaa !8
  %30 = load i32, ptr %7, align 2, !tbaa !8
  %31 = ashr i32 %29, %30
  %32 = getelementptr inbounds [256 x i8], ptr %28, i32 0, i32 %31
  %33 = load i8, ptr %32, align 1, !tbaa !16
  %34 = zext i8 %33 to i32
  %35 = sub nsw i32 %34, 128
  %36 = mul nsw i32 %35, 1024
  store i32 %36, ptr %11, align 2, !tbaa !24
  call void @llvm.lifetime.start.p0(ptr %12) #3
  %37 = load i32, ptr %5, align 2, !tbaa !8
  %38 = load i32, ptr %8, align 2, !tbaa !8
  %39 = ashr i32 %37, %38
  %40 = getelementptr inbounds [16 x [256 x i8]], ptr getelementptr inbounds nuw (i8, ptr @planes, i32 8192), i32 0, i32 %39
  %41 = load i32, ptr %9, align 2, !tbaa !8
  %42 = load i32, ptr %7, align 2, !tbaa !8
  %43 = ashr i32 %41, %42
  %44 = getelementptr inbounds [256 x i8], ptr %40, i32 0, i32 %43
  %45 = load i8, ptr %44, align 1, !tbaa !16
  %46 = zext i8 %45 to i32
  %47 = sub nsw i32 %46, 128
  %48 = mul nsw i32 %47, 1024
  store i32 %48, ptr %12, align 2, !tbaa !24
  %49 = load i32, ptr %10, align 2, !tbaa !24
  %50 = load i32, ptr %11, align 2, !tbaa !24
  %51 = call i32 @tmul(i32 noundef 1815, i32 noundef %50) #4
  %52 = add nsw i32 %49, %51
  %53 = add nsw i32 %52, 512
  %54 = sdiv i32 %53, 1024
  %55 = call i32 @clamp(i32 noundef %54) #4
  %56 = trunc i32 %55 to i8
  %57 = load i32, ptr %9, align 2, !tbaa !8
  %58 = mul nsw i32 3, %57
  %59 = getelementptr inbounds [771 x i8], ptr @line, i32 0, i32 %58
  store i8 %56, ptr %59, align 1, !tbaa !16
  %60 = load i32, ptr %10, align 2, !tbaa !24
  %61 = load i32, ptr %11, align 2, !tbaa !24
  %62 = call i32 @tmul(i32 noundef 352, i32 noundef %61) #4
  %63 = sub nsw i32 %60, %62
  %64 = load i32, ptr %12, align 2, !tbaa !24
  %65 = call i32 @tmul(i32 noundef 731, i32 noundef %64) #4
  %66 = sub nsw i32 %63, %65
  %67 = add nsw i32 %66, 512
  %68 = sdiv i32 %67, 1024
  %69 = call i32 @clamp(i32 noundef %68) #4
  %70 = trunc i32 %69 to i8
  %71 = load i32, ptr %9, align 2, !tbaa !8
  %72 = mul nsw i32 3, %71
  %73 = add nsw i32 %72, 1
  %74 = getelementptr inbounds [771 x i8], ptr @line, i32 0, i32 %73
  store i8 %70, ptr %74, align 1, !tbaa !16
  %75 = load i32, ptr %10, align 2, !tbaa !24
  %76 = load i32, ptr %12, align 2, !tbaa !24
  %77 = call i32 @tmul(i32 noundef 1436, i32 noundef %76) #4
  %78 = add nsw i32 %75, %77
  %79 = add nsw i32 %78, 512
  %80 = sdiv i32 %79, 1024
  %81 = call i32 @clamp(i32 noundef %80) #4
  %82 = trunc i32 %81 to i8
  %83 = load i32, ptr %9, align 2, !tbaa !8
  %84 = mul nsw i32 3, %83
  %85 = add nsw i32 %84, 2
  %86 = getelementptr inbounds [771 x i8], ptr @line, i32 0, i32 %85
  store i8 %82, ptr %86, align 1, !tbaa !16
  call void @llvm.lifetime.end.p0(ptr %12) #3
  call void @llvm.lifetime.end.p0(ptr %11) #3
  call void @llvm.lifetime.end.p0(ptr %10) #3
  br label %87

87:                                               ; preds = %17
  %88 = load i32, ptr %9, align 2, !tbaa !8
  %89 = add nsw i32 %88, 1
  store i32 %89, ptr %9, align 2, !tbaa !8
  br label %13, !llvm.loop !53

90:                                               ; preds = %13
  call void @llvm.lifetime.end.p0(ptr %9) #3
  ret void
}

declare i32 @llrm_os_close(i32 noundef) #2

; Function Attrs: nounwind
define dso_local i32 @main() #0 {
  %1 = call i32 @bench_jpeg_fixed() #4
  call void @report(i32 noundef %1) #4
  ret i32 0
}

declare void @report(i32 noundef) #2

declare i32 @llrm_os_write_file(i32 noundef, ptr noundef, i32 noundef) #2

declare i32 @llrm_os_read(i32 noundef, ptr noundef, i32 noundef) #2

; Function Attrs: nounwind
define internal i32 @getbit() #0 {
  %1 = load i32, ptr @bitcnt, align 2, !tbaa !8
  %2 = icmp eq i32 %1, 0
  br i1 %2, label %3, label %12

3:                                                ; preds = %0
  %4 = call i32 @next() #4
  store i32 %4, ptr @bitbuf, align 2, !tbaa !8
  %5 = load i32, ptr @bitbuf, align 2, !tbaa !8
  %6 = icmp eq i32 %5, 255
  br i1 %6, label %7, label %11

7:                                                ; preds = %3
  %8 = call i32 @next() #4
  %9 = icmp ne i32 %8, 0
  br i1 %9, label %10, label %11

10:                                               ; preds = %7
  store i32 1, ptr @bad, align 2, !tbaa !8
  br label %11

11:                                               ; preds = %10, %7, %3
  store i32 8, ptr @bitcnt, align 2, !tbaa !8
  br label %12

12:                                               ; preds = %11, %0
  %13 = load i32, ptr @bitcnt, align 2, !tbaa !8
  %14 = add nsw i32 %13, -1
  store i32 %14, ptr @bitcnt, align 2, !tbaa !8
  %15 = load i32, ptr @bitbuf, align 2, !tbaa !8
  %16 = load i32, ptr @bitcnt, align 2, !tbaa !8
  %17 = ashr i32 %15, %16
  %18 = and i32 %17, 1
  ret i32 %18
}

; Function Attrs: nounwind
define internal i32 @tmul(i32 noundef %0, i32 noundef %1) #0 {
  %3 = alloca i32, align 2
  %4 = alloca i32, align 2
  store i32 %0, ptr %3, align 2, !tbaa !24
  store i32 %1, ptr %4, align 2, !tbaa !24
  %5 = load i32, ptr %3, align 2, !tbaa !24
  %6 = load i32, ptr %4, align 2, !tbaa !24
  %7 = mul nsw i32 %5, %6
  %8 = sdiv i32 %7, 1024
  ret i32 %8
}

; Function Attrs: nounwind
define internal i32 @clamp(i32 noundef %0) #0 {
  %2 = alloca i32, align 2
  store i32 %0, ptr %2, align 2, !tbaa !24
  %3 = load i32, ptr %2, align 2, !tbaa !24
  %4 = icmp slt i32 %3, 0
  br i1 %4, label %5, label %6

5:                                                ; preds = %1
  br label %14

6:                                                ; preds = %1
  %7 = load i32, ptr %2, align 2, !tbaa !24
  %8 = icmp sgt i32 %7, 255
  br i1 %8, label %9, label %10

9:                                                ; preds = %6
  br label %12

10:                                               ; preds = %6
  %11 = load i32, ptr %2, align 2, !tbaa !24
  br label %12

12:                                               ; preds = %10, %9
  %13 = phi i32 [ 255, %9 ], [ %11, %10 ]
  br label %14

14:                                               ; preds = %12, %5
  %15 = phi i32 [ 0, %5 ], [ %13, %12 ]
  ret i32 %15
}

attributes #0 = { nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="M68000" }
attributes #3 = { nounwind }
attributes #4 = { nobuiltin "no-builtins" }

!llvm.module.flags = !{!0, !1}
!llvm.ident = !{!2}
!llvm.errno.tbaa = !{!3}

!0 = !{i32 8, !"PIC Level", i32 2}
!1 = !{i32 7, !"PIE Level", i32 2}
!2 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!3 = !{!4, !5, i64 0}
!4 = !{!"__libc_errno", !5, i64 0}
!5 = !{!"int", !6, i64 0}
!6 = !{!"omnipotent char", !7, i64 0}
!7 = !{!"Simple C/C++ TBAA"}
!8 = !{!5, !5, i64 0}
!9 = distinct !{!9, !10}
!10 = !{!"llvm.loop.mustprogress"}
!11 = distinct !{!11, !10}
!12 = distinct !{!12, !10}
!13 = distinct !{!13, !10}
!14 = distinct !{!14, !10}
!15 = distinct !{!15, !10}
!16 = !{!6, !6, i64 0}
!17 = distinct !{!17, !10}
!18 = distinct !{!18, !10}
!19 = distinct !{!19, !10}
!20 = distinct !{!20, !10}
!21 = distinct !{!21, !10}
!22 = distinct !{!22, !10}
!23 = distinct !{!23, !10}
!24 = !{!25, !25, i64 0}
!25 = !{!"long", !6, i64 0}
!26 = distinct !{!26, !10}
!27 = distinct !{!27, !10}
!28 = distinct !{!28, !10}
!29 = distinct !{!29, !10}
!30 = distinct !{!30, !10}
!31 = distinct !{!31, !10}
!32 = distinct !{!32, !10}
!33 = distinct !{!33, !10}
!34 = distinct !{!34, !10}
!35 = distinct !{!35, !10}
!36 = distinct !{!36, !10}
!37 = !{!38, !38, i64 0}
!38 = !{!"p1 omnipotent char", !39, i64 0}
!39 = !{!"any pointer", !6, i64 0}
!40 = distinct !{!40, !10}
!41 = distinct !{!41, !10}
!42 = distinct !{!42, !10}
!43 = distinct !{!43, !10}
!44 = distinct !{!44, !10}
!45 = distinct !{!45, !10}
!46 = distinct !{!46, !10}
!47 = distinct !{!47, !10}
!48 = distinct !{!48, !10}
!49 = distinct !{!49, !10}
!50 = distinct !{!50, !10}
!51 = distinct !{!51, !10}
!52 = distinct !{!52, !10}
!53 = distinct !{!53, !10}
