; ModuleID = 'cur.ll'
source_filename = "bench/jpeg_fixed/jpeg_fixed.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

@.str = hidden unnamed_addr constant [10 x i8] c"PHOTO.JPG\00", align 1
@infile = hidden global i32 0, align 4
@.str.1 = hidden unnamed_addr constant [22 x i8] c"PHOTO.JPG not found\0D\0A\00", align 1
@.str.2 = hidden unnamed_addr constant [13 x i8] c"not a JPEG\0D\0A\00", align 1
@bad = hidden global i32 0, align 4
@.str.3 = hidden unnamed_addr constant [43 x i8] c"16-bit quantization tables not supported\0D\0A\00", align 1
@quant = hidden global [4 x [64 x i32]] zeroinitializer, align 16
@valptr = hidden global [4 x [17 x i32]] zeroinitializer, align 16
@mincode = hidden global [4 x [17 x i32]] zeroinitializer, align 16
@maxcode = hidden global [4 x [18 x i32]] zeroinitializer, align 16
@huffval = hidden global [4 x [256 x i8]] zeroinitializer, align 16
@.str.4 = hidden unnamed_addr constant [35 x i8] c"only 8-bit samples are supported\0D\0A\00", align 1
@.str.5 = hidden unnamed_addr constant [33 x i8] c"progressive JPEG not supported\0D\0A\00", align 1
@.str.6 = hidden unnamed_addr constant [38 x i8] c"arithmetic-coded JPEG not supported\0D\0A\00", align 1
@.str.7 = hidden unnamed_addr constant [34 x i8] c"only baseline JPEG is supported\0D\0A\00", align 1
@.str.8 = hidden unnamed_addr constant [19 x i8] c"unsupported JPEG\0D\0A\00", align 1
@bmp = hidden global [54 x i8] zeroinitializer, align 16
@.str.9 = hidden unnamed_addr constant [8 x i8] c"OUT.BMP\00", align 1
@.str.10 = hidden unnamed_addr constant [23 x i8] c"cannot write OUT.BMP\0D\0A\00", align 1
@coef = hidden global [64 x i64] zeroinitializer, align 16
@zigzag = hidden constant [64 x i8] c"\00\01\08\10\09\02\03\0A\11\18 \19\12\0B\04\05\0C\13\1A!(0)\22\1B\14\0D\06\07\0E\15\1C#*1892+$\1D\16\0F\17\1E%,3:;4-&\1F'.5<=6/7>?", align 16
@line = hidden global [771 x i8] zeroinitializer, align 16
@adler_b = hidden global i64 0, align 8
@adler_a = hidden global i64 1, align 8
@inpos = hidden global i32 0, align 4
@inlen = hidden global i32 0, align 4
@inbuf = hidden global [512 x i8] zeroinitializer, align 16
@bitcnt = hidden global i32 0, align 4
@bitbuf = hidden global i32 0, align 4
@cosine = hidden constant [8 x [8 x i32]] [[8 x i32] [i32 362, i32 502, i32 473, i32 426, i32 362, i32 284, i32 196, i32 100], [8 x i32] [i32 362, i32 426, i32 196, i32 -100, i32 -362, i32 -502, i32 -473, i32 -284], [8 x i32] [i32 362, i32 284, i32 -196, i32 -502, i32 -362, i32 100, i32 473, i32 426], [8 x i32] [i32 362, i32 100, i32 -473, i32 -284, i32 362, i32 426, i32 -196, i32 -502], [8 x i32] [i32 362, i32 -100, i32 -473, i32 284, i32 362, i32 -426, i32 -196, i32 502], [8 x i32] [i32 362, i32 -284, i32 -196, i32 502, i32 -362, i32 -100, i32 473, i32 -426], [8 x i32] [i32 362, i32 -426, i32 196, i32 100, i32 -362, i32 502, i32 -473, i32 284], [8 x i32] [i32 362, i32 -502, i32 473, i32 -426, i32 362, i32 -284, i32 196, i32 -100]], align 16
@planes = hidden global [3 x [16 x [256 x i8]]] zeroinitializer, align 16

; Function Attrs: noinline nounwind uwtable
define dso_local i64 @bench_jpeg_fixed() #0 {
  %1 = alloca [3 x i32], align 4
  %2 = alloca [3 x i32], align 4
  %3 = alloca [3 x i32], align 4
  %4 = alloca [3 x i32], align 4
  %5 = alloca [3 x i32], align 4
  %6 = alloca [3 x i32], align 4
  %7 = alloca [17 x i32], align 16
  %8 = call i32 @llrm_os_open(ptr noundef nonnull @.str, i8 noundef zeroext 0) #2
  %9 = icmp slt i32 %8, 0
  br i1 %9, label %10, label %11

10:                                               ; preds = %0
  br label %364

