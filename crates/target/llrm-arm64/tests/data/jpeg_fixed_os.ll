; ModuleID = '/tmp/t1/src_jpeg_fixed.c'
source_filename = "/tmp/t1/src_jpeg_fixed.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

@.str = private unnamed_addr constant [10 x i8] c"PHOTO.JPG\00", align 1
@infile = internal global i32 0, align 4
@.str.1 = private unnamed_addr constant [22 x i8] c"PHOTO.JPG not found\0D\0A\00", align 1
@.str.2 = private unnamed_addr constant [13 x i8] c"not a JPEG\0D\0A\00", align 1
@bad = internal global i32 0, align 4
@.str.3 = private unnamed_addr constant [43 x i8] c"16-bit quantization tables not supported\0D\0A\00", align 1
@quant = internal global [4 x [64 x i32]] zeroinitializer, align 4
@valptr = internal global [4 x [17 x i32]] zeroinitializer, align 4
@mincode = internal global [4 x [17 x i32]] zeroinitializer, align 4
@maxcode = internal global [4 x [18 x i32]] zeroinitializer, align 4
@huffval = internal global [4 x [256 x i8]] zeroinitializer, align 1
@.str.4 = private unnamed_addr constant [35 x i8] c"only 8-bit samples are supported\0D\0A\00", align 1
@.str.5 = private unnamed_addr constant [33 x i8] c"progressive JPEG not supported\0D\0A\00", align 1
@.str.6 = private unnamed_addr constant [38 x i8] c"arithmetic-coded JPEG not supported\0D\0A\00", align 1
@.str.7 = private unnamed_addr constant [34 x i8] c"only baseline JPEG is supported\0D\0A\00", align 1
@.str.8 = private unnamed_addr constant [19 x i8] c"unsupported JPEG\0D\0A\00", align 1
@bmp = internal global [54 x i8] zeroinitializer, align 1
@.str.9 = private unnamed_addr constant [8 x i8] c"OUT.BMP\00", align 1
@.str.10 = private unnamed_addr constant [23 x i8] c"cannot write OUT.BMP\0D\0A\00", align 1
@coef = internal global [64 x i64] zeroinitializer, align 8
@zigzag = internal constant [64 x i8] c"\00\01\08\10\09\02\03\0A\11\18 \19\12\0B\04\05\0C\13\1A!(0)\22\1B\14\0D\06\07\0E\15\1C#*1892+$\1D\16\0F\17\1E%,3:;4-&\1F'.5<=6/7>?", align 1
@line = internal global [771 x i8] zeroinitializer, align 1
@adler_b = internal global i64 0, align 8
@adler_a = internal global i64 1, align 8
@inpos = internal global i32 0, align 4
@inlen = internal global i32 0, align 4
@inbuf = internal global [512 x i8] zeroinitializer, align 1
@bitcnt = internal global i32 0, align 4
@bitbuf = internal global i32 0, align 4
@cosine = internal constant [8 x [8 x i32]] [[8 x i32] [i32 362, i32 502, i32 473, i32 426, i32 362, i32 284, i32 196, i32 100], [8 x i32] [i32 362, i32 426, i32 196, i32 -100, i32 -362, i32 -502, i32 -473, i32 -284], [8 x i32] [i32 362, i32 284, i32 -196, i32 -502, i32 -362, i32 100, i32 473, i32 426], [8 x i32] [i32 362, i32 100, i32 -473, i32 -284, i32 362, i32 426, i32 -196, i32 -502], [8 x i32] [i32 362, i32 -100, i32 -473, i32 284, i32 362, i32 -426, i32 -196, i32 502], [8 x i32] [i32 362, i32 -284, i32 -196, i32 502, i32 -362, i32 -100, i32 473, i32 -426], [8 x i32] [i32 362, i32 -426, i32 196, i32 100, i32 -362, i32 502, i32 -473, i32 284], [8 x i32] [i32 362, i32 -502, i32 473, i32 -426, i32 362, i32 -284, i32 196, i32 -100]], align 4
@planes = internal global [3 x [16 x [256 x i8]]] zeroinitializer, align 1

; Function Attrs: noinline nounwind optsize
define dso_local i64 @bench_jpeg_fixed() #0 {
  %1 = alloca i64, align 8
  %2 = alloca [3 x i32], align 4
  %3 = alloca [3 x i32], align 4
  %4 = alloca [3 x i32], align 4
  %5 = alloca [3 x i32], align 4
  %6 = alloca [3 x i32], align 4
  %7 = alloca [3 x i32], align 4
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
  %29 = alloca i32, align 4
  %30 = alloca i32, align 4
  %31 = alloca i32, align 4
  %32 = alloca i64, align 8
  %33 = alloca i32, align 4
  %34 = alloca [17 x i32], align 4
  %35 = alloca i32, align 4
  %36 = alloca i32, align 4
  %37 = alloca i32, align 4
  %38 = alloca i32, align 4
  %39 = alloca i32, align 4
  %40 = alloca i32, align 4
  %41 = alloca i32, align 4
  %42 = alloca i32, align 4
  %43 = alloca i32, align 4
  %44 = alloca i32, align 4
  %45 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %2) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %6) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %7) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %8) #4
  store i32 0, ptr %8, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %9) #4
  store i32 0, ptr %9, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %10) #4
  store i32 0, ptr %10, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %11) #4
  store i32 0, ptr %11, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %12) #4
  store i32 0, ptr %12, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %13) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %14) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %15) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %16) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %17) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %18) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %19) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %20) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %21) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %22) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %23) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %24) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %25) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %26) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %27) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %28) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %29) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %30) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %31) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %32) #4
  %46 = call i32 @llrm_os_open(ptr noundef @.str, i8 noundef 0) #5
  store i32 %46, ptr @infile, align 4, !tbaa !15
  %47 = load i32, ptr @infile, align 4, !tbaa !15
  %48 = icmp slt i32 %47, 0
  br i1 %48, label %49, label %50

49:                                               ; preds = %0
  call void @message(ptr noundef @.str.1) #5
  store i64 0, ptr %1, align 8
  store i32 1, ptr %33, align 4
  br label %728

50:                                               ; preds = %0
  %51 = call i32 @next() #5
  %52 = icmp ne i32 %51, 255
  br i1 %52, label %56, label %53

53:                                               ; preds = %50
  %54 = call i32 @next() #5
  %55 = icmp ne i32 %54, 216
  br i1 %55, label %56, label %57

56:                                               ; preds = %53, %50
  call void @message(ptr noundef @.str.2) #5
  store i64 0, ptr %1, align 8
  store i32 1, ptr %33, align 4
  br label %728

57:                                               ; preds = %53
  br label %58

58:                                               ; preds = %374, %57
  %59 = load i32, ptr %11, align 4, !tbaa !15
  %60 = icmp ne i32 %59, 0
  br i1 %60, label %65, label %61

61:                                               ; preds = %58
  %62 = load i32, ptr @bad, align 4, !tbaa !15
  %63 = icmp ne i32 %62, 0
  %64 = xor i1 %63, true
  br label %65

65:                                               ; preds = %61, %58
  %66 = phi i1 [ false, %58 ], [ %64, %61 ]
  br i1 %66, label %67, label %375

67:                                               ; preds = %65
  br label %68

68:                                               ; preds = %77, %67
  %69 = call i32 @next() #5
  %70 = icmp ne i32 %69, 255
  br i1 %70, label %71, label %75

71:                                               ; preds = %68
  %72 = load i32, ptr @bad, align 4, !tbaa !15
  %73 = icmp ne i32 %72, 0
  %74 = xor i1 %73, true
  br label %75

75:                                               ; preds = %71, %68
  %76 = phi i1 [ false, %68 ], [ %74, %71 ]
  br i1 %76, label %77, label %78

77:                                               ; preds = %75
  br label %68, !llvm.loop !16

78:                                               ; preds = %75
  br label %79

79:                                               ; preds = %82, %78
  %80 = call i32 @next() #5
  store i32 %80, ptr %13, align 4, !tbaa !15
  %81 = icmp eq i32 %80, 255
  br i1 %81, label %82, label %83

82:                                               ; preds = %79
  br label %79, !llvm.loop !18

83:                                               ; preds = %79
  %84 = call i32 @next16() #5
  %85 = sub nsw i32 %84, 2
  store i32 %85, ptr %14, align 4, !tbaa !15
  %86 = load i32, ptr %13, align 4, !tbaa !15
  %87 = icmp eq i32 %86, 219
  br i1 %87, label %88, label %118

88:                                               ; preds = %83
  br label %89

89:                                               ; preds = %114, %88
  %90 = load i32, ptr %14, align 4, !tbaa !15
  %91 = icmp sgt i32 %90, 0
  br i1 %91, label %92, label %117

92:                                               ; preds = %89
  %93 = call i32 @next() #5
  store i32 %93, ptr %18, align 4, !tbaa !15
  %94 = load i32, ptr %18, align 4, !tbaa !15
  %95 = ashr i32 %94, 4
  %96 = icmp ne i32 %95, 0
  br i1 %96, label %97, label %98

97:                                               ; preds = %92
  call void @message(ptr noundef @.str.3) #5
  store i64 0, ptr %1, align 8
  store i32 1, ptr %33, align 4
  br label %728

98:                                               ; preds = %92
  store i32 0, ptr %16, align 4, !tbaa !15
  br label %99

99:                                               ; preds = %111, %98
  %100 = load i32, ptr %16, align 4, !tbaa !15
  %101 = icmp slt i32 %100, 64
  br i1 %101, label %102, label %114

102:                                              ; preds = %99
  %103 = call i32 @next() #5
  %104 = load i32, ptr %18, align 4, !tbaa !15
  %105 = and i32 %104, 3
  %106 = sext i32 %105 to i64
  %107 = getelementptr inbounds [4 x [64 x i32]], ptr @quant, i64 0, i64 %106
  %108 = load i32, ptr %16, align 4, !tbaa !15
  %109 = sext i32 %108 to i64
  %110 = getelementptr inbounds [64 x i32], ptr %107, i64 0, i64 %109
  store i32 %103, ptr %110, align 4, !tbaa !15
  br label %111

111:                                              ; preds = %102
  %112 = load i32, ptr %16, align 4, !tbaa !15
  %113 = add nsw i32 %112, 1
  store i32 %113, ptr %16, align 4, !tbaa !15
  br label %99, !llvm.loop !19

114:                                              ; preds = %99
  %115 = load i32, ptr %14, align 4, !tbaa !15
  %116 = sub nsw i32 %115, 65
  store i32 %116, ptr %14, align 4, !tbaa !15
  br label %89, !llvm.loop !20

117:                                              ; preds = %89
  br label %367

118:                                              ; preds = %83
  %119 = load i32, ptr %13, align 4, !tbaa !15
  %120 = icmp eq i32 %119, 196
  br i1 %120, label %121, label %228

121:                                              ; preds = %118
  br label %122

122:                                              ; preds = %222, %121
  %123 = load i32, ptr %14, align 4, !tbaa !15
  %124 = icmp sgt i32 %123, 0
  br i1 %124, label %125, label %129

125:                                              ; preds = %122
  %126 = load i32, ptr @bad, align 4, !tbaa !15
  %127 = icmp ne i32 %126, 0
  %128 = xor i1 %127, true
  br label %129

129:                                              ; preds = %125, %122
  %130 = phi i1 [ false, %122 ], [ %128, %125 ]
  br i1 %130, label %131, label %227

131:                                              ; preds = %129
  call void @llvm.lifetime.start.p0(i64 -1, ptr %34) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %35) #4
  store i32 0, ptr %35, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %36) #4
  store i32 0, ptr %36, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %37) #4
  store i32 0, ptr %37, align 4, !tbaa !15
  %132 = call i32 @next() #5
  store i32 %132, ptr %18, align 4, !tbaa !15
  %133 = load i32, ptr %18, align 4, !tbaa !15
  %134 = ashr i32 %133, 4
  %135 = mul nsw i32 %134, 2
  %136 = load i32, ptr %18, align 4, !tbaa !15
  %137 = and i32 %136, 1
  %138 = add nsw i32 %135, %137
  store i32 %138, ptr %18, align 4, !tbaa !15
  store i32 1, ptr %16, align 4, !tbaa !15
  br label %139