11:                                               ; preds = %0
  %12 = call i32 @next()
  %.not = icmp eq i32 %12, 255
  br i1 %.not, label %13, label %15

13:                                               ; preds = %11
  %14 = call i32 @next()
  %.not229 = icmp eq i32 %14, 216
  br i1 %.not229, label %16, label %15

15:                                               ; preds = %13, %11
  br label %364

16:                                               ; preds = %177, %13
  %.0195 = phi i32 [ 0, %13 ], [ %.1196, %177 ]
  %.0190 = phi i32 [ 0, %13 ], [ %.1191, %177 ]
  %.0186 = phi i32 [ 0, %13 ], [ %.1187, %177 ]
  %.0182 = phi i32 [ 0, %13 ], [ %.1183, %177 ]
  %.0181 = phi i32 [ 0, %13 ], [ %.1, %177 ]
  %.not230 = icmp eq i32 %.0190, 0
  %17 = load i32, ptr @bad, align 4
  %.not231 = icmp eq i32 %17, 0
  %18 = select i1 %.not230, i1 %.not231, i1 false
  br i1 %18, label %19, label %182

19:                                               ; preds = %19, %16
  %20 = call i32 @next()
  %.not244 = icmp eq i32 %20, 255
  %21 = load i32, ptr @bad, align 4
  %.not245 = icmp eq i32 %21, 0
  %22 = select i1 %.not244, i1 false, i1 %.not245
  br i1 %22, label %19, label %23, !llvm.loop !5

23:                                               ; preds = %23, %19
  %24 = call i32 @next()
  %25 = icmp eq i32 %24, 255
  br i1 %25, label %23, label %26, !llvm.loop !7

26:                                               ; preds = %23
  %27 = call i32 @next16()
  %28 = add nsw i32 %27, -2
  switch i32 %24, label %133 [
    i32 219, label %29
    i32 196, label %46
    i32 192, label %107
  ]

29:                                               ; preds = %26, %44
  %.0199 = phi i32 [ %45, %44 ], [ %28, %26 ]
  %30 = icmp sgt i32 %.0199, 0
  br i1 %30, label %31, label %176

31:                                               ; preds = %29
  %32 = call i32 @next()
  %.not254 = icmp ult i32 %32, 16
  br i1 %.not254, label %34, label %33

33:                                               ; preds = %31
  br label %364

34:                                               ; preds = %31, %36
  %.0213 = phi i32 [ %43, %36 ], [ 0, %31 ]
  %35 = icmp samesign ult i32 %.0213, 64
  br i1 %35, label %36, label %44

36:                                               ; preds = %34
  %37 = call i32 @next()
  %38 = and i32 %32, 3
  %39 = zext nneg i32 %38 to i64
  %40 = getelementptr inbounds nuw [256 x i8], ptr @quant, i64 %39
  %41 = zext nneg i32 %.0213 to i64
  %42 = getelementptr inbounds nuw [4 x i8], ptr %40, i64 %41
  %43 = add nuw nsw i32 %.0213, 1
  br label %34, !llvm.loop !8

44:                                               ; preds = %34
  %45 = add nsw i32 %.0199, -65
  br label %29, !llvm.loop !9

46:                                               ; preds = %26, %104
  %.2201 = phi i32 [ %106, %104 ], [ %28, %26 ]
  %47 = icmp sgt i32 %.2201, 0
  %48 = load i32, ptr @bad, align 4
  %.not252 = icmp eq i32 %48, 0
  %49 = select i1 %47, i1 %.not252, i1 false
  br i1 %49, label %50, label %176

50:                                               ; preds = %46
  %51 = call i32 @next()
  %52 = ashr i32 %51, 3
  %53 = and i32 %52, -2
  %54 = and i32 %51, 1
  %55 = or disjoint i32 %53, %54
  br label %56

56:                                               ; preds = %58, %50
  %.1214 = phi i32 [ 1, %50 ], [ %63, %58 ]
  %.0210 = phi i32 [ 0, %50 ], [ %62, %58 ]
  %57 = icmp samesign ult i32 %.1214, 17
  br i1 %57, label %58, label %64

58:                                               ; preds = %56
  %59 = call i32 @next()
  %60 = zext nneg i32 %.1214 to i64
  %61 = getelementptr inbounds nuw [4 x i8], ptr %7, i64 %60
  %62 = add nsw i32 %.0210, %59
  %63 = add nuw nsw i32 %.1214, 1
  br label %56, !llvm.loop !10

64:                                               ; preds = %56, %66
  %.2215 = phi i32 [ %93, %66 ], [ 1, %56 ]
  %.0212 = phi i32 [ %92, %66 ], [ 0, %56 ]
  %.0211 = phi i32 [ %82, %66 ], [ 0, %56 ]
  %65 = icmp samesign ult i32 %.2215, 17
  br i1 %65, label %66, label %94

66:                                               ; preds = %64
  %67 = sext i32 %55 to i64
  %68 = getelementptr inbounds [68 x i8], ptr @valptr, i64 %67
  %69 = zext nneg i32 %.2215 to i64
  %70 = getelementptr inbounds nuw [4 x i8], ptr %68, i64 %69
  %71 = sext i32 %55 to i64
  %72 = getelementptr inbounds [68 x i8], ptr @mincode, i64 %71
  %73 = zext nneg i32 %.2215 to i64
  %74 = getelementptr inbounds nuw [4 x i8], ptr %72, i64 %73
  %75 = zext nneg i32 %.2215 to i64
  %76 = getelementptr inbounds nuw [4 x i8], ptr %7, i64 %75
  %77 = load i32, ptr %76, align 4
  %78 = add nsw i32 %.0212, %77
  %79 = zext nneg i32 %.2215 to i64
  %80 = getelementptr inbounds nuw [4 x i8], ptr %7, i64 %79
  %81 = load i32, ptr %80, align 4
  %82 = add nsw i32 %.0211, %81
  %83 = zext nneg i32 %.2215 to i64
  %84 = getelementptr inbounds nuw [4 x i8], ptr %7, i64 %83
  %85 = load i32, ptr %84, align 4
  %.not253 = icmp eq i32 %85, 0
  %86 = add nsw i32 %78, -1
  %87 = select i1 %.not253, i32 -1, i32 %86
  %88 = sext i32 %55 to i64
  %89 = getelementptr inbounds [72 x i8], ptr @maxcode, i64 %88
  %90 = zext nneg i32 %.2215 to i64
  %91 = getelementptr inbounds nuw [4 x i8], ptr %89, i64 %90
  %92 = shl nsw i32 %78, 1
  %93 = add nuw nsw i32 %.2215, 1
  br label %64, !llvm.loop !11

94:                                               ; preds = %64, %96
  %.3216 = phi i32 [ %103, %96 ], [ 0, %64 ]
  %95 = icmp slt i32 %.3216, %.0210
  br i1 %95, label %96, label %104

96:                                               ; preds = %94
  %97 = call i32 @next()
  %98 = trunc i32 %97 to i8
  %99 = sext i32 %55 to i64
  %100 = getelementptr inbounds [256 x i8], ptr @huffval, i64 %99
  %101 = zext nneg i32 %.3216 to i64
  %102 = getelementptr inbounds nuw i8, ptr %100, i64 %101
  store i8 %98, ptr %102, align 1
  %103 = add nuw nsw i32 %.3216, 1
  br label %94, !llvm.loop !12

104:                                              ; preds = %94
  %105 = add nsw i32 %.0210, 17
  %106 = sub nsw i32 %.2201, %105
  br label %46, !llvm.loop !13

107:                                              ; preds = %26
  %108 = call i32 @next()
  %.not249 = icmp eq i32 %108, 8
  br i1 %.not249, label %110, label %109

109:                                              ; preds = %107
  br label %364

110:                                              ; preds = %107
  %111 = call i32 @next16()
  %112 = call i32 @next16()
  %113 = call i32 @next()
  br label %114

114:                                              ; preds = %117, %110
  %.0205 = phi i32 [ 0, %110 ], [ %128, %117 ]
  %115 = icmp slt i32 %.0205, %113
  %116 = icmp samesign ult i32 %.0205, 3
  %spec.select = select i1 %115, i1 %116, i1 false
  br i1 %spec.select, label %117, label %129

117:                                              ; preds = %114
  %118 = call i32 @next()
  %119 = zext nneg i32 %.0205 to i64
  %120 = getelementptr inbounds nuw [4 x i8], ptr %1, i64 %119
  store i32 %118, ptr %120, align 4
  %121 = call i32 @next()
  %122 = zext nneg i32 %.0205 to i64
  %123 = getelementptr inbounds nuw [4 x i8], ptr %2, i64 %122
  store i32 %121, ptr %123, align 4
  %124 = call i32 @next()
  %125 = and i32 %124, 3
  %126 = zext nneg i32 %.0205 to i64
  %127 = getelementptr inbounds nuw [4 x i8], ptr %3, i64 %126
  %128 = add nuw nsw i32 %.0205, 1
  br label %114, !llvm.loop !14

129:                                              ; preds = %114
  %130 = icmp slt i32 %113, 3
  %.neg = mul i32 %113, -3
  %.neg250 = add i32 %.neg, -6
  %.neg251 = select i1 %130, i32 %.neg250, i32 -15
  %131 = add i32 %.neg251, %28
  br label %176

132:                                              ; preds = %26
  br label %364