139:                                              ; preds = %149, %131
  %140 = load i32, ptr %16, align 4, !tbaa !15
  %141 = icmp sle i32 %140, 16
  br i1 %141, label %142, label %152

142:                                              ; preds = %139
  %143 = call i32 @next() #5
  %144 = load i32, ptr %16, align 4, !tbaa !15
  %145 = sext i32 %144 to i64
  %146 = getelementptr inbounds [17 x i32], ptr %34, i64 0, i64 %145
  store i32 %143, ptr %146, align 4, !tbaa !15
  %147 = load i32, ptr %37, align 4, !tbaa !15
  %148 = add nsw i32 %147, %143
  store i32 %148, ptr %37, align 4, !tbaa !15
  br label %149

149:                                              ; preds = %142
  %150 = load i32, ptr %16, align 4, !tbaa !15
  %151 = add nsw i32 %150, 1
  store i32 %151, ptr %16, align 4, !tbaa !15
  br label %139, !llvm.loop !21

152:                                              ; preds = %139
  store i32 1, ptr %16, align 4, !tbaa !15
  br label %153

153:                                              ; preds = %202, %152
  %154 = load i32, ptr %16, align 4, !tbaa !15
  %155 = icmp sle i32 %154, 16
  br i1 %155, label %156, label %205

156:                                              ; preds = %153
  %157 = load i32, ptr %36, align 4, !tbaa !15
  %158 = load i32, ptr %18, align 4, !tbaa !15
  %159 = sext i32 %158 to i64
  %160 = getelementptr inbounds [4 x [17 x i32]], ptr @valptr, i64 0, i64 %159
  %161 = load i32, ptr %16, align 4, !tbaa !15
  %162 = sext i32 %161 to i64
  %163 = getelementptr inbounds [17 x i32], ptr %160, i64 0, i64 %162
  store i32 %157, ptr %163, align 4, !tbaa !15
  %164 = load i32, ptr %35, align 4, !tbaa !15
  %165 = load i32, ptr %18, align 4, !tbaa !15
  %166 = sext i32 %165 to i64
  %167 = getelementptr inbounds [4 x [17 x i32]], ptr @mincode, i64 0, i64 %166
  %168 = load i32, ptr %16, align 4, !tbaa !15
  %169 = sext i32 %168 to i64
  %170 = getelementptr inbounds [17 x i32], ptr %167, i64 0, i64 %169
  store i32 %164, ptr %170, align 4, !tbaa !15
  %171 = load i32, ptr %16, align 4, !tbaa !15
  %172 = sext i32 %171 to i64
  %173 = getelementptr inbounds [17 x i32], ptr %34, i64 0, i64 %172
  %174 = load i32, ptr %173, align 4, !tbaa !15
  %175 = load i32, ptr %35, align 4, !tbaa !15
  %176 = add nsw i32 %175, %174
  store i32 %176, ptr %35, align 4, !tbaa !15
  %177 = load i32, ptr %16, align 4, !tbaa !15
  %178 = sext i32 %177 to i64
  %179 = getelementptr inbounds [17 x i32], ptr %34, i64 0, i64 %178
  %180 = load i32, ptr %179, align 4, !tbaa !15
  %181 = load i32, ptr %36, align 4, !tbaa !15
  %182 = add nsw i32 %181, %180
  store i32 %182, ptr %36, align 4, !tbaa !15
  %183 = load i32, ptr %16, align 4, !tbaa !15
  %184 = sext i32 %183 to i64
  %185 = getelementptr inbounds [17 x i32], ptr %34, i64 0, i64 %184
  %186 = load i32, ptr %185, align 4, !tbaa !15
  %187 = icmp ne i32 %186, 0
  br i1 %187, label %188, label %191

188:                                              ; preds = %156
  %189 = load i32, ptr %35, align 4, !tbaa !15
  %190 = sub nsw i32 %189, 1
  br label %192

191:                                              ; preds = %156
  br label %192

192:                                              ; preds = %191, %188
  %193 = phi i32 [ %190, %188 ], [ -1, %191 ]
  %194 = load i32, ptr %18, align 4, !tbaa !15
  %195 = sext i32 %194 to i64
  %196 = getelementptr inbounds [4 x [18 x i32]], ptr @maxcode, i64 0, i64 %195
  %197 = load i32, ptr %16, align 4, !tbaa !15
  %198 = sext i32 %197 to i64
  %199 = getelementptr inbounds [18 x i32], ptr %196, i64 0, i64 %198
  store i32 %193, ptr %199, align 4, !tbaa !15
  %200 = load i32, ptr %35, align 4, !tbaa !15
  %201 = mul nsw i32 %200, 2
  store i32 %201, ptr %35, align 4, !tbaa !15
  br label %202

202:                                              ; preds = %192
  %203 = load i32, ptr %16, align 4, !tbaa !15
  %204 = add nsw i32 %203, 1
  store i32 %204, ptr %16, align 4, !tbaa !15
  br label %153, !llvm.loop !22

205:                                              ; preds = %153
  store i32 0, ptr %16, align 4, !tbaa !15
  br label %206

206:                                              ; preds = %219, %205
  %207 = load i32, ptr %16, align 4, !tbaa !15
  %208 = load i32, ptr %37, align 4, !tbaa !15
  %209 = icmp slt i32 %207, %208
  br i1 %209, label %210, label %222

210:                                              ; preds = %206
  %211 = call i32 @next() #5
  %212 = trunc i32 %211 to i8
  %213 = load i32, ptr %18, align 4, !tbaa !15
  %214 = sext i32 %213 to i64
  %215 = getelementptr inbounds [4 x [256 x i8]], ptr @huffval, i64 0, i64 %214
  %216 = load i32, ptr %16, align 4, !tbaa !15
  %217 = sext i32 %216 to i64
  %218 = getelementptr inbounds [256 x i8], ptr %215, i64 0, i64 %217
  store i8 %212, ptr %218, align 1, !tbaa !23
  br label %219

219:                                              ; preds = %210
  %220 = load i32, ptr %16, align 4, !tbaa !15
  %221 = add nsw i32 %220, 1
  store i32 %221, ptr %16, align 4, !tbaa !15
  br label %206, !llvm.loop !24

222:                                              ; preds = %206
  %223 = load i32, ptr %37, align 4, !tbaa !15
  %224 = add nsw i32 17, %223
  %225 = load i32, ptr %14, align 4, !tbaa !15
  %226 = sub nsw i32 %225, %224
  store i32 %226, ptr %14, align 4, !tbaa !15
  call void @llvm.lifetime.end.p0(i64 -1, ptr %37) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %36) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %35) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %34) #4
  br label %122, !llvm.loop !25

227:                                              ; preds = %129
  br label %366

228:                                              ; preds = %118
  %229 = load i32, ptr %13, align 4, !tbaa !15
  %230 = icmp eq i32 %229, 192
  br i1 %230, label %231, label %277

231:                                              ; preds = %228
  %232 = call i32 @next() #5
  %233 = icmp ne i32 %232, 8
  br i1 %233, label %234, label %235

234:                                              ; preds = %231
  call void @message(ptr noundef @.str.4) #5
  store i64 0, ptr %1, align 8
  store i32 1, ptr %33, align 4
  br label %728

235:                                              ; preds = %231
  %236 = call i32 @next16() #5
  store i32 %236, ptr %9, align 4, !tbaa !15
  %237 = call i32 @next16() #5
  store i32 %237, ptr %8, align 4, !tbaa !15
  %238 = call i32 @next() #5
  store i32 %238, ptr %12, align 4, !tbaa !15
  store i32 0, ptr %15, align 4, !tbaa !15
  br label %239

239:                                              ; preds = %262, %235
  %240 = load i32, ptr %15, align 4, !tbaa !15
  %241 = load i32, ptr %12, align 4, !tbaa !15
  %242 = icmp slt i32 %240, %241
  br i1 %242, label %243, label %246

243:                                              ; preds = %239
  %244 = load i32, ptr %15, align 4, !tbaa !15
  %245 = icmp slt i32 %244, 3
  br label %246

246:                                              ; preds = %243, %239
  %247 = phi i1 [ false, %239 ], [ %245, %243 ]
  br i1 %247, label %248, label %265

248:                                              ; preds = %246
  %249 = call i32 @next() #5
  %250 = load i32, ptr %15, align 4, !tbaa !15
  %251 = sext i32 %250 to i64
  %252 = getelementptr inbounds [3 x i32], ptr %2, i64 0, i64 %251
  store i32 %249, ptr %252, align 4, !tbaa !15
  %253 = call i32 @next() #5
  %254 = load i32, ptr %15, align 4, !tbaa !15
  %255 = sext i32 %254 to i64
  %256 = getelementptr inbounds [3 x i32], ptr %3, i64 0, i64 %255
  store i32 %253, ptr %256, align 4, !tbaa !15
  %257 = call i32 @next() #5
  %258 = and i32 %257, 3
  %259 = load i32, ptr %15, align 4, !tbaa !15
  %260 = sext i32 %259 to i64
  %261 = getelementptr inbounds [3 x i32], ptr %4, i64 0, i64 %260
  store i32 %258, ptr %261, align 4, !tbaa !15
  br label %262

262:                                              ; preds = %248
  %263 = load i32, ptr %15, align 4, !tbaa !15
  %264 = add nsw i32 %263, 1
  store i32 %264, ptr %15, align 4, !tbaa !15
  br label %239, !llvm.loop !26

265:                                              ; preds = %246
  %266 = load i32, ptr %12, align 4, !tbaa !15
  %267 = icmp slt i32 %266, 3
  br i1 %267, label %268, label %270

268:                                              ; preds = %265
  %269 = load i32, ptr %12, align 4, !tbaa !15
  br label %271

270:                                              ; preds = %265
  br label %271

271:                                              ; preds = %270, %268
  %272 = phi i32 [ %269, %268 ], [ 3, %270 ]
  %273 = mul nsw i32 3, %272
  %274 = add nsw i32 6, %273
  %275 = load i32, ptr %14, align 4, !tbaa !15
  %276 = sub nsw i32 %275, %274
  store i32 %276, ptr %14, align 4, !tbaa !15
  br label %365

277:                                              ; preds = %228
  %278 = load i32, ptr %13, align 4, !tbaa !15
  %279 = icmp eq i32 %278, 194
  br i1 %279, label %280, label %281

280:                                              ; preds = %277
  call void @message(ptr noundef @.str.5) #5
  store i64 0, ptr %1, align 8
  store i32 1, ptr %33, align 4
  br label %728

281:                                              ; preds = %277
  %282 = load i32, ptr %13, align 4, !tbaa !15
  %283 = icmp sge i32 %282, 201
  br i1 %283, label %284, label %291

284:                                              ; preds = %281
  %285 = load i32, ptr %13, align 4, !tbaa !15
  %286 = icmp sle i32 %285, 207
  br i1 %286, label %287, label %291

287:                                              ; preds = %284
  %288 = load i32, ptr %13, align 4, !tbaa !15
  %289 = icmp ne i32 %288, 204
  br i1 %289, label %290, label %291

290:                                              ; preds = %287
  call void @message(ptr noundef @.str.6) #5
  store i64 0, ptr %1, align 8
  store i32 1, ptr %33, align 4
  br label %728

291:                                              ; preds = %287, %284, %281
  %292 = load i32, ptr %13, align 4, !tbaa !15
  %293 = icmp sge i32 %292, 193
  br i1 %293, label %294, label %307

294:                                              ; preds = %291
  %295 = load i32, ptr %13, align 4, !tbaa !15
  %296 = icmp sle i32 %295, 207
  br i1 %296, label %297, label %307

297:                                              ; preds = %294
  %298 = load i32, ptr %13, align 4, !tbaa !15
  %299 = icmp ne i32 %298, 196
  br i1 %299, label %300, label %307

300:                                              ; preds = %297
  %301 = load i32, ptr %13, align 4, !tbaa !15
  %302 = icmp ne i32 %301, 200
  br i1 %302, label %303, label %307

303:                                              ; preds = %300
  %304 = load i32, ptr %13, align 4, !tbaa !15
  %305 = icmp ne i32 %304, 204
  br i1 %305, label %306, label %307

306:                                              ; preds = %303
  call void @message(ptr noundef @.str.7) #5
  store i64 0, ptr %1, align 8
  store i32 1, ptr %33, align 4
  br label %728

307:                                              ; preds = %303, %300, %297, %294, %291
  %308 = load i32, ptr %13, align 4, !tbaa !15
  %309 = icmp eq i32 %308, 221
  br i1 %309, label %310, label %314

310:                                              ; preds = %307
  %311 = call i32 @next16() #5
  store i32 %311, ptr %10, align 4, !tbaa !15
  %312 = load i32, ptr %14, align 4, !tbaa !15
  %313 = sub nsw i32 %312, 2
  store i32 %313, ptr %14, align 4, !tbaa !15
  br label %361

314:                                              ; preds = %307
  %315 = load i32, ptr %13, align 4, !tbaa !15
  %316 = icmp eq i32 %315, 218
  br i1 %316, label %317, label %360

317:                                              ; preds = %314
  call void @llvm.lifetime.start.p0(i64 -1, ptr %38) #4
  %318 = call i32 @next() #5
  store i32 %318, ptr %38, align 4, !tbaa !15
  store i32 0, ptr %15, align 4, !tbaa !15
  br label %319

319:                                              ; preds = %353, %317
  %320 = load i32, ptr %15, align 4, !tbaa !15
  %321 = load i32, ptr %38, align 4, !tbaa !15
  %322 = icmp slt i32 %320, %321
  br i1 %322, label %323, label %356

323:                                              ; preds = %319
  call void @llvm.lifetime.start.p0(i64 -1, ptr %39) #4
  %324 = call i32 @next() #5
  store i32 %324, ptr %39, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %40) #4
  %325 = call i32 @next() #5
  store i32 %325, ptr %40, align 4, !tbaa !15
  store i32 0, ptr %17, align 4, !tbaa !15
  br label %326

326:                                              ; preds = %349, %323
  %327 = load i32, ptr %17, align 4, !tbaa !15
  %328 = icmp slt i32 %327, 3
  br i1 %328, label %329, label %352

329:                                              ; preds = %326
  %330 = load i32, ptr %17, align 4, !tbaa !15
  %331 = sext i32 %330 to i64
  %332 = getelementptr inbounds [3 x i32], ptr %2, i64 0, i64 %331
  %333 = load i32, ptr %332, align 4, !tbaa !15
  %334 = load i32, ptr %39, align 4, !tbaa !15
  %335 = icmp eq i32 %333, %334
  br i1 %335, label %336, label %348

336:                                              ; preds = %329
  %337 = load i32, ptr %40, align 4, !tbaa !15
  %338 = ashr i32 %337, 4
  %339 = load i32, ptr %17, align 4, !tbaa !15
  %340 = sext i32 %339 to i64
  %341 = getelementptr inbounds [3 x i32], ptr %5, i64 0, i64 %340
  store i32 %338, ptr %341, align 4, !tbaa !15
  %342 = load i32, ptr %40, align 4, !tbaa !15
  %343 = and i32 %342, 1
  %344 = add nsw i32 2, %343
  %345 = load i32, ptr %17, align 4, !tbaa !15
  %346 = sext i32 %345 to i64
  %347 = getelementptr inbounds [3 x i32], ptr %6, i64 0, i64 %346
  store i32 %344, ptr %347, align 4, !tbaa !15
  br label %348

348:                                              ; preds = %336, %329
  br label %349

349:                                              ; preds = %348
  %350 = load i32, ptr %17, align 4, !tbaa !15
  %351 = add nsw i32 %350, 1
  store i32 %351, ptr %17, align 4, !tbaa !15
  br label %326, !llvm.loop !27

352:                                              ; preds = %326
  call void @llvm.lifetime.end.p0(i64 -1, ptr %40) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %39) #4
  br label %353

353:                                              ; preds = %352
  %354 = load i32, ptr %15, align 4, !tbaa !15
  %355 = add nsw i32 %354, 1
  store i32 %355, ptr %15, align 4, !tbaa !15
  br label %319, !llvm.loop !28

356:                                              ; preds = %319
  %357 = call i32 @next() #5
  %358 = call i32 @next() #5
  %359 = call i32 @next() #5
  store i32 1, ptr %11, align 4, !tbaa !15
  store i32 0, ptr %14, align 4, !tbaa !15
  call void @llvm.lifetime.end.p0(i64 -1, ptr %38) #4
  br label %360

360:                                              ; preds = %356, %314
  br label %361

361:                                              ; preds = %360, %310
  br label %362

362:                                              ; preds = %361
  br label %363

363:                                              ; preds = %362
  br label %364

364:                                              ; preds = %363
  br label %365

365:                                              ; preds = %364, %271
  br label %366

366:                                              ; preds = %365, %227
  br label %367

367:                                              ; preds = %366, %117
  br label %368

368:                                              ; preds = %372, %367
  %369 = load i32, ptr %14, align 4, !tbaa !15
  %370 = add nsw i32 %369, -1
  store i32 %370, ptr %14, align 4, !tbaa !15
  %371 = icmp sgt i32 %369, 0
  br i1 %371, label %372, label %374

372:                                              ; preds = %368
  %373 = call i32 @next() #5
  br label %368, !llvm.loop !29

374:                                              ; preds = %368
  br label %58, !llvm.loop !30

375:                                              ; preds = %65
  %376 = getelementptr inbounds [3 x i32], ptr %3, i64 0, i64 0
  %377 = load i32, ptr %376, align 4, !tbaa !15
  %378 = ashr i32 %377, 4
  store i32 %378, ptr %19, align 4, !tbaa !15
  %379 = getelementptr inbounds [3 x i32], ptr %3, i64 0, i64 0
  %380 = load i32, ptr %379, align 4, !tbaa !15
  %381 = and i32 %380, 15
  store i32 %381, ptr %20, align 4, !tbaa !15
  %382 = load i32, ptr @bad, align 4, !tbaa !15
  %383 = icmp ne i32 %382, 0
  br i1 %383, label %416, label %384

384:                                              ; preds = %375
  %385 = load i32, ptr %12, align 4, !tbaa !15
  %386 = icmp ne i32 %385, 3
  br i1 %386, label %416, label %387

387:                                              ; preds = %384
  %388 = load i32, ptr %19, align 4, !tbaa !15
  %389 = icmp slt i32 %388, 1
  br i1 %389, label %416, label %390

390:                                              ; preds = %387
  %391 = load i32, ptr %19, align 4, !tbaa !15
  %392 = icmp sgt i32 %391, 2
  br i1 %392, label %416, label %393

393:                                              ; preds = %390
  %394 = load i32, ptr %20, align 4, !tbaa !15
  %395 = icmp slt i32 %394, 1
  br i1 %395, label %416, label %396

396:                                              ; preds = %393
  %397 = load i32, ptr %20, align 4, !tbaa !15
  %398 = icmp sgt i32 %397, 2
  br i1 %398, label %416, label %399

399:                                              ; preds = %396
  %400 = getelementptr inbounds [3 x i32], ptr %3, i64 0, i64 1
  %401 = load i32, ptr %400, align 4, !tbaa !15
  %402 = icmp ne i32 %401, 17
  br i1 %402, label %416, label %403

403:                                              ; preds = %399
  %404 = getelementptr inbounds [3 x i32], ptr %3, i64 0, i64 2
  %405 = load i32, ptr %404, align 4, !tbaa !15
  %406 = icmp ne i32 %405, 17
  br i1 %406, label %416, label %407

407:                                              ; preds = %403
  %408 = load i32, ptr %8, align 4, !tbaa !15
  %409 = icmp slt i32 %408, 1
  br i1 %409, label %416, label %410

410:                                              ; preds = %407
  %411 = load i32, ptr %8, align 4, !tbaa !15
  %412 = icmp sgt i32 %411, 256
  br i1 %412, label %416, label %413

413:                                              ; preds = %410
  %414 = load i32, ptr %9, align 4, !tbaa !15
  %415 = icmp slt i32 %414, 1
  br i1 %415, label %416, label %417

416:                                              ; preds = %413, %410, %407, %403, %399, %396, %393, %390, %387, %384, %375
  call void @message(ptr noundef @.str.8) #5
  store i64 0, ptr %1, align 8
  store i32 1, ptr %33, align 4
  br label %728

417:                                              ; preds = %413
  %418 = load i32, ptr %19, align 4, !tbaa !15
  %419 = mul nsw i32 8, %418
  store i32 %419, ptr %27, align 4, !tbaa !15
  %420 = load i32, ptr %20, align 4, !tbaa !15
  %421 = mul nsw i32 8, %420
  store i32 %421, ptr %28, align 4, !tbaa !15
  %422 = load i32, ptr %8, align 4, !tbaa !15
  %423 = load i32, ptr %27, align 4, !tbaa !15
  %424 = add nsw i32 %422, %423
  %425 = sub nsw i32 %424, 1
  %426 = load i32, ptr %27, align 4, !tbaa !15
  %427 = sdiv i32 %425, %426
  store i32 %427, ptr %21, align 4, !tbaa !15
  %428 = load i32, ptr %9, align 4, !tbaa !15
  %429 = load i32, ptr %28, align 4, !tbaa !15
  %430 = add nsw i32 %428, %429
  %431 = sub nsw i32 %430, 1
  %432 = load i32, ptr %28, align 4, !tbaa !15
  %433 = sdiv i32 %431, %432
  store i32 %433, ptr %22, align 4, !tbaa !15
  %434 = load i32, ptr %8, align 4, !tbaa !15
  %435 = mul nsw i32 %434, 3
  %436 = add nsw i32 %435, 3
  %437 = and i32 %436, -4
  store i32 %437, ptr %29, align 4, !tbaa !15
  %438 = load i32, ptr %29, align 4, !tbaa !15
  %439 = sext i32 %438 to i64
  %440 = load i32, ptr %9, align 4, !tbaa !15
  %441 = sext i32 %440 to i64
  %442 = mul i64 %439, %441
  %443 = add i64 54, %442
  store i64 %443, ptr %32, align 8, !tbaa !31
  store i8 66, ptr @bmp, align 1, !tbaa !23
  store i8 77, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 1), align 1, !tbaa !23
  %444 = load i64, ptr %32, align 8, !tbaa !31
  call void @put32(i32 noundef 2, i64 noundef %444) #5
  call void @put32(i32 noundef 10, i64 noundef 54) #5
  call void @put32(i32 noundef 14, i64 noundef 40) #5
  %445 = load i32, ptr %8, align 4, !tbaa !15
  %446 = sext i32 %445 to i64
  call void @put32(i32 noundef 18, i64 noundef %446) #5
  %447 = load i32, ptr %9, align 4, !tbaa !15
  %448 = sext i32 %447 to i64
  %449 = sub nsw i64 0, %448
  call void @put32(i32 noundef 22, i64 noundef %449) #5
  store i8 1, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 26), align 1, !tbaa !23
  store i8 24, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 28), align 1, !tbaa !23
  %450 = load i64, ptr %32, align 8, !tbaa !31
  %451 = sub i64 %450, 54
  call void @put32(i32 noundef 34, i64 noundef %451) #5
  call void @put32(i32 noundef 38, i64 noundef 2835) #5
  call void @put32(i32 noundef 42, i64 noundef 2835) #5
  %452 = call i32 @llrm_os_create(ptr noundef @.str.9) #5
  store i32 %452, ptr %31, align 4, !tbaa !15
  %453 = load i32, ptr %31, align 4, !tbaa !15
  %454 = icmp slt i32 %453, 0
  br i1 %454, label %455, label %456