133:                                              ; preds = %26
  %134 = icmp sgt i32 %24, 200
  %135 = icmp slt i32 %24, 208
  %or.cond = and i1 %134, %135
  %or.cond.not = xor i1 %or.cond, true
  %.not246 = icmp eq i32 %24, 204
  %or.cond255 = or i1 %or.cond.not, %.not246
  br i1 %or.cond255, label %137, label %136

136:                                              ; preds = %133
  br label %364

137:                                              ; preds = %133
  %138 = icmp sgt i32 %24, 192
  %139 = icmp slt i32 %24, 208
  %or.cond256 = and i1 %138, %139
  %or.cond256.not = xor i1 %or.cond256, true
  %140 = freeze i1 %or.cond256.not
  br i1 %140, label %142, label %switch.early.test

switch.early.test:                                ; preds = %137
  switch i32 %24, label %141 [
    i32 200, label %176
    i32 204, label %176
  ]

141:                                              ; preds = %switch.early.test
  br label %364

142:                                              ; preds = %137
  switch i32 %24, label %176 [
    i32 221, label %143
    i32 218, label %146
  ]

143:                                              ; preds = %142
  %144 = call i32 @next16()
  %145 = add nsw i32 %27, -4
  br label %176

146:                                              ; preds = %142
  %147 = call i32 @next()
  br label %148

148:                                              ; preds = %170, %146
  %.1206 = phi i32 [ 0, %146 ], [ %171, %170 ]
  %149 = icmp slt i32 %.1206, %147
  br i1 %149, label %150, label %172

150:                                              ; preds = %148
  %151 = call i32 @next()
  %152 = call i32 @next()
  br label %153

153:                                              ; preds = %168, %150
  %.0220 = phi i32 [ 0, %150 ], [ %169, %168 ]
  %154 = icmp samesign ult i32 %.0220, 3
  br i1 %154, label %155, label %170

155:                                              ; preds = %153
  %156 = zext nneg i32 %.0220 to i64
  %157 = getelementptr inbounds nuw [4 x i8], ptr %1, i64 %156
  %158 = load i32, ptr %157, align 4
  %159 = icmp eq i32 %158, %151
  br i1 %159, label %160, label %168

160:                                              ; preds = %155
  %161 = ashr i32 %152, 4
  %162 = zext nneg i32 %.0220 to i64
  %163 = getelementptr inbounds nuw [4 x i8], ptr %4, i64 %162
  %164 = and i32 %152, 1
  %165 = or disjoint i32 %164, 2
  %166 = zext nneg i32 %.0220 to i64
  %167 = getelementptr inbounds nuw [4 x i8], ptr %5, i64 %166
  store i32 %165, ptr %167, align 4
  br label %168

168:                                              ; preds = %155, %160
  %169 = add nuw nsw i32 %.0220, 1
  br label %153, !llvm.loop !15

170:                                              ; preds = %153
  %171 = add nuw nsw i32 %.1206, 1
  br label %148, !llvm.loop !16

172:                                              ; preds = %148
  %173 = call i32 @next()
  %174 = call i32 @next()
  %175 = call i32 @next()
  br label %176

176:                                              ; preds = %switch.early.test, %switch.early.test, %46, %172, %142, %143, %129, %29
  %.1200 = phi i32 [ %.0199, %29 ], [ %.2201, %46 ], [ %131, %129 ], [ %145, %143 ], [ 0, %172 ], [ %28, %142 ], [ %28, %switch.early.test ], [ %28, %switch.early.test ]
  %.1196 = phi i32 [ %.0195, %29 ], [ %.0195, %46 ], [ %113, %129 ], [ %.0195, %143 ], [ %.0195, %142 ], [ %.0195, %172 ], [ %.0195, %switch.early.test ], [ %.0195, %switch.early.test ]
  %.1191 = phi i32 [ %.0190, %29 ], [ %.0190, %46 ], [ %.0190, %129 ], [ %.0190, %143 ], [ 1, %172 ], [ %.0190, %142 ], [ %.0190, %switch.early.test ], [ %.0190, %switch.early.test ]
  %.1187 = phi i32 [ %.0186, %29 ], [ %.0186, %46 ], [ %.0186, %129 ], [ %144, %143 ], [ %.0186, %142 ], [ %.0186, %172 ], [ %.0186, %switch.early.test ], [ %.0186, %switch.early.test ]
  %.1183 = phi i32 [ %.0182, %29 ], [ %.0182, %46 ], [ %111, %129 ], [ %.0182, %143 ], [ %.0182, %142 ], [ %.0182, %172 ], [ %.0182, %switch.early.test ], [ %.0182, %switch.early.test ]
  %.1 = phi i32 [ %.0181, %29 ], [ %.0181, %46 ], [ %112, %129 ], [ %.0181, %143 ], [ %.0181, %142 ], [ %.0181, %172 ], [ %.0181, %switch.early.test ], [ %.0181, %switch.early.test ]
  br label %177