455:                                              ; preds = %417
  call void @message(ptr noundef @.str.10) #5
  store i64 0, ptr %1, align 8
  store i32 1, ptr %33, align 4
  br label %728

456:                                              ; preds = %417
  %457 = load i32, ptr %31, align 4, !tbaa !15
  call void @emit(i32 noundef %457, ptr noundef @bmp, i32 noundef 54) #5
  store i32 0, ptr %15, align 4, !tbaa !15
  br label %458

458:                                              ; preds = %465, %456
  %459 = load i32, ptr %15, align 4, !tbaa !15
  %460 = icmp slt i32 %459, 3
  br i1 %460, label %461, label %468

461:                                              ; preds = %458
  %462 = load i32, ptr %15, align 4, !tbaa !15
  %463 = sext i32 %462 to i64
  %464 = getelementptr inbounds [3 x i32], ptr %7, i64 0, i64 %463
  store i32 0, ptr %464, align 4, !tbaa !15
  br label %465

465:                                              ; preds = %461
  %466 = load i32, ptr %15, align 4, !tbaa !15
  %467 = add nsw i32 %466, 1
  store i32 %467, ptr %15, align 4, !tbaa !15
  br label %458, !llvm.loop !33

468:                                              ; preds = %458
  %469 = load i32, ptr %10, align 4, !tbaa !15
  store i32 %469, ptr %30, align 4, !tbaa !15
  store i32 0, ptr %24, align 4, !tbaa !15
  br label %470

470:                                              ; preds = %716, %468
  %471 = load i32, ptr %24, align 4, !tbaa !15
  %472 = load i32, ptr %22, align 4, !tbaa !15
  %473 = icmp slt i32 %471, %472
  br i1 %473, label %474, label %478

474:                                              ; preds = %470
  %475 = load i32, ptr @bad, align 4, !tbaa !15
  %476 = icmp ne i32 %475, 0
  %477 = xor i1 %476, true
  br label %478

478:                                              ; preds = %474, %470
  %479 = phi i1 [ false, %470 ], [ %477, %474 ]
  br i1 %479, label %480, label %719

480:                                              ; preds = %478
  store i32 0, ptr %23, align 4, !tbaa !15
  br label %481

481:                                              ; preds = %671, %480
  %482 = load i32, ptr %23, align 4, !tbaa !15
  %483 = load i32, ptr %21, align 4, !tbaa !15
  %484 = icmp slt i32 %482, %483
  br i1 %484, label %485, label %489

485:                                              ; preds = %481
  %486 = load i32, ptr @bad, align 4, !tbaa !15
  %487 = icmp ne i32 %486, 0
  %488 = xor i1 %487, true
  br label %489

489:                                              ; preds = %485, %481
  %490 = phi i1 [ false, %481 ], [ %488, %485 ]
  br i1 %490, label %491, label %674

491:                                              ; preds = %489
  %492 = load i32, ptr %10, align 4, !tbaa !15
  %493 = icmp ne i32 %492, 0
  br i1 %493, label %494, label %510

494:                                              ; preds = %491
  %495 = load i32, ptr %30, align 4, !tbaa !15
  %496 = icmp eq i32 %495, 0
  br i1 %496, label %497, label %510

497:                                              ; preds = %494
  call void @restart() #5
  store i32 0, ptr %15, align 4, !tbaa !15
  br label %498

498:                                              ; preds = %505, %497
  %499 = load i32, ptr %15, align 4, !tbaa !15
  %500 = icmp slt i32 %499, 3
  br i1 %500, label %501, label %508

501:                                              ; preds = %498
  %502 = load i32, ptr %15, align 4, !tbaa !15
  %503 = sext i32 %502 to i64
  %504 = getelementptr inbounds [3 x i32], ptr %7, i64 0, i64 %503
  store i32 0, ptr %504, align 4, !tbaa !15
  br label %505

505:                                              ; preds = %501
  %506 = load i32, ptr %15, align 4, !tbaa !15
  %507 = add nsw i32 %506, 1
  store i32 %507, ptr %15, align 4, !tbaa !15
  br label %498, !llvm.loop !34

508:                                              ; preds = %498
  %509 = load i32, ptr %10, align 4, !tbaa !15
  store i32 %509, ptr %30, align 4, !tbaa !15
  br label %510

510:                                              ; preds = %508, %494, %491
  %511 = load i32, ptr %30, align 4, !tbaa !15
  %512 = add nsw i32 %511, -1
  store i32 %512, ptr %30, align 4, !tbaa !15
  store i32 0, ptr %17, align 4, !tbaa !15
  br label %513

513:                                              ; preds = %667, %510
  %514 = load i32, ptr %17, align 4, !tbaa !15
  %515 = icmp slt i32 %514, 3
  br i1 %515, label %516, label %670

516:                                              ; preds = %513
  call void @llvm.lifetime.start.p0(i64 -1, ptr %41) #4
  %517 = load i32, ptr %17, align 4, !tbaa !15
  %518 = icmp ne i32 %517, 0
  br i1 %518, label %519, label %520

519:                                              ; preds = %516
  br label %522

520:                                              ; preds = %516
  %521 = load i32, ptr %19, align 4, !tbaa !15
  br label %522

522:                                              ; preds = %520, %519
  %523 = phi i32 [ 1, %519 ], [ %521, %520 ]
  store i32 %523, ptr %41, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %42) #4
  %524 = load i32, ptr %17, align 4, !tbaa !15
  %525 = icmp ne i32 %524, 0
  br i1 %525, label %526, label %527

526:                                              ; preds = %522
  br label %529

527:                                              ; preds = %522
  %528 = load i32, ptr %20, align 4, !tbaa !15
  br label %529

529:                                              ; preds = %527, %526
  %530 = phi i32 [ 1, %526 ], [ %528, %527 ]
  store i32 %530, ptr %42, align 4, !tbaa !15
  store i32 0, ptr %26, align 4, !tbaa !15
  br label %531

531:                                              ; preds = %663, %529
  %532 = load i32, ptr %26, align 4, !tbaa !15
  %533 = load i32, ptr %42, align 4, !tbaa !15
  %534 = icmp slt i32 %532, %533
  br i1 %534, label %535, label %666

535:                                              ; preds = %531
  store i32 0, ptr %25, align 4, !tbaa !15
  br label %536

536:                                              ; preds = %659, %535
  %537 = load i32, ptr %25, align 4, !tbaa !15
  %538 = load i32, ptr %41, align 4, !tbaa !15
  %539 = icmp slt i32 %537, %538
  br i1 %539, label %540, label %662

540:                                              ; preds = %536
  store i32 0, ptr %16, align 4, !tbaa !15
  br label %541

541:                                              ; preds = %548, %540
  %542 = load i32, ptr %16, align 4, !tbaa !15
  %543 = icmp slt i32 %542, 64
  br i1 %543, label %544, label %551

544:                                              ; preds = %541
  %545 = load i32, ptr %16, align 4, !tbaa !15
  %546 = sext i32 %545 to i64
  %547 = getelementptr inbounds [64 x i64], ptr @coef, i64 0, i64 %546
  store i64 0, ptr %547, align 8, !tbaa !31
  br label %548

548:                                              ; preds = %544
  %549 = load i32, ptr %16, align 4, !tbaa !15
  %550 = add nsw i32 %549, 1
  store i32 %550, ptr %16, align 4, !tbaa !15
  br label %541, !llvm.loop !35

551:                                              ; preds = %541
  %552 = load i32, ptr %17, align 4, !tbaa !15
  %553 = sext i32 %552 to i64
  %554 = getelementptr inbounds [3 x i32], ptr %5, i64 0, i64 %553
  %555 = load i32, ptr %554, align 4, !tbaa !15
  %556 = call i32 @decode(i32 noundef %555) #5
  store i32 %556, ptr %18, align 4, !tbaa !15
  %557 = load i32, ptr %18, align 4, !tbaa !15
  %558 = icmp ne i32 %557, 0
  br i1 %558, label %559, label %569

559:                                              ; preds = %551
  %560 = load i32, ptr %18, align 4, !tbaa !15
  %561 = call i32 @receive(i32 noundef %560) #5
  %562 = load i32, ptr %18, align 4, !tbaa !15
  %563 = call i32 @extend(i32 noundef %561, i32 noundef %562) #5
  %564 = load i32, ptr %17, align 4, !tbaa !15
  %565 = sext i32 %564 to i64
  %566 = getelementptr inbounds [3 x i32], ptr %7, i64 0, i64 %565
  %567 = load i32, ptr %566, align 4, !tbaa !15
  %568 = add nsw i32 %567, %563
  store i32 %568, ptr %566, align 4, !tbaa !15
  br label %569

569:                                              ; preds = %559, %551
  %570 = load i32, ptr %17, align 4, !tbaa !15
  %571 = sext i32 %570 to i64
  %572 = getelementptr inbounds [3 x i32], ptr %7, i64 0, i64 %571
  %573 = load i32, ptr %572, align 4, !tbaa !15
  %574 = sext i32 %573 to i64
  %575 = load i32, ptr %17, align 4, !tbaa !15
  %576 = sext i32 %575 to i64
  %577 = getelementptr inbounds [3 x i32], ptr %4, i64 0, i64 %576
  %578 = load i32, ptr %577, align 4, !tbaa !15
  %579 = sext i32 %578 to i64
  %580 = getelementptr inbounds [4 x [64 x i32]], ptr @quant, i64 0, i64 %579
  %581 = getelementptr inbounds [64 x i32], ptr %580, i64 0, i64 0
  %582 = load i32, ptr %581, align 4, !tbaa !15
  %583 = sext i32 %582 to i64
  %584 = mul nsw i64 %574, %583
  store i64 %584, ptr @coef, align 8, !tbaa !31
  store i32 1, ptr %16, align 4, !tbaa !15
  br label %585

585:                                              ; preds = %648, %646, %569
  %586 = load i32, ptr %16, align 4, !tbaa !15
  %587 = icmp slt i32 %586, 64
  br i1 %587, label %588, label %592

588:                                              ; preds = %585
  %589 = load i32, ptr @bad, align 4, !tbaa !15
  %590 = icmp ne i32 %589, 0
  %591 = xor i1 %590, true
  br label %592

592:                                              ; preds = %588, %585
  %593 = phi i1 [ false, %585 ], [ %591, %588 ]
  br i1 %593, label %594, label %649

594:                                              ; preds = %592
  call void @llvm.lifetime.start.p0(i64 -1, ptr %43) #4
  %595 = load i32, ptr %17, align 4, !tbaa !15
  %596 = sext i32 %595 to i64
  %597 = getelementptr inbounds [3 x i32], ptr %6, i64 0, i64 %596
  %598 = load i32, ptr %597, align 4, !tbaa !15
  %599 = call i32 @decode(i32 noundef %598) #5
  store i32 %599, ptr %43, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %44) #4
  %600 = load i32, ptr %43, align 4, !tbaa !15
  %601 = ashr i32 %600, 4
  store i32 %601, ptr %44, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %45) #4
  %602 = load i32, ptr %43, align 4, !tbaa !15
  %603 = and i32 %602, 15
  store i32 %603, ptr %45, align 4, !tbaa !15
  %604 = load i32, ptr %45, align 4, !tbaa !15
  %605 = icmp eq i32 %604, 0
  br i1 %605, label %606, label %613

606:                                              ; preds = %594
  %607 = load i32, ptr %44, align 4, !tbaa !15
  %608 = icmp ne i32 %607, 15
  br i1 %608, label %609, label %610

609:                                              ; preds = %606
  store i32 60, ptr %33, align 4
  br label %646

610:                                              ; preds = %606
  %611 = load i32, ptr %16, align 4, !tbaa !15
  %612 = add nsw i32 %611, 16
  store i32 %612, ptr %16, align 4, !tbaa !15
  store i32 59, ptr %33, align 4
  br label %646, !llvm.loop !36