177:                                              ; preds = %179, %176
  %.7 = phi i32 [ %.1200, %176 ], [ %180, %179 ]
  %178 = icmp sgt i32 %.7, 0
  br i1 %178, label %179, label %16, !llvm.loop !17

179:                                              ; preds = %177
  %180 = add nsw i32 %.7, -1
  %181 = call i32 @next()
  br label %177, !llvm.loop !18

182:                                              ; preds = %16
  %183 = load i32, ptr %2, align 4
  %184 = ashr i32 %183, 4
  %185 = and i32 %183, 15
  %186 = load i32, ptr @bad, align 4
  %.not232 = icmp eq i32 %186, 0
  %.not233 = icmp eq i32 %.0195, 3
  %or.cond259 = select i1 %.not232, i1 %.not233, i1 false
  %or.cond259.not = xor i1 %or.cond259, true
  %187 = icmp slt i32 %184, 1
  %or.cond260 = select i1 %or.cond259.not, i1 true, i1 %187
  %188 = icmp sgt i32 %184, 2
  %or.cond261 = select i1 %or.cond260, i1 true, i1 %188
  %189 = icmp eq i32 %185, 0
  %or.cond262 = select i1 %or.cond261, i1 true, i1 %189
  %190 = icmp samesign ugt i32 %185, 2
  %or.cond263 = select i1 %or.cond262, i1 true, i1 %190
  %or.cond263.not = xor i1 %or.cond263, true
  %191 = getelementptr inbounds nuw i8, ptr %2, i64 4
  %192 = load i32, ptr %191, align 4
  %.not234 = icmp eq i32 %192, 17
  %or.cond265 = select i1 %or.cond263.not, i1 %.not234, i1 false
  %193 = getelementptr inbounds nuw i8, ptr %2, i64 8
  %194 = load i32, ptr %193, align 4
  %.not235 = icmp eq i32 %194, 17
  %or.cond267 = select i1 %or.cond265, i1 %.not235, i1 false
  %or.cond267.not = xor i1 %or.cond267, true
  %195 = icmp slt i32 %.0181, 1
  %or.cond268 = select i1 %or.cond267.not, i1 true, i1 %195
  %196 = icmp sgt i32 %.0181, 256
  %or.cond269 = select i1 %or.cond268, i1 true, i1 %196
  %197 = icmp slt i32 %.0182, 1
  %or.cond270 = select i1 %or.cond269, i1 true, i1 %197
  br i1 %or.cond270, label %198, label %199

198:                                              ; preds = %182
  br label %364

199:                                              ; preds = %182
  %200 = shl nsw i32 %184, 3
  %201 = shl nuw nsw i32 %185, 3
  %202 = add nuw nsw i32 %.0181, %200
  %203 = add nsw i32 %202, -1
  %204 = sdiv i32 %203, %200
  %205 = add nuw nsw i32 %.0182, %201
  %206 = add nsw i32 %205, -1
  %207 = sdiv i32 %206, %201
  %208 = mul nuw nsw i32 %.0181, 3
  %209 = add nuw nsw i32 %208, 3
  %210 = and i32 %209, 2147483644
  %211 = zext nneg i32 %210 to i64
  %212 = zext nneg i32 %.0182 to i64
  %213 = mul nuw nsw i64 %211, %212
  %214 = add nuw nsw i64 %213, 54
  %215 = zext nneg i32 %.0181 to i64
  %216 = zext nneg i32 %.0182 to i64
  %217 = sub nsw i64 0, %216
  %218 = call i32 @llrm_os_create(ptr noundef nonnull @.str.9) #2
  %219 = icmp slt i32 %218, 0
  br i1 %219, label %220, label %221

220:                                              ; preds = %199
  br label %364

221:                                              ; preds = %199
  br label %222

222:                                              ; preds = %224, %221
  %.2207 = phi i32 [ 0, %221 ], [ %227, %224 ]
  %223 = icmp samesign ult i32 %.2207, 3
  br i1 %223, label %224, label %228

224:                                              ; preds = %222
  %225 = zext nneg i32 %.2207 to i64
  %226 = getelementptr inbounds nuw [4 x i8], ptr %6, i64 %225
  %227 = add nuw nsw i32 %.2207, 1
  br label %222, !llvm.loop !19

228:                                              ; preds = %222, %354
  %.0226 = phi i32 [ %355, %354 ], [ 0, %222 ]
  %.0222 = phi i32 [ %.1223, %354 ], [ %.0186, %222 ]
  %229 = icmp slt i32 %.0226, %207
  %230 = load i32, ptr @bad, align 4
  %.not236 = icmp eq i32 %230, 0
  %231 = select i1 %229, i1 %.not236, i1 false
  br i1 %231, label %232, label %356

232:                                              ; preds = %228, %334
  %.0225 = phi i32 [ %335, %334 ], [ 0, %228 ]
  %.1223 = phi i32 [ %246, %334 ], [ %.0222, %228 ]
  %233 = icmp slt i32 %.0225, %204
  %234 = load i32, ptr @bad, align 4
  %.not237 = icmp eq i32 %234, 0
  %235 = select i1 %233, i1 %.not237, i1 false
  br i1 %235, label %236, label %336

236:                                              ; preds = %232
  %.not238 = icmp ne i32 %.0186, 0
  %237 = icmp eq i32 %.1223, 0
  %or.cond271 = select i1 %.not238, i1 %237, i1 false
  br i1 %or.cond271, label %238, label %245

238:                                              ; preds = %236
  br label %239

239:                                              ; preds = %241, %238
  %.3208 = phi i32 [ 0, %238 ], [ %244, %241 ]
  %240 = icmp samesign ult i32 %.3208, 3
  br i1 %240, label %241, label %245

241:                                              ; preds = %239
  %242 = zext nneg i32 %.3208 to i64
  %243 = getelementptr inbounds nuw [4 x i8], ptr %6, i64 %242
  store i32 0, ptr %243, align 4
  %244 = add nuw nsw i32 %.3208, 1
  br label %239, !llvm.loop !20

245:                                              ; preds = %239, %236
  %.2224 = phi i32 [ %.1223, %236 ], [ %.0186, %239 ]
  %246 = add nsw i32 %.2224, -1
  br label %247

247:                                              ; preds = %332, %245
  %.1221 = phi i32 [ 0, %245 ], [ %333, %332 ]
  %248 = icmp samesign ult i32 %.1221, 3
  br i1 %248, label %249, label %334

249:                                              ; preds = %247
  %.not239 = icmp eq i32 %.1221, 0
  %. = select i1 %.not239, i32 %184, i32 1
  %.not240 = icmp eq i32 %.1221, 0
  %250 = select i1 %.not240, i32 %185, i32 1
  br label %251

251:                                              ; preds = %330, %249
  %.0227 = phi i32 [ 0, %249 ], [ %331, %330 ]
  %252 = icmp samesign ult i32 %.0227, %250
  br i1 %252, label %253, label %332

253:                                              ; preds = %251, %324
  %.0228 = phi i32 [ %329, %324 ], [ 0, %251 ]
  %254 = icmp slt i32 %.0228, %.
  br i1 %254, label %255, label %330

255:                                              ; preds = %253, %257
  %.4217 = phi i32 [ %260, %257 ], [ 0, %253 ]
  %256 = icmp samesign ult i32 %.4217, 64
  br i1 %256, label %257, label %261

257:                                              ; preds = %255
  %258 = zext nneg i32 %.4217 to i64
  %259 = getelementptr inbounds nuw [8 x i8], ptr @coef, i64 %258
  %260 = add nuw nsw i32 %.4217, 1
  br label %255, !llvm.loop !21

261:                                              ; preds = %255
  %262 = zext nneg i32 %.1221 to i64
  %263 = getelementptr inbounds nuw [4 x i8], ptr %4, i64 %262
  %264 = load i32, ptr %263, align 4
  %265 = call i32 @decode(i32 noundef %264)
  %.not241 = icmp eq i32 %265, 0
  br i1 %.not241, label %273, label %266

266:                                              ; preds = %261
  %267 = call i32 @receive(i32 noundef %265)
  %268 = call i32 @extend(i32 noundef %267, i32 noundef %265)
  %269 = zext nneg i32 %.1221 to i64
  %270 = getelementptr inbounds nuw [4 x i8], ptr %6, i64 %269
  %271 = load i32, ptr %270, align 4
  %272 = add nsw i32 %271, %268
  br label %273

273:                                              ; preds = %266, %261
  %274 = zext nneg i32 %.1221 to i64
  %275 = getelementptr inbounds nuw [4 x i8], ptr %6, i64 %274
  %276 = load i32, ptr %275, align 4
  %277 = sext i32 %276 to i64
  %278 = zext nneg i32 %.1221 to i64
  %279 = getelementptr inbounds nuw [4 x i8], ptr %3, i64 %278
  %280 = load i32, ptr %279, align 4
  %281 = sext i32 %280 to i64
  %282 = getelementptr inbounds [256 x i8], ptr @quant, i64 %281
  %283 = load i32, ptr %282, align 16
  %284 = sext i32 %283 to i64
  %285 = mul nsw i64 %277, %284
  store i64 %285, ptr @coef, align 16
  br label %286