613:                                              ; preds = %594
  %614 = load i32, ptr %44, align 4, !tbaa !15
  %615 = load i32, ptr %16, align 4, !tbaa !15
  %616 = add nsw i32 %615, %614
  store i32 %616, ptr %16, align 4, !tbaa !15
  %617 = load i32, ptr %16, align 4, !tbaa !15
  %618 = icmp sgt i32 %617, 63
  br i1 %618, label %619, label %620

619:                                              ; preds = %613
  store i32 60, ptr %33, align 4
  br label %646

620:                                              ; preds = %613
  %621 = load i32, ptr %45, align 4, !tbaa !15
  %622 = call i32 @receive(i32 noundef %621) #5
  %623 = load i32, ptr %45, align 4, !tbaa !15
  %624 = call i32 @extend(i32 noundef %622, i32 noundef %623) #5
  %625 = sext i32 %624 to i64
  %626 = load i32, ptr %17, align 4, !tbaa !15
  %627 = sext i32 %626 to i64
  %628 = getelementptr inbounds [3 x i32], ptr %4, i64 0, i64 %627
  %629 = load i32, ptr %628, align 4, !tbaa !15
  %630 = sext i32 %629 to i64
  %631 = getelementptr inbounds [4 x [64 x i32]], ptr @quant, i64 0, i64 %630
  %632 = load i32, ptr %16, align 4, !tbaa !15
  %633 = sext i32 %632 to i64
  %634 = getelementptr inbounds [64 x i32], ptr %631, i64 0, i64 %633
  %635 = load i32, ptr %634, align 4, !tbaa !15
  %636 = sext i32 %635 to i64
  %637 = mul nsw i64 %625, %636
  %638 = load i32, ptr %16, align 4, !tbaa !15
  %639 = sext i32 %638 to i64
  %640 = getelementptr inbounds [64 x i8], ptr @zigzag, i64 0, i64 %639
  %641 = load i8, ptr %640, align 1, !tbaa !23
  %642 = zext i8 %641 to i64
  %643 = getelementptr inbounds nuw [64 x i64], ptr @coef, i64 0, i64 %642
  store i64 %637, ptr %643, align 8, !tbaa !31
  %644 = load i32, ptr %16, align 4, !tbaa !15
  %645 = add nsw i32 %644, 1
  store i32 %645, ptr %16, align 4, !tbaa !15
  store i32 0, ptr %33, align 4
  br label %646

646:                                              ; preds = %620, %619, %610, %609
  call void @llvm.lifetime.end.p0(i64 -1, ptr %45) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %44) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %43) #4
  %647 = load i32, ptr %33, align 4
  switch i32 %647, label %730 [
    i32 0, label %648
    i32 60, label %649
    i32 59, label %585
  ]

648:                                              ; preds = %646
  br label %585, !llvm.loop !36

649:                                              ; preds = %646, %592
  %650 = load i32, ptr %17, align 4, !tbaa !15
  %651 = load i32, ptr %23, align 4, !tbaa !15
  %652 = load i32, ptr %41, align 4, !tbaa !15
  %653 = mul nsw i32 %651, %652
  %654 = load i32, ptr %25, align 4, !tbaa !15
  %655 = add nsw i32 %653, %654
  %656 = mul nsw i32 %655, 8
  %657 = load i32, ptr %26, align 4, !tbaa !15
  %658 = mul nsw i32 %657, 8
  call void @idct(i32 noundef %650, i32 noundef %656, i32 noundef %658) #5
  br label %659

659:                                              ; preds = %649
  %660 = load i32, ptr %25, align 4, !tbaa !15
  %661 = add nsw i32 %660, 1
  store i32 %661, ptr %25, align 4, !tbaa !15
  br label %536, !llvm.loop !37

662:                                              ; preds = %536
  br label %663

663:                                              ; preds = %662
  %664 = load i32, ptr %26, align 4, !tbaa !15
  %665 = add nsw i32 %664, 1
  store i32 %665, ptr %26, align 4, !tbaa !15
  br label %531, !llvm.loop !38

666:                                              ; preds = %531
  call void @llvm.lifetime.end.p0(i64 -1, ptr %42) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %41) #4
  br label %667

667:                                              ; preds = %666
  %668 = load i32, ptr %17, align 4, !tbaa !15
  %669 = add nsw i32 %668, 1
  store i32 %669, ptr %17, align 4, !tbaa !15
  br label %513, !llvm.loop !39

670:                                              ; preds = %513
  br label %671

671:                                              ; preds = %670
  %672 = load i32, ptr %23, align 4, !tbaa !15
  %673 = add nsw i32 %672, 1
  store i32 %673, ptr %23, align 4, !tbaa !15
  br label %481, !llvm.loop !40

674:                                              ; preds = %489
  store i32 0, ptr %15, align 4, !tbaa !15
  br label %675

675:                                              ; preds = %712, %674
  %676 = load i32, ptr %15, align 4, !tbaa !15
  %677 = load i32, ptr %28, align 4, !tbaa !15
  %678 = icmp slt i32 %676, %677
  br i1 %678, label %679, label %687

679:                                              ; preds = %675
  %680 = load i32, ptr %24, align 4, !tbaa !15
  %681 = load i32, ptr %28, align 4, !tbaa !15
  %682 = mul nsw i32 %680, %681
  %683 = load i32, ptr %15, align 4, !tbaa !15
  %684 = add nsw i32 %682, %683
  %685 = load i32, ptr %9, align 4, !tbaa !15
  %686 = icmp slt i32 %684, %685
  br label %687

687:                                              ; preds = %679, %675
  %688 = phi i1 [ false, %675 ], [ %686, %679 ]
  br i1 %688, label %689, label %715

689:                                              ; preds = %687
  %690 = load i32, ptr %15, align 4, !tbaa !15
  %691 = load i32, ptr %8, align 4, !tbaa !15
  %692 = load i32, ptr %19, align 4, !tbaa !15
  %693 = sub nsw i32 %692, 1
  %694 = load i32, ptr %20, align 4, !tbaa !15
  %695 = sub nsw i32 %694, 1
  call void @convert(i32 noundef %690, i32 noundef %691, i32 noundef %693, i32 noundef %695) #5
  %696 = load i32, ptr %8, align 4, !tbaa !15
  %697 = mul nsw i32 %696, 3
  store i32 %697, ptr %16, align 4, !tbaa !15
  br label %698

698:                                              ; preds = %706, %689
  %699 = load i32, ptr %16, align 4, !tbaa !15
  %700 = load i32, ptr %29, align 4, !tbaa !15
  %701 = icmp slt i32 %699, %700
  br i1 %701, label %702, label %709

702:                                              ; preds = %698
  %703 = load i32, ptr %16, align 4, !tbaa !15
  %704 = sext i32 %703 to i64
  %705 = getelementptr inbounds [771 x i8], ptr @line, i64 0, i64 %704
  store i8 0, ptr %705, align 1, !tbaa !23
  br label %706

706:                                              ; preds = %702
  %707 = load i32, ptr %16, align 4, !tbaa !15
  %708 = add nsw i32 %707, 1
  store i32 %708, ptr %16, align 4, !tbaa !15
  br label %698, !llvm.loop !41

709:                                              ; preds = %698
  %710 = load i32, ptr %31, align 4, !tbaa !15
  %711 = load i32, ptr %29, align 4, !tbaa !15
  call void @emit(i32 noundef %710, ptr noundef @line, i32 noundef %711) #5
  br label %712

712:                                              ; preds = %709
  %713 = load i32, ptr %15, align 4, !tbaa !15
  %714 = add nsw i32 %713, 1
  store i32 %714, ptr %15, align 4, !tbaa !15
  br label %675, !llvm.loop !42

715:                                              ; preds = %687
  br label %716

716:                                              ; preds = %715
  %717 = load i32, ptr %24, align 4, !tbaa !15
  %718 = add nsw i32 %717, 1
  store i32 %718, ptr %24, align 4, !tbaa !15
  br label %470, !llvm.loop !43

719:                                              ; preds = %478
  %720 = load i32, ptr %31, align 4, !tbaa !15
  %721 = call i64 @llrm_os_close(i32 noundef %720) #5
  %722 = load i32, ptr @infile, align 4, !tbaa !15
  %723 = call i64 @llrm_os_close(i32 noundef %722) #5
  %724 = load i64, ptr @adler_b, align 8, !tbaa !31
  %725 = shl i64 %724, 16
  %726 = load i64, ptr @adler_a, align 8, !tbaa !31
  %727 = or i64 %725, %726
  store i64 %727, ptr %1, align 8
  store i32 1, ptr %33, align 4
  br label %728

728:                                              ; preds = %719, %455, %416, %306, %290, %280, %234, %97, %56, %49
  call void @llvm.lifetime.end.p0(i64 -1, ptr %32) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %31) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %30) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %29) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %28) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %27) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %26) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %25) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %24) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %23) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %22) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %21) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %20) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %19) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %18) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %17) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %16) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %15) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %14) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %13) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %12) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %11) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %10) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %9) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %8) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %7) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %6) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %2) #4
  %729 = load i64, ptr %1, align 8
  ret i64 %729

730:                                              ; preds = %646
  unreachable
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(i64 immarg, ptr captures(none)) #1

; Function Attrs: optsize
declare i32 @llrm_os_open(ptr noundef, i8 noundef) #2

; Function Attrs: nounwind optsize
define internal void @message(ptr noundef %0) #3 {
  %2 = alloca ptr, align 8
  %3 = alloca i32, align 4
  store ptr %0, ptr %2, align 8, !tbaa !44
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) #4
  store i32 0, ptr %3, align 4, !tbaa !15
  br label %4

4:                                                ; preds = %11, %1
  %5 = load ptr, ptr %2, align 8, !tbaa !44
  %6 = load i32, ptr %3, align 4, !tbaa !15
  %7 = sext i32 %6 to i64
  %8 = getelementptr inbounds i8, ptr %5, i64 %7
  %9 = load i8, ptr %8, align 1, !tbaa !23
  %10 = icmp ne i8 %9, 0
  br i1 %10, label %11, label %14

11:                                               ; preds = %4
  %12 = load i32, ptr %3, align 4, !tbaa !15
  %13 = add nsw i32 %12, 1
  store i32 %13, ptr %3, align 4, !tbaa !15
  br label %4, !llvm.loop !47

14:                                               ; preds = %4
  %15 = load ptr, ptr %2, align 8, !tbaa !44
  %16 = load i32, ptr %3, align 4, !tbaa !15
  %17 = sext i32 %16 to i64
  %18 = call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef %15, i64 noundef %17) #5
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) #4
  ret void
}

; Function Attrs: nounwind optsize
define internal i32 @next() #3 {
  %1 = alloca i32, align 4
  %2 = load i32, ptr @inpos, align 4, !tbaa !15
  %3 = load i32, ptr @inlen, align 4, !tbaa !15
  %4 = icmp eq i32 %2, %3
  br i1 %4, label %5, label %12

5:                                                ; preds = %0
  %6 = load i32, ptr @infile, align 4, !tbaa !15
  %7 = call i32 @llrm_os_read(i32 noundef %6, ptr noundef @inbuf, i64 noundef 512) #5
  store i32 %7, ptr @inlen, align 4, !tbaa !15
  store i32 0, ptr @inpos, align 4, !tbaa !15
  %8 = load i32, ptr @inlen, align 4, !tbaa !15
  %9 = icmp sle i32 %8, 0
  br i1 %9, label %10, label %11

10:                                               ; preds = %5
  store i32 0, ptr @inlen, align 4, !tbaa !15
  store i32 1, ptr @bad, align 4, !tbaa !15
  store i32 0, ptr %1, align 4
  br label %19

11:                                               ; preds = %5
  br label %12

12:                                               ; preds = %11, %0
  %13 = load i32, ptr @inpos, align 4, !tbaa !15
  %14 = add nsw i32 %13, 1
  store i32 %14, ptr @inpos, align 4, !tbaa !15
  %15 = sext i32 %13 to i64
  %16 = getelementptr inbounds [512 x i8], ptr @inbuf, i64 0, i64 %15
  %17 = load i8, ptr %16, align 1, !tbaa !23
  %18 = zext i8 %17 to i32
  store i32 %18, ptr %1, align 4
  br label %19