286:                                              ; preds = %304, %299, %273
  %.5218 = phi i32 [ 1, %273 ], [ %300, %299 ], [ %323, %304 ]
  %287 = icmp slt i32 %.5218, 64
  %288 = load i32, ptr @bad, align 4
  %.not242 = icmp eq i32 %288, 0
  %289 = select i1 %287, i1 %.not242, i1 false
  br i1 %289, label %290, label %324

290:                                              ; preds = %286
  %291 = zext nneg i32 %.1221 to i64
  %292 = getelementptr inbounds nuw [4 x i8], ptr %5, i64 %291
  %293 = load i32, ptr %292, align 4
  %294 = call i32 @decode(i32 noundef %293)
  %295 = ashr i32 %294, 4
  %296 = and i32 %294, 15
  %297 = icmp eq i32 %296, 0
  br i1 %297, label %298, label %301

298:                                              ; preds = %290
  %.not243 = icmp eq i32 %295, 15
  br i1 %.not243, label %299, label %324

299:                                              ; preds = %298
  %300 = add nsw i32 %.5218, 16
  br label %286, !llvm.loop !22

301:                                              ; preds = %290
  %302 = add nsw i32 %.5218, %295
  %303 = icmp sgt i32 %302, 63
  br i1 %303, label %324, label %304

304:                                              ; preds = %301
  %305 = call i32 @receive(i32 noundef %296)
  %306 = call i32 @extend(i32 noundef %305, i32 noundef %296)
  %307 = sext i32 %306 to i64
  %308 = zext nneg i32 %.1221 to i64
  %309 = getelementptr inbounds nuw [4 x i8], ptr %3, i64 %308
  %310 = load i32, ptr %309, align 4
  %311 = sext i32 %310 to i64
  %312 = getelementptr inbounds [256 x i8], ptr @quant, i64 %311
  %313 = sext i32 %302 to i64
  %314 = getelementptr inbounds [4 x i8], ptr %312, i64 %313
  %315 = load i32, ptr %314, align 4
  %316 = sext i32 %315 to i64
  %317 = mul nsw i64 %307, %316
  %318 = sext i32 %302 to i64
  %319 = getelementptr inbounds i8, ptr @zigzag, i64 %318
  %320 = load i8, ptr %319, align 1
  %321 = zext i8 %320 to i64
  %322 = getelementptr inbounds nuw [8 x i8], ptr @coef, i64 %321
  %323 = add nsw i32 %302, 1
  br label %286, !llvm.loop !22

324:                                              ; preds = %301, %298, %286
  %325 = mul nuw nsw i32 %.0225, %.
  %326 = add nuw nsw i32 %325, %.0228
  %327 = shl nsw i32 %326, 3
  %328 = shl nsw i32 %.0227, 3
  call void @idct(i32 noundef %.1221, i32 noundef %327, i32 noundef %328)
  %329 = add nuw nsw i32 %.0228, 1
  br label %253, !llvm.loop !23

330:                                              ; preds = %253
  %331 = add nuw nsw i32 %.0227, 1
  br label %251, !llvm.loop !24

332:                                              ; preds = %251
  %333 = add nuw nsw i32 %.1221, 1
  br label %247, !llvm.loop !25

334:                                              ; preds = %247
  %335 = add nuw nsw i32 %.0225, 1
  br label %232, !llvm.loop !26

336:                                              ; preds = %232, %352
  %.4209 = phi i32 [ %353, %352 ], [ 0, %232 ]
  %337 = icmp samesign ult i32 %.4209, %201
  %338 = mul nuw nsw i32 %.0226, %201
  %339 = add nuw nsw i32 %338, %.4209
  %340 = icmp slt i32 %339, %.0182
  %341 = select i1 %337, i1 %340, i1 false
  br i1 %341, label %342, label %354

342:                                              ; preds = %336
  %343 = add nsw i32 %184, -1
  %344 = add nsw i32 %185, -1
  call void @convert(i32 noundef %.4209, i32 noundef %.0181, i32 noundef %343, i32 noundef %344)
  %345 = mul nuw nsw i32 %.0181, 3
  br label %346

346:                                              ; preds = %348, %342
  %.6219 = phi i32 [ %345, %342 ], [ %351, %348 ]
  %347 = icmp samesign ult i32 %.6219, %210
  br i1 %347, label %348, label %352

348:                                              ; preds = %346
  %349 = zext nneg i32 %.6219 to i64
  %350 = getelementptr inbounds nuw i8, ptr @line, i64 %349
  %351 = add nuw nsw i32 %.6219, 1
  br label %346, !llvm.loop !27

352:                                              ; preds = %346
  call void @emit(i32 noundef %218, ptr noundef nonnull @line, i32 noundef %210)
  %353 = add nuw nsw i32 %.4209, 1
  br label %336, !llvm.loop !28

354:                                              ; preds = %336
  %355 = add nuw nsw i32 %.0226, 1
  br label %228, !llvm.loop !29