19:                                               ; preds = %12, %10
  %20 = load i32, ptr %1, align 4
  ret i32 %20
}

; Function Attrs: nounwind optsize
define internal i32 @next16() #3 {
  %1 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) #4
  %2 = call i32 @next() #5
  store i32 %2, ptr %1, align 4, !tbaa !15
  %3 = load i32, ptr %1, align 4, !tbaa !15
  %4 = mul nsw i32 %3, 256
  %5 = call i32 @next() #5
  %6 = add nsw i32 %4, %5
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) #4
  ret i32 %6
}

; Function Attrs: nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(i64 immarg, ptr captures(none)) #1

; Function Attrs: nounwind optsize
define internal void @put32(i32 noundef %0, i64 noundef %1) #3 {
  %3 = alloca i32, align 4
  %4 = alloca i64, align 8
  %5 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !15
  store i64 %1, ptr %4, align 8, !tbaa !31
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) #4
  store i32 0, ptr %5, align 4, !tbaa !15
  br label %6

6:                                                ; preds = %21, %2
  %7 = load i32, ptr %5, align 4, !tbaa !15
  %8 = icmp slt i32 %7, 4
  br i1 %8, label %9, label %24

9:                                                ; preds = %6
  %10 = load i64, ptr %4, align 8, !tbaa !31
  %11 = load i32, ptr %5, align 4, !tbaa !15
  %12 = mul nsw i32 8, %11
  %13 = zext i32 %12 to i64
  %14 = lshr i64 %10, %13
  %15 = trunc i64 %14 to i8
  %16 = load i32, ptr %3, align 4, !tbaa !15
  %17 = load i32, ptr %5, align 4, !tbaa !15
  %18 = add nsw i32 %16, %17
  %19 = sext i32 %18 to i64
  %20 = getelementptr inbounds [54 x i8], ptr @bmp, i64 0, i64 %19
  store i8 %15, ptr %20, align 1, !tbaa !23
  br label %21

21:                                               ; preds = %9
  %22 = load i32, ptr %5, align 4, !tbaa !15
  %23 = add nsw i32 %22, 1
  store i32 %23, ptr %5, align 4, !tbaa !15
  br label %6, !llvm.loop !48

24:                                               ; preds = %6
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) #4
  ret void
}

; Function Attrs: optsize
declare i32 @llrm_os_create(ptr noundef) #2

; Function Attrs: nounwind optsize
define internal void @emit(i32 noundef %0, ptr noundef %1, i32 noundef %2) #3 {
  %4 = alloca i32, align 4
  %5 = alloca ptr, align 8
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  store i32 %0, ptr %4, align 4, !tbaa !15
  store ptr %1, ptr %5, align 8, !tbaa !44
  store i32 %2, ptr %6, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %7) #4
  %8 = load i32, ptr %4, align 4, !tbaa !15
  %9 = load ptr, ptr %5, align 8, !tbaa !44
  %10 = load i32, ptr %6, align 4, !tbaa !15
  %11 = sext i32 %10 to i64
  %12 = call i32 @llrm_os_write_file(i32 noundef %8, ptr noundef %9, i64 noundef %11) #5
  store i32 0, ptr %7, align 4, !tbaa !15
  br label %13

13:                                               ; preds = %41, %3
  %14 = load i32, ptr %7, align 4, !tbaa !15
  %15 = load i32, ptr %6, align 4, !tbaa !15
  %16 = icmp slt i32 %14, %15
  br i1 %16, label %17, label %44

17:                                               ; preds = %13
  %18 = load ptr, ptr %5, align 8, !tbaa !44
  %19 = load i32, ptr %7, align 4, !tbaa !15
  %20 = sext i32 %19 to i64
  %21 = getelementptr inbounds i8, ptr %18, i64 %20
  %22 = load i8, ptr %21, align 1, !tbaa !23
  %23 = zext i8 %22 to i64
  %24 = load i64, ptr @adler_a, align 8, !tbaa !31
  %25 = add i64 %24, %23
  store i64 %25, ptr @adler_a, align 8, !tbaa !31
  %26 = load i64, ptr @adler_a, align 8, !tbaa !31
  %27 = icmp uge i64 %26, 65521
  br i1 %27, label %28, label %31

28:                                               ; preds = %17
  %29 = load i64, ptr @adler_a, align 8, !tbaa !31
  %30 = sub i64 %29, 65521
  store i64 %30, ptr @adler_a, align 8, !tbaa !31
  br label %31

31:                                               ; preds = %28, %17
  %32 = load i64, ptr @adler_a, align 8, !tbaa !31
  %33 = load i64, ptr @adler_b, align 8, !tbaa !31
  %34 = add i64 %33, %32
  store i64 %34, ptr @adler_b, align 8, !tbaa !31
  %35 = load i64, ptr @adler_b, align 8, !tbaa !31
  %36 = icmp uge i64 %35, 65521
  br i1 %36, label %37, label %40

37:                                               ; preds = %31
  %38 = load i64, ptr @adler_b, align 8, !tbaa !31
  %39 = sub i64 %38, 65521
  store i64 %39, ptr @adler_b, align 8, !tbaa !31
  br label %40

40:                                               ; preds = %37, %31
  br label %41

41:                                               ; preds = %40
  %42 = load i32, ptr %7, align 4, !tbaa !15
  %43 = add nsw i32 %42, 1
  store i32 %43, ptr %7, align 4, !tbaa !15
  br label %13, !llvm.loop !49

44:                                               ; preds = %13
  call void @llvm.lifetime.end.p0(i64 -1, ptr %7) #4
  ret void
}

; Function Attrs: nounwind optsize
define internal void @restart() #3 {
  %1 = alloca i32, align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) #4
  store i32 0, ptr @bitcnt, align 4, !tbaa !15
  br label %2

2:                                                ; preds = %26, %0
  br label %3

3:                                                ; preds = %12, %2
  %4 = call i32 @next() #5
  %5 = icmp ne i32 %4, 255
  br i1 %5, label %6, label %10

6:                                                ; preds = %3
  %7 = load i32, ptr @bad, align 4, !tbaa !15
  %8 = icmp ne i32 %7, 0
  %9 = xor i1 %8, true
  br label %10

10:                                               ; preds = %6, %3
  %11 = phi i1 [ false, %3 ], [ %9, %6 ]
  br i1 %11, label %12, label %13

12:                                               ; preds = %10
  br label %3, !llvm.loop !50

13:                                               ; preds = %10
  %14 = call i32 @next() #5
  store i32 %14, ptr %1, align 4, !tbaa !15
  br label %15

15:                                               ; preds = %13
  %16 = load i32, ptr @bad, align 4, !tbaa !15
  %17 = icmp ne i32 %16, 0
  br i1 %17, label %26, label %18

18:                                               ; preds = %15
  %19 = load i32, ptr %1, align 4, !tbaa !15
  %20 = icmp slt i32 %19, 208
  br i1 %20, label %24, label %21

21:                                               ; preds = %18
  %22 = load i32, ptr %1, align 4, !tbaa !15
  %23 = icmp sgt i32 %22, 215
  br label %24

24:                                               ; preds = %21, %18
  %25 = phi i1 [ true, %18 ], [ %23, %21 ]
  br label %26

26:                                               ; preds = %24, %15
  %27 = phi i1 [ false, %15 ], [ %25, %24 ]
  br i1 %27, label %2, label %28, !llvm.loop !51

28:                                               ; preds = %26
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) #4
  ret void
}

; Function Attrs: nounwind optsize
define internal i32 @decode(i32 noundef %0) #3 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) #4
  %7 = call i32 @getbit() #5
  store i32 %7, ptr %4, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) #4
  store i32 1, ptr %5, align 4, !tbaa !15
  br label %8

8:                                                ; preds = %27, %1
  %9 = load i32, ptr %4, align 4, !tbaa !15
  %10 = load i32, ptr %3, align 4, !tbaa !15
  %11 = sext i32 %10 to i64
  %12 = getelementptr inbounds [4 x [18 x i32]], ptr @maxcode, i64 0, i64 %11
  %13 = load i32, ptr %5, align 4, !tbaa !15
  %14 = sext i32 %13 to i64
  %15 = getelementptr inbounds [18 x i32], ptr %12, i64 0, i64 %14
  %16 = load i32, ptr %15, align 4, !tbaa !15
  %17 = icmp sgt i32 %9, %16
  br i1 %17, label %18, label %28

18:                                               ; preds = %8
  %19 = load i32, ptr %4, align 4, !tbaa !15
  %20 = mul nsw i32 %19, 2
  %21 = call i32 @getbit() #5
  %22 = add nsw i32 %20, %21
  store i32 %22, ptr %4, align 4, !tbaa !15
  %23 = load i32, ptr %5, align 4, !tbaa !15
  %24 = add nsw i32 %23, 1
  store i32 %24, ptr %5, align 4, !tbaa !15
  %25 = icmp sgt i32 %24, 16
  br i1 %25, label %26, label %27

26:                                               ; preds = %18
  store i32 1, ptr @bad, align 4, !tbaa !15
  store i32 0, ptr %2, align 4
  store i32 1, ptr %6, align 4
  br label %53

27:                                               ; preds = %18
  br label %8, !llvm.loop !52

28:                                               ; preds = %8
  %29 = load i32, ptr %3, align 4, !tbaa !15
  %30 = sext i32 %29 to i64
  %31 = getelementptr inbounds [4 x [256 x i8]], ptr @huffval, i64 0, i64 %30
  %32 = load i32, ptr %3, align 4, !tbaa !15
  %33 = sext i32 %32 to i64
  %34 = getelementptr inbounds [4 x [17 x i32]], ptr @valptr, i64 0, i64 %33
  %35 = load i32, ptr %5, align 4, !tbaa !15
  %36 = sext i32 %35 to i64
  %37 = getelementptr inbounds [17 x i32], ptr %34, i64 0, i64 %36
  %38 = load i32, ptr %37, align 4, !tbaa !15
  %39 = load i32, ptr %4, align 4, !tbaa !15
  %40 = add nsw i32 %38, %39
  %41 = load i32, ptr %3, align 4, !tbaa !15
  %42 = sext i32 %41 to i64
  %43 = getelementptr inbounds [4 x [17 x i32]], ptr @mincode, i64 0, i64 %42
  %44 = load i32, ptr %5, align 4, !tbaa !15
  %45 = sext i32 %44 to i64
  %46 = getelementptr inbounds [17 x i32], ptr %43, i64 0, i64 %45
  %47 = load i32, ptr %46, align 4, !tbaa !15
  %48 = sub nsw i32 %40, %47
  %49 = sext i32 %48 to i64
  %50 = getelementptr inbounds [256 x i8], ptr %31, i64 0, i64 %49
  %51 = load i8, ptr %50, align 1, !tbaa !23
  %52 = zext i8 %51 to i32
  store i32 %52, ptr %2, align 4
  store i32 1, ptr %6, align 4
  br label %53

53:                                               ; preds = %28, %26
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) #4
  %54 = load i32, ptr %2, align 4
  ret i32 %54
}

; Function Attrs: nounwind optsize
define internal i32 @extend(i32 noundef %0, i32 noundef %1) #3 {
  %3 = alloca i32, align 4
  %4 = alloca i32, align 4
  store i32 %0, ptr %3, align 4, !tbaa !15
  store i32 %1, ptr %4, align 4, !tbaa !15
  %5 = load i32, ptr %3, align 4, !tbaa !15
  %6 = load i32, ptr %4, align 4, !tbaa !15
  %7 = sub nsw i32 %6, 1
  %8 = shl i32 1, %7
  %9 = icmp sge i32 %5, %8
  br i1 %9, label %10, label %12

10:                                               ; preds = %2
  %11 = load i32, ptr %3, align 4, !tbaa !15
  br label %18

12:                                               ; preds = %2
  %13 = load i32, ptr %3, align 4, !tbaa !15
  %14 = load i32, ptr %4, align 4, !tbaa !15
  %15 = shl i32 1, %14
  %16 = sub nsw i32 %13, %15
  %17 = add nsw i32 %16, 1
  br label %18

18:                                               ; preds = %12, %10
  %19 = phi i32 [ %11, %10 ], [ %17, %12 ]
  ret i32 %19
}

; Function Attrs: nounwind optsize
define internal i32 @receive(i32 noundef %0) #3 {
  %2 = alloca i32, align 4
  %3 = alloca i32, align 4
  store i32 %0, ptr %2, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) #4
  store i32 0, ptr %3, align 4, !tbaa !15
  br label %4

4:                                                ; preds = %8, %1
  %5 = load i32, ptr %2, align 4, !tbaa !15
  %6 = add nsw i32 %5, -1
  store i32 %6, ptr %2, align 4, !tbaa !15
  %7 = icmp ne i32 %5, 0
  br i1 %7, label %8, label %13

8:                                                ; preds = %4
  %9 = load i32, ptr %3, align 4, !tbaa !15
  %10 = mul nsw i32 %9, 2
  %11 = call i32 @getbit() #5
  %12 = add nsw i32 %10, %11
  store i32 %12, ptr %3, align 4, !tbaa !15
  br label %4, !llvm.loop !53

13:                                               ; preds = %4
  %14 = load i32, ptr %3, align 4, !tbaa !15
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) #4
  ret i32 %14
}

; Function Attrs: nounwind optsize
define internal void @idct(i32 noundef %0, i32 noundef %1, i32 noundef %2) #3 {
  %4 = alloca i32, align 4
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca [64 x i64], align 8
  %8 = alloca i64, align 8
  %9 = alloca i32, align 4
  %10 = alloca i32, align 4
  %11 = alloca i32, align 4
  %12 = alloca i32, align 4
  %13 = alloca i32, align 4
  store i32 %0, ptr %4, align 4, !tbaa !15
  store i32 %1, ptr %5, align 4, !tbaa !15
  store i32 %2, ptr %6, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %7) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %8) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %9) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %10) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %11) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %12) #4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %13) #4
  store i32 0, ptr %9, align 4, !tbaa !15
  br label %14

14:                                               ; preds = %60, %3
  %15 = load i32, ptr %9, align 4, !tbaa !15
  %16 = icmp slt i32 %15, 8
  br i1 %16, label %17, label %63

17:                                               ; preds = %14
  store i32 0, ptr %10, align 4, !tbaa !15
  br label %18

18:                                               ; preds = %56, %17
  %19 = load i32, ptr %10, align 4, !tbaa !15
  %20 = icmp slt i32 %19, 8
  br i1 %20, label %21, label %59

21:                                               ; preds = %18
  store i64 0, ptr %8, align 8, !tbaa !31
  store i32 0, ptr %11, align 4, !tbaa !15
  br label %22

22:                                               ; preds = %45, %21
  %23 = load i32, ptr %11, align 4, !tbaa !15
  %24 = icmp slt i32 %23, 8
  br i1 %24, label %25, label %48

25:                                               ; preds = %22
  %26 = load i32, ptr %10, align 4, !tbaa !15
  %27 = sext i32 %26 to i64
  %28 = getelementptr inbounds [8 x [8 x i32]], ptr @cosine, i64 0, i64 %27
  %29 = load i32, ptr %11, align 4, !tbaa !15
  %30 = sext i32 %29 to i64
  %31 = getelementptr inbounds [8 x i32], ptr %28, i64 0, i64 %30
  %32 = load i32, ptr %31, align 4, !tbaa !15
  %33 = sext i32 %32 to i64
  %34 = load i32, ptr %9, align 4, !tbaa !15
  %35 = mul nsw i32 %34, 8
  %36 = load i32, ptr %11, align 4, !tbaa !15
  %37 = add nsw i32 %35, %36
  %38 = sext i32 %37 to i64
  %39 = getelementptr inbounds [64 x i64], ptr @coef, i64 0, i64 %38
  %40 = load i64, ptr %39, align 8, !tbaa !31
  %41 = mul nsw i64 %40, 1024
  %42 = call i64 @tmul(i64 noundef %33, i64 noundef %41) #5
  %43 = load i64, ptr %8, align 8, !tbaa !31
  %44 = add nsw i64 %43, %42
  store i64 %44, ptr %8, align 8, !tbaa !31
  br label %45

45:                                               ; preds = %25
  %46 = load i32, ptr %11, align 4, !tbaa !15
  %47 = add nsw i32 %46, 1
  store i32 %47, ptr %11, align 4, !tbaa !15
  br label %22, !llvm.loop !54

48:                                               ; preds = %22
  %49 = load i64, ptr %8, align 8, !tbaa !31
  %50 = load i32, ptr %9, align 4, !tbaa !15
  %51 = mul nsw i32 %50, 8
  %52 = load i32, ptr %10, align 4, !tbaa !15
  %53 = add nsw i32 %51, %52
  %54 = sext i32 %53 to i64
  %55 = getelementptr inbounds [64 x i64], ptr %7, i64 0, i64 %54
  store i64 %49, ptr %55, align 8, !tbaa !31
  br label %56

56:                                               ; preds = %48
  %57 = load i32, ptr %10, align 4, !tbaa !15
  %58 = add nsw i32 %57, 1
  store i32 %58, ptr %10, align 4, !tbaa !15
  br label %18, !llvm.loop !55

59:                                               ; preds = %18
  br label %60

60:                                               ; preds = %59
  %61 = load i32, ptr %9, align 4, !tbaa !15
  %62 = add nsw i32 %61, 1
  store i32 %62, ptr %9, align 4, !tbaa !15
  br label %14, !llvm.loop !56

63:                                               ; preds = %14
  store i32 0, ptr %12, align 4, !tbaa !15
  br label %64

64:                                               ; preds = %132, %63
  %65 = load i32, ptr %12, align 4, !tbaa !15
  %66 = icmp slt i32 %65, 8
  br i1 %66, label %67, label %135

67:                                               ; preds = %64
  store i32 0, ptr %10, align 4, !tbaa !15
  br label %68

68:                                               ; preds = %128, %67
  %69 = load i32, ptr %10, align 4, !tbaa !15
  %70 = icmp slt i32 %69, 8
  br i1 %70, label %71, label %131

71:                                               ; preds = %68
  store i64 131584, ptr %8, align 8, !tbaa !31
  store i32 0, ptr %9, align 4, !tbaa !15
  br label %72

72:                                               ; preds = %94, %71
  %73 = load i32, ptr %9, align 4, !tbaa !15
  %74 = icmp slt i32 %73, 8
  br i1 %74, label %75, label %97

75:                                               ; preds = %72
  %76 = load i32, ptr %12, align 4, !tbaa !15
  %77 = sext i32 %76 to i64
  %78 = getelementptr inbounds [8 x [8 x i32]], ptr @cosine, i64 0, i64 %77
  %79 = load i32, ptr %9, align 4, !tbaa !15
  %80 = sext i32 %79 to i64
  %81 = getelementptr inbounds [8 x i32], ptr %78, i64 0, i64 %80
  %82 = load i32, ptr %81, align 4, !tbaa !15
  %83 = sext i32 %82 to i64
  %84 = load i32, ptr %9, align 4, !tbaa !15
  %85 = mul nsw i32 %84, 8
  %86 = load i32, ptr %10, align 4, !tbaa !15
  %87 = add nsw i32 %85, %86
  %88 = sext i32 %87 to i64
  %89 = getelementptr inbounds [64 x i64], ptr %7, i64 0, i64 %88
  %90 = load i64, ptr %89, align 8, !tbaa !31
  %91 = call i64 @tmul(i64 noundef %83, i64 noundef %90) #5
  %92 = load i64, ptr %8, align 8, !tbaa !31
  %93 = add nsw i64 %92, %91
  store i64 %93, ptr %8, align 8, !tbaa !31
  br label %94

94:                                               ; preds = %75
  %95 = load i32, ptr %9, align 4, !tbaa !15
  %96 = add nsw i32 %95, 1
  store i32 %96, ptr %9, align 4, !tbaa !15
  br label %72, !llvm.loop !57

97:                                               ; preds = %72
  %98 = load i64, ptr %8, align 8, !tbaa !31
  %99 = sdiv i64 %98, 1024
  %100 = trunc i64 %99 to i32
  store i32 %100, ptr %13, align 4, !tbaa !15
  %101 = load i32, ptr %13, align 4, !tbaa !15
  %102 = icmp slt i32 %101, 0
  br i1 %102, label %103, label %104

103:                                              ; preds = %97
  br label %112

104:                                              ; preds = %97
  %105 = load i32, ptr %13, align 4, !tbaa !15
  %106 = icmp sgt i32 %105, 255
  br i1 %106, label %107, label %108

107:                                              ; preds = %104
  br label %110

108:                                              ; preds = %104
  %109 = load i32, ptr %13, align 4, !tbaa !15
  br label %110

110:                                              ; preds = %108, %107
  %111 = phi i32 [ 255, %107 ], [ %109, %108 ]
  br label %112

112:                                              ; preds = %110, %103
  %113 = phi i32 [ 0, %103 ], [ %111, %110 ]
  %114 = trunc i32 %113 to i8
  %115 = load i32, ptr %4, align 4, !tbaa !15
  %116 = sext i32 %115 to i64
  %117 = getelementptr inbounds [3 x [16 x [256 x i8]]], ptr @planes, i64 0, i64 %116
  %118 = load i32, ptr %6, align 4, !tbaa !15
  %119 = load i32, ptr %12, align 4, !tbaa !15
  %120 = add nsw i32 %118, %119
  %121 = sext i32 %120 to i64
  %122 = getelementptr inbounds [16 x [256 x i8]], ptr %117, i64 0, i64 %121
  %123 = load i32, ptr %5, align 4, !tbaa !15
  %124 = load i32, ptr %10, align 4, !tbaa !15
  %125 = add nsw i32 %123, %124
  %126 = sext i32 %125 to i64
  %127 = getelementptr inbounds [256 x i8], ptr %122, i64 0, i64 %126
  store i8 %114, ptr %127, align 1, !tbaa !23
  br label %128

128:                                              ; preds = %112
  %129 = load i32, ptr %10, align 4, !tbaa !15
  %130 = add nsw i32 %129, 1
  store i32 %130, ptr %10, align 4, !tbaa !15
  br label %68, !llvm.loop !58

131:                                              ; preds = %68
  br label %132

132:                                              ; preds = %131
  %133 = load i32, ptr %12, align 4, !tbaa !15
  %134 = add nsw i32 %133, 1
  store i32 %134, ptr %12, align 4, !tbaa !15
  br label %64, !llvm.loop !59

135:                                              ; preds = %64
  call void @llvm.lifetime.end.p0(i64 -1, ptr %13) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %12) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %11) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %10) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %9) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %8) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %7) #4
  ret void
}

; Function Attrs: nounwind optsize
define internal void @convert(i32 noundef %0, i32 noundef %1, i32 noundef %2, i32 noundef %3) #3 {
  %5 = alloca i32, align 4
  %6 = alloca i32, align 4
  %7 = alloca i32, align 4
  %8 = alloca i32, align 4
  %9 = alloca i32, align 4
  %10 = alloca i64, align 8
  %11 = alloca i64, align 8
  %12 = alloca i64, align 8
  store i32 %0, ptr %5, align 4, !tbaa !15
  store i32 %1, ptr %6, align 4, !tbaa !15
  store i32 %2, ptr %7, align 4, !tbaa !15
  store i32 %3, ptr %8, align 4, !tbaa !15
  call void @llvm.lifetime.start.p0(i64 -1, ptr %9) #4
  store i32 0, ptr %9, align 4, !tbaa !15
  br label %13