356:                                              ; preds = %228
  %357 = call i64 @llrm_os_close(i32 noundef %218) #2
  %358 = load i32, ptr @infile, align 4
  %359 = call i64 @llrm_os_close(i32 noundef %358) #2
  %360 = load i64, ptr @adler_b, align 8
  %361 = shl i64 %360, 16
  %362 = load i64, ptr @adler_a, align 8
  %363 = or i64 %361, %362
  br label %364

364:                                              ; preds = %356, %220, %198, %141, %136, %132, %109, %33, %15, %10
  %.0 = phi i64 [ 0, %10 ], [ 0, %15 ], [ 0, %33 ], [ 0, %109 ], [ 0, %132 ], [ 0, %136 ], [ 0, %141 ], [ 0, %198 ], [ 0, %220 ], [ %363, %356 ]
  ret i64 %.0
}

declare i32 @llrm_os_open(ptr noundef, i8 noundef zeroext) #1

; Function Attrs: noinline nounwind uwtable
declare hidden void @message(ptr noundef) #0

; Function Attrs: noinline nounwind uwtable
define hidden i32 @next() #0 {
  %1 = load i32, ptr @inpos, align 4
  %2 = load i32, ptr @inlen, align 4
  %3 = icmp eq i32 %1, %2
  br i1 %3, label %4, label %9

4:                                                ; preds = %0
  %5 = load i32, ptr @infile, align 4
  %6 = call i32 @llrm_os_read(i32 noundef %5, ptr noundef nonnull @inbuf, i64 noundef 512) #2
  %7 = icmp slt i32 %6, 1
  br i1 %7, label %8, label %9

8:                                                ; preds = %4
  br label %16

9:                                                ; preds = %4, %0
  %10 = load i32, ptr @inpos, align 4
  %11 = add nsw i32 %10, 1
  %12 = sext i32 %10 to i64
  %13 = getelementptr inbounds i8, ptr @inbuf, i64 %12
  %14 = load i8, ptr %13, align 1
  %15 = zext i8 %14 to i32
  br label %16

16:                                               ; preds = %9, %8
  %.0 = phi i32 [ 0, %8 ], [ %15, %9 ]
  ret i32 %.0
}

; Function Attrs: noinline nounwind uwtable
declare hidden i32 @next16() #0

; Function Attrs: noinline nounwind uwtable
declare hidden void @put32(i32 noundef, i64 noundef) #0

declare i32 @llrm_os_create(ptr noundef) #1

; Function Attrs: noinline nounwind uwtable
declare hidden void @emit(i32 noundef, ptr noundef, i32 noundef) #0

; Function Attrs: noinline nounwind uwtable
declare hidden void @restart() #0

; Function Attrs: noinline nounwind uwtable
declare hidden i32 @decode(i32 noundef) #0

; Function Attrs: noinline nounwind uwtable
declare hidden i32 @extend(i32 noundef, i32 noundef) #0

; Function Attrs: noinline nounwind uwtable
declare hidden i32 @receive(i32 noundef) #0

; Function Attrs: noinline nounwind uwtable
declare hidden void @idct(i32 noundef, i32 noundef, i32 noundef) #0

; Function Attrs: noinline nounwind uwtable
declare hidden void @convert(i32 noundef, i32 noundef, i32 noundef, i32 noundef) #0

declare i64 @llrm_os_close(i32 noundef) #1

declare i32 @llrm_os_read(i32 noundef, ptr noundef, i64 noundef) #1

attributes #0 = { noinline nounwind uwtable "frame-pointer"="all" "min-legal-vector-width"="0" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #1 = { "frame-pointer"="all" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="x86-64" "target-features"="+cmov,+cx8,+fxsr,+mmx,+sse,+sse2,+x87" "tune-cpu"="generic" }
attributes #2 = { nounwind }


!5 = distinct !{!5, !6}
!6 = !{!"llvm.loop.mustprogress"}
!7 = distinct !{!7, !6}
!8 = distinct !{!8, !6}
!9 = distinct !{!9, !6}
!10 = distinct !{!10, !6}
!11 = distinct !{!11, !6}
!12 = distinct !{!12, !6}
!13 = distinct !{!13, !6}
!14 = distinct !{!14, !6}
!15 = distinct !{!15, !6}
!16 = distinct !{!16, !6}
!17 = distinct !{!17, !6}
!18 = distinct !{!18, !6}
!19 = distinct !{!19, !6}
!20 = distinct !{!20, !6}
!21 = distinct !{!21, !6}
!22 = distinct !{!22, !6}
!23 = distinct !{!23, !6}
!24 = distinct !{!24, !6}
!25 = distinct !{!25, !6}
!26 = distinct !{!26, !6}
!27 = distinct !{!27, !6}
!28 = distinct !{!28, !6}
!29 = distinct !{!29, !6}