13:                                               ; preds = %96, %4
  %14 = load i32, ptr %9, align 4, !tbaa !15
  %15 = load i32, ptr %6, align 4, !tbaa !15
  %16 = icmp slt i32 %14, %15
  br i1 %16, label %17, label %99

17:                                               ; preds = %13
  call void @llvm.lifetime.start.p0(i64 -1, ptr %10) #4
  %18 = load i32, ptr %5, align 4, !tbaa !15
  %19 = sext i32 %18 to i64
  %20 = getelementptr inbounds [16 x [256 x i8]], ptr @planes, i64 0, i64 %19
  %21 = load i32, ptr %9, align 4, !tbaa !15
  %22 = sext i32 %21 to i64
  %23 = getelementptr inbounds [256 x i8], ptr %20, i64 0, i64 %22
  %24 = load i8, ptr %23, align 1, !tbaa !23
  %25 = zext i8 %24 to i64
  %26 = mul nsw i64 %25, 1024
  store i64 %26, ptr %10, align 8, !tbaa !31
  call void @llvm.lifetime.start.p0(i64 -1, ptr %11) #4
  %27 = load i32, ptr %5, align 4, !tbaa !15
  %28 = load i32, ptr %8, align 4, !tbaa !15
  %29 = ashr i32 %27, %28
  %30 = sext i32 %29 to i64
  %31 = getelementptr inbounds [16 x [256 x i8]], ptr getelementptr inbounds nuw (i8, ptr @planes, i64 4096), i64 0, i64 %30
  %32 = load i32, ptr %9, align 4, !tbaa !15
  %33 = load i32, ptr %7, align 4, !tbaa !15
  %34 = ashr i32 %32, %33
  %35 = sext i32 %34 to i64
  %36 = getelementptr inbounds [256 x i8], ptr %31, i64 0, i64 %35
  %37 = load i8, ptr %36, align 1, !tbaa !23
  %38 = zext i8 %37 to i64
  %39 = sub nsw i64 %38, 128
  %40 = mul nsw i64 %39, 1024
  store i64 %40, ptr %11, align 8, !tbaa !31
  call void @llvm.lifetime.start.p0(i64 -1, ptr %12) #4
  %41 = load i32, ptr %5, align 4, !tbaa !15
  %42 = load i32, ptr %8, align 4, !tbaa !15
  %43 = ashr i32 %41, %42
  %44 = sext i32 %43 to i64
  %45 = getelementptr inbounds [16 x [256 x i8]], ptr getelementptr inbounds nuw (i8, ptr @planes, i64 8192), i64 0, i64 %44
  %46 = load i32, ptr %9, align 4, !tbaa !15
  %47 = load i32, ptr %7, align 4, !tbaa !15
  %48 = ashr i32 %46, %47
  %49 = sext i32 %48 to i64
  %50 = getelementptr inbounds [256 x i8], ptr %45, i64 0, i64 %49
  %51 = load i8, ptr %50, align 1, !tbaa !23
  %52 = zext i8 %51 to i64
  %53 = sub nsw i64 %52, 128
  %54 = mul nsw i64 %53, 1024
  store i64 %54, ptr %12, align 8, !tbaa !31
  %55 = load i64, ptr %10, align 8, !tbaa !31
  %56 = load i64, ptr %11, align 8, !tbaa !31
  %57 = call i64 @tmul(i64 noundef 1815, i64 noundef %56) #5
  %58 = add nsw i64 %55, %57
  %59 = add nsw i64 %58, 512
  %60 = sdiv i64 %59, 1024
  %61 = call i32 @clamp(i64 noundef %60) #5
  %62 = trunc i32 %61 to i8
  %63 = load i32, ptr %9, align 4, !tbaa !15
  %64 = mul nsw i32 3, %63
  %65 = sext i32 %64 to i64
  %66 = getelementptr inbounds [771 x i8], ptr @line, i64 0, i64 %65
  store i8 %62, ptr %66, align 1, !tbaa !23
  %67 = load i64, ptr %10, align 8, !tbaa !31
  %68 = load i64, ptr %11, align 8, !tbaa !31
  %69 = call i64 @tmul(i64 noundef 352, i64 noundef %68) #5
  %70 = sub nsw i64 %67, %69
  %71 = load i64, ptr %12, align 8, !tbaa !31
  %72 = call i64 @tmul(i64 noundef 731, i64 noundef %71) #5
  %73 = sub nsw i64 %70, %72
  %74 = add nsw i64 %73, 512
  %75 = sdiv i64 %74, 1024
  %76 = call i32 @clamp(i64 noundef %75) #5
  %77 = trunc i32 %76 to i8
  %78 = load i32, ptr %9, align 4, !tbaa !15
  %79 = mul nsw i32 3, %78
  %80 = add nsw i32 %79, 1
  %81 = sext i32 %80 to i64
  %82 = getelementptr inbounds [771 x i8], ptr @line, i64 0, i64 %81
  store i8 %77, ptr %82, align 1, !tbaa !23
  %83 = load i64, ptr %10, align 8, !tbaa !31
  %84 = load i64, ptr %12, align 8, !tbaa !31
  %85 = call i64 @tmul(i64 noundef 1436, i64 noundef %84) #5
  %86 = add nsw i64 %83, %85
  %87 = add nsw i64 %86, 512
  %88 = sdiv i64 %87, 1024
  %89 = call i32 @clamp(i64 noundef %88) #5
  %90 = trunc i32 %89 to i8
  %91 = load i32, ptr %9, align 4, !tbaa !15
  %92 = mul nsw i32 3, %91
  %93 = add nsw i32 %92, 2
  %94 = sext i32 %93 to i64
  %95 = getelementptr inbounds [771 x i8], ptr @line, i64 0, i64 %94
  store i8 %90, ptr %95, align 1, !tbaa !23
  call void @llvm.lifetime.end.p0(i64 -1, ptr %12) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %11) #4
  call void @llvm.lifetime.end.p0(i64 -1, ptr %10) #4
  br label %96

96:                                               ; preds = %17
  %97 = load i32, ptr %9, align 4, !tbaa !15
  %98 = add nsw i32 %97, 1
  store i32 %98, ptr %9, align 4, !tbaa !15
  br label %13, !llvm.loop !60

99:                                               ; preds = %13
  call void @llvm.lifetime.end.p0(i64 -1, ptr %9) #4
  ret void
}

; Function Attrs: optsize
declare i64 @llrm_os_close(i32 noundef) #2

; Function Attrs: nounwind optsize
define dso_local i32 @main() #3 {
  %1 = call i64 @bench_jpeg_fixed() #5
  call void @report(i64 noundef %1) #5
  ret i32 0
}

; Function Attrs: optsize
declare void @report(i64 noundef) #2

; Function Attrs: optsize
declare i32 @llrm_os_write_file(i32 noundef, ptr noundef, i64 noundef) #2

; Function Attrs: optsize
declare i32 @llrm_os_read(i32 noundef, ptr noundef, i64 noundef) #2

; Function Attrs: nounwind optsize
define internal i32 @getbit() #3 {
  %1 = load i32, ptr @bitcnt, align 4, !tbaa !15
  %2 = icmp eq i32 %1, 0
  br i1 %2, label %3, label %12

3:                                                ; preds = %0
  %4 = call i32 @next() #5
  store i32 %4, ptr @bitbuf, align 4, !tbaa !15
  %5 = load i32, ptr @bitbuf, align 4, !tbaa !15
  %6 = icmp eq i32 %5, 255
  br i1 %6, label %7, label %11

7:                                                ; preds = %3
  %8 = call i32 @next() #5
  %9 = icmp ne i32 %8, 0
  br i1 %9, label %10, label %11

10:                                               ; preds = %7
  store i32 1, ptr @bad, align 4, !tbaa !15
  br label %11

11:                                               ; preds = %10, %7, %3
  store i32 8, ptr @bitcnt, align 4, !tbaa !15
  br label %12

12:                                               ; preds = %11, %0
  %13 = load i32, ptr @bitcnt, align 4, !tbaa !15
  %14 = add nsw i32 %13, -1
  store i32 %14, ptr @bitcnt, align 4, !tbaa !15
  %15 = load i32, ptr @bitbuf, align 4, !tbaa !15
  %16 = load i32, ptr @bitcnt, align 4, !tbaa !15
  %17 = ashr i32 %15, %16
  %18 = and i32 %17, 1
  ret i32 %18
}

; Function Attrs: nounwind optsize
define internal i64 @tmul(i64 noundef %0, i64 noundef %1) #3 {
  %3 = alloca i64, align 8
  %4 = alloca i64, align 8
  store i64 %0, ptr %3, align 8, !tbaa !31
  store i64 %1, ptr %4, align 8, !tbaa !31
  %5 = load i64, ptr %3, align 8, !tbaa !31
  %6 = load i64, ptr %4, align 8, !tbaa !31
  %7 = mul nsw i64 %5, %6
  %8 = sdiv i64 %7, 1024
  ret i64 %8
}

; Function Attrs: nounwind optsize
define internal i32 @clamp(i64 noundef %0) #3 {
  %2 = alloca i64, align 8
  store i64 %0, ptr %2, align 8, !tbaa !31
  %3 = load i64, ptr %2, align 8, !tbaa !31
  %4 = icmp slt i64 %3, 0
  br i1 %4, label %5, label %6

5:                                                ; preds = %1
  br label %15

6:                                                ; preds = %1
  %7 = load i64, ptr %2, align 8, !tbaa !31
  %8 = icmp sgt i64 %7, 255
  br i1 %8, label %9, label %10

9:                                                ; preds = %6
  br label %13

10:                                               ; preds = %6
  %11 = load i64, ptr %2, align 8, !tbaa !31
  %12 = trunc i64 %11 to i32
  br label %13

13:                                               ; preds = %10, %9
  %14 = phi i32 [ 255, %9 ], [ %12, %10 ]
  br label %15

15:                                               ; preds = %13, %5
  %16 = phi i32 [ 0, %5 ], [ %14, %13 ]
  ret i32 %16
}

attributes #0 = { noinline nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #4 = { nounwind }
attributes #5 = { nobuiltin optsize "no-builtins" }

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
!15 = !{!12, !12, i64 0}
!16 = distinct !{!16, !17}
!17 = !{!"llvm.loop.mustprogress"}
!18 = distinct !{!18, !17}
!19 = distinct !{!19, !17}
!20 = distinct !{!20, !17}
!21 = distinct !{!21, !17}
!22 = distinct !{!22, !17}
!23 = !{!13, !13, i64 0}
!24 = distinct !{!24, !17}
!25 = distinct !{!25, !17}
!26 = distinct !{!26, !17}
!27 = distinct !{!27, !17}
!28 = distinct !{!28, !17}
!29 = distinct !{!29, !17}
!30 = distinct !{!30, !17}
!31 = !{!32, !32, i64 0}
!32 = !{!"long", !13, i64 0}
!33 = distinct !{!33, !17}
!34 = distinct !{!34, !17}
!35 = distinct !{!35, !17}
!36 = distinct !{!36, !17}
!37 = distinct !{!37, !17}
!38 = distinct !{!38, !17}
!39 = distinct !{!39, !17}
!40 = distinct !{!40, !17}
!41 = distinct !{!41, !17}
!42 = distinct !{!42, !17}
!43 = distinct !{!43, !17}
!44 = !{!45, !45, i64 0}
!45 = !{!"p1 omnipotent char", !46, i64 0}
!46 = !{!"any pointer", !13, i64 0}
!47 = distinct !{!47, !17}
!48 = distinct !{!48, !17}
!49 = distinct !{!49, !17}
!50 = distinct !{!50, !17}
!51 = distinct !{!51, !17}
!52 = distinct !{!52, !17}
!53 = distinct !{!53, !17}
!54 = distinct !{!54, !17}
!55 = distinct !{!55, !17}
!56 = distinct !{!56, !17}
!57 = distinct !{!57, !17}
!58 = distinct !{!58, !17}
!59 = distinct !{!59, !17}
!60 = distinct !{!60, !17}
