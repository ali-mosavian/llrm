target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@.str = private unnamed_addr constant [10 x i8] c"PHOTO.JPG\00", align 1
@infile = internal unnamed_addr global i32 0, align 4
@.str.1 = private unnamed_addr constant [22 x i8] c"PHOTO.JPG not found\0D\0A\00", align 1
@.str.2 = private unnamed_addr constant [13 x i8] c"not a JPEG\0D\0A\00", align 1
@bad = internal unnamed_addr global i1 false, align 4
@.str.3 = private unnamed_addr constant [43 x i8] c"16-bit quantization tables not supported\0D\0A\00", align 1
@quant = internal unnamed_addr global [4 x [64 x i32]] zeroinitializer, align 4
@valptr = internal unnamed_addr global [4 x [17 x i32]] zeroinitializer, align 4
@mincode = internal unnamed_addr global [4 x [17 x i32]] zeroinitializer, align 4
@maxcode = internal unnamed_addr global [4 x [18 x i32]] zeroinitializer, align 4
@huffval = internal unnamed_addr global [4 x [256 x i8]] zeroinitializer, align 1
@.str.4 = private unnamed_addr constant [35 x i8] c"only 8-bit samples are supported\0D\0A\00", align 1
@.str.5 = private unnamed_addr constant [33 x i8] c"progressive JPEG not supported\0D\0A\00", align 1
@.str.6 = private unnamed_addr constant [38 x i8] c"arithmetic-coded JPEG not supported\0D\0A\00", align 1
@.str.7 = private unnamed_addr constant [34 x i8] c"only baseline JPEG is supported\0D\0A\00", align 1
@.str.8 = private unnamed_addr constant [19 x i8] c"unsupported JPEG\0D\0A\00", align 1
@bmp = internal global [54 x i8] zeroinitializer, align 1
@.str.9 = private unnamed_addr constant [8 x i8] c"OUT.BMP\00", align 1
@.str.10 = private unnamed_addr constant [23 x i8] c"cannot write OUT.BMP\0D\0A\00", align 1
@coef = internal unnamed_addr global [64 x i64] zeroinitializer, align 8
@zigzag = internal unnamed_addr constant [64 x i8] c"\00\01\08\10\09\02\03\0A\11\18 \19\12\0B\04\05\0C\13\1A!(0)\22\1B\14\0D\06\07\0E\15\1C#*1892+$\1D\16\0F\17\1E%,3:;4-&\1F'.5<=6/7>?", align 1
@line = internal global [771 x i8] zeroinitializer, align 1
@adler_b = internal unnamed_addr global i64 0, align 8
@adler_a = internal unnamed_addr global i64 1, align 8
@inpos = internal unnamed_addr global i32 0, align 4
@inlen = internal unnamed_addr global i32 0, align 4
@inbuf = internal global [512 x i8] zeroinitializer, align 1
@bitcnt = internal unnamed_addr global i32 0, align 4
@bitbuf = internal unnamed_addr global i32 0, align 4
@cosine = internal unnamed_addr constant [8 x [8 x float]] [[8 x float] [float 0x3FD6A09E60000000, float 0x3FDF6297C0000000, float 0x3FDD906BC0000000, float 0x3FDA9B6620000000, float 0x3FD6A09E60000000, float 0x3FD1C73B40000000, float 0x3FC87DE2A0000000, float 0x3FB8F8B840000000], [8 x float] [float 0x3FD6A09E60000000, float 0x3FDA9B6620000000, float 0x3FC87DE2A0000000, float 0xBFB8F8B840000000, float 0xBFD6A09E60000000, float 0xBFDF6297C0000000, float 0xBFDD906BC0000000, float 0xBFD1C73B40000000], [8 x float] [float 0x3FD6A09E60000000, float 0x3FD1C73B40000000, float 0xBFC87DE2A0000000, float 0xBFDF6297C0000000, float 0xBFD6A09E60000000, float 0x3FB8F8B840000000, float 0x3FDD906BC0000000, float 0x3FDA9B6620000000], [8 x float] [float 0x3FD6A09E60000000, float 0x3FB8F8B840000000, float 0xBFDD906BC0000000, float 0xBFD1C73B40000000, float 0x3FD6A09E60000000, float 0x3FDA9B6620000000, float 0xBFC87DE2A0000000, float 0xBFDF6297C0000000], [8 x float] [float 0x3FD6A09E60000000, float 0xBFB8F8B840000000, float 0xBFDD906BC0000000, float 0x3FD1C73B40000000, float 0x3FD6A09E60000000, float 0xBFDA9B6620000000, float 0xBFC87DE2A0000000, float 0x3FDF6297C0000000], [8 x float] [float 0x3FD6A09E60000000, float 0xBFD1C73B40000000, float 0xBFC87DE2A0000000, float 0x3FDF6297C0000000, float 0xBFD6A09E60000000, float 0xBFB8F8B840000000, float 0x3FDD906BC0000000, float 0xBFDA9B6620000000], [8 x float] [float 0x3FD6A09E60000000, float 0xBFDA9B6620000000, float 0x3FC87DE2A0000000, float 0x3FB8F8B840000000, float 0xBFD6A09E60000000, float 0x3FDF6297C0000000, float 0xBFDD906BC0000000, float 0x3FD1C73B40000000], [8 x float] [float 0x3FD6A09E60000000, float 0xBFDF6297C0000000, float 0x3FDD906BC0000000, float 0xBFDA9B6620000000, float 0x3FD6A09E60000000, float 0xBFD1C73B40000000, float 0x3FC87DE2A0000000, float 0xBFB8F8B840000000]], align 4
@planes = internal unnamed_addr global [3 x [16 x [256 x i8]]] zeroinitializer, align 1

define i64 @bench_jpeg_single() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"  {
  %1 = alloca [64 x float], align 4
  %2 = alloca [3 x i32], align 4
  %3 = alloca [3 x i32], align 4
  %4 = alloca [3 x i32], align 4
  %5 = alloca [3 x i32], align 4
  %6 = alloca [3 x i32], align 4
  %7 = alloca [3 x i32], align 4
  %8 = alloca [17 x i32], align 4
  call void @llvm.lifetime.start.p0(i64 -1, ptr %2) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %5) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %6) nounwind
  call void @llvm.lifetime.start.p0(i64 -1, ptr %7) nounwind
  %9 = tail call i32 @llrm_os_open(ptr noundef @.str, i8 noundef 0) nobuiltin "no-builtins"
  store i32 %9, ptr @infile, align 4, !tbaa !14
  %10 = icmp slt i32 %9, 0
  br i1 %10, label %11, label %12

11:
  tail call fastcc void @message(ptr noundef @.str.1) nobuiltin "no-builtins"
  br label %533

12:
  %13 = tail call i32 @next() nobuiltin "no-builtins"
  %14 = icmp eq i32 %13, 255
  br i1 %14, label %15, label %25

15:
  %16 = tail call i32 @next() nobuiltin "no-builtins"
  %17 = icmp eq i32 %16, 216
  br i1 %17, label %18, label %25

18:
  %19 = getelementptr i8, ptr %8, i64 68
  %20 = getelementptr i8, ptr @valptr, i64 68
  %21 = getelementptr i8, ptr @mincode, i64 68
  %22 = getelementptr i8, ptr %8, i64 68
  %23 = getelementptr i8, ptr @maxcode, i64 68
  %24 = getelementptr i8, ptr @quant, i64 256
  br label %27

25:
  tail call fastcc void @message(ptr noundef @.str.2) nobuiltin "no-builtins"
  br label %533

26:
  br i1 %205, label %27, label %214

27:
  %28 = phi i32 [ %206, %26 ], [ 0, %18 ]
  %29 = phi i32 [ %204, %26 ], [ 0, %18 ]
  %30 = phi i32 [ %203, %26 ], [ 0, %18 ]
  %31 = phi i32 [ %202, %26 ], [ 0, %18 ]
  %32 = load i1, ptr @bad, align 4
  br i1 %32, label %214, label %33

33:
  br label %34

34:
  %35 = tail call i32 @next() nobuiltin "no-builtins"
  %36 = icmp eq i32 %35, 255
  br i1 %36, label %39, label %37

37:
  %38 = load i1, ptr @bad, align 4
  br i1 %38, label %39, label %34, !llvm.loop !15

39:
  br label %40

40:
  %41 = tail call i32 @next() nobuiltin "no-builtins"
  %42 = freeze i32 %41
  %43 = icmp eq i32 %42, 255
  br i1 %43, label %40, label %44, !llvm.loop !18

44:
  %45 = tail call i32 @next16() nobuiltin "no-builtins"
  %46 = add nsw i32 %45, -2
  %47 = trunc nuw i32 %42 to i8
  %48 = zext i8 %47 to i32
  switch i32 %48, label %158 [
    i32 219, label %52
    i32 196, label %49
    i32 192, label %129
    i32 194, label %157
  ]

49:
  %50 = icmp samesign ugt i32 %45, 2
  br i1 %50, label %51, label %201

51:
  br label %73

52:
  %53 = icmp samesign ugt i32 %45, 2
  br i1 %53, label %54, label %201

54:
  br label %55

55:
  %56 = phi i32 [ %71, %70 ], [ %46, %54 ]
  %57 = tail call i32 @next() nobuiltin "no-builtins"
  %58 = icmp samesign ult i32 %57, 16
  br i1 %58, label %59, label %64

59:
  %60 = and i32 %57, 3
  %61 = zext nneg i32 %60 to i64
  %62 = shl nuw nsw i64 %61, 8
  %63 = getelementptr i8, ptr %24, i64 %62
  br label %65

64:
  tail call fastcc void @message(ptr noundef @.str.3) nobuiltin "no-builtins"
  br label %533

65:
  %66 = phi i64 [ %69, %65 ], [ -256, %59 ]
  %67 = tail call i32 @next() nobuiltin "no-builtins"
  %68 = getelementptr i8, ptr %63, i64 %66
  store i32 %67, ptr %68, align 4, !tbaa !14
  %69 = add nsw i64 %66, 4
  %exitcond10 = icmp eq i64 %69, 0
  br i1 %exitcond10, label %70, label %65, !llvm.loop !19

70:
  %71 = add nsw i32 %56, -65
  %72 = icmp sgt i32 %56, 65
  br i1 %72, label %55, label %201, !llvm.loop !20

73:
  %74 = phi i32 [ %127, %125 ], [ %46, %51 ]
  %75 = load i1, ptr @bad, align 4
  br i1 %75, label %201, label %76

76:
  call void @llvm.lifetime.start.p0(i64 -1, ptr %8) nounwind
  %77 = tail call i32 @next() nobuiltin "no-builtins"
  %78 = lshr i32 %77, 3
  %79 = and i32 %78, 30
  %80 = and i32 %77, 1
  %81 = or disjoint i32 %79, %80
  br label %89

82:
  %83 = zext nneg i32 %81 to i64
  %84 = mul nuw nsw i64 %83, 68
  %85 = getelementptr i8, ptr %20, i64 %84
  %86 = getelementptr i8, ptr %21, i64 %84
  %87 = mul nuw nsw i64 %83, 72
  %88 = getelementptr i8, ptr %23, i64 %87
  br label %103

89:
  %90 = phi i64 [ %95, %89 ], [ -64, %76 ]
  %91 = phi i32 [ 0, %76 ], [ %94, %89 ]
  %92 = tail call i32 @next() nobuiltin "no-builtins"
  %93 = getelementptr i8, ptr %19, i64 %90
  store i32 %92, ptr %93, align 4, !tbaa !14
  %94 = add i32 %92, %91
  %95 = add nsw i64 %90, 4
  %exitcond7 = icmp eq i64 %95, 0
  br i1 %exitcond7, label %82, label %89, !llvm.loop !21

96:
  %97 = icmp eq i32 %94, 0
  br i1 %97, label %125, label %98

98:
  %wide.trip.count2 = zext i32 %94 to i64
  %99 = shl nuw nsw i64 %83, 8
  %100 = getelementptr i8, ptr @huffval, i64 %99
  %101 = getelementptr i8, ptr %100, i64 %wide.trip.count2
  %102 = mul nsw i64 %wide.trip.count2, -1
  br label %119

103:
  %104 = phi i64 [ %118, %103 ], [ -64, %82 ]
  %105 = phi i32 [ 0, %82 ], [ %112, %103 ]
  %106 = phi i32 [ 0, %82 ], [ %117, %103 ]
  %107 = getelementptr i8, ptr %85, i64 %104
  store i32 %105, ptr %107, align 4, !tbaa !14
  %108 = getelementptr i8, ptr %86, i64 %104
  store i32 %106, ptr %108, align 4, !tbaa !14
  %109 = getelementptr i8, ptr %22, i64 %104
  %110 = load i32, ptr %109, align 4, !tbaa !14
  %111 = add nsw i32 %110, %106
  %112 = add nsw i32 %110, %105
  %113 = icmp eq i32 %110, 0
  %114 = add nsw i32 %111, -1
  %115 = select i1 %113, i32 -1, i32 %114
  %116 = getelementptr i8, ptr %88, i64 %104
  store i32 %115, ptr %116, align 4, !tbaa !14
  %117 = shl nsw i32 %111, 1
  %118 = add nsw i64 %104, 4
  %exitcond8 = icmp eq i64 %118, 0
  br i1 %exitcond8, label %96, label %103, !llvm.loop !22

119:
  %120 = phi i64 [ %math, %119 ], [ %102, %98 ]
  %121 = tail call i32 @next() nobuiltin "no-builtins"
  %122 = trunc nuw i32 %121 to i8
  %123 = getelementptr i8, ptr %101, i64 %120
  store i8 %122, ptr %123, align 1, !tbaa !23
  %124 = call { i64, i1 } @llvm.uadd.with.overflow.i64(i64 %120, i64 1)
  %math = extractvalue { i64, i1 } %124, 0
  %ov = extractvalue { i64, i1 } %124, 1
  br i1 %ov, label %125, label %119, !llvm.loop !24

125:
  %126 = add nsw i32 %74, -17
  %127 = sub i32 %126, %94
  call void @llvm.lifetime.end.p0(i64 -1, ptr %8) nounwind
  %128 = icmp sgt i32 %127, 0
  br i1 %128, label %73, label %201, !llvm.loop !25

129:
  %130 = tail call i32 @next() nobuiltin "no-builtins"
  %131 = icmp eq i32 %130, 8
  br i1 %131, label %133, label %132

132:
  tail call fastcc void @message(ptr noundef @.str.4) nobuiltin "no-builtins"
  br label %533

133:
  %134 = tail call i32 @next16() nobuiltin "no-builtins"
  %135 = tail call i32 @next16() nobuiltin "no-builtins"
  %136 = tail call i32 @next() nobuiltin "no-builtins"
  %invariant.umin = tail call i32 @llvm.umin.i32(i32 %136, i32 3)
  %137 = icmp eq i32 %invariant.umin, 0
  br i1 %137, label %152, label %138

138:
  %wide.trip.count1 = zext i32 %invariant.umin to i64
  br label %139

139:
  %140 = phi i64 [ %151, %139 ], [ %wide.trip.count1, %138 ]
  %141 = phi ptr [ %150, %139 ], [ %4, %138 ]
  %142 = phi ptr [ %149, %139 ], [ %3, %138 ]
  %143 = phi ptr [ %148, %139 ], [ %2, %138 ]
  %144 = tail call i32 @next() nobuiltin "no-builtins"
  store i32 %144, ptr %143, align 4, !tbaa !14
  %145 = tail call i32 @next() nobuiltin "no-builtins"
  store i32 %145, ptr %142, align 4, !tbaa !14
  %146 = tail call i32 @next() nobuiltin "no-builtins"
  %147 = and i32 %146, 3
  store i32 %147, ptr %141, align 4, !tbaa !14
  %148 = getelementptr i8, ptr %143, i64 4
  %149 = getelementptr i8, ptr %142, i64 4
  %150 = getelementptr i8, ptr %141, i64 4
  %151 = add nsw i64 %140, -1
  %exitcond6 = icmp eq i64 %151, 0
  br i1 %exitcond6, label %152, label %139, !llvm.loop !26

152:
  %153 = tail call i32 @llvm.umin.i32(i32 %136, i32 3)
  %154 = mul nsw i32 %153, -3
  %155 = add nsw i32 %45, -8
  %156 = add nsw i32 %155, %154
  br label %201

157:
  tail call fastcc void @message(ptr noundef @.str.5) nobuiltin "no-builtins"
  br label %533

158:
  %159 = add nsw i32 %42, -201
  %160 = icmp ult i32 %159, 7
  %161 = icmp ne i32 %42, 204
  %162 = and i1 %161, %160
  br i1 %162, label %163, label %164

163:
  tail call fastcc void @message(ptr noundef @.str.6) nobuiltin "no-builtins"
  br label %533

164:
  %165 = add i32 %42, -193
  %166 = icmp ult i32 %165, 15
  br i1 %166, label %switch.early.test, label %170

switch.early.test:
  %167 = trunc nuw i32 %42 to i8
  %168 = zext i8 %167 to i32
  switch i32 %168, label %169 [
    i32 200, label %201
    i32 204, label %201
  ]

169:
  tail call fastcc void @message(ptr noundef @.str.7) nobuiltin "no-builtins"
  br label %533

170:
  switch i32 %42, label %201 [
    i32 221, label %171
    i32 218, label %174
  ]

171:
  %172 = tail call i32 @next16() nobuiltin "no-builtins"
  %173 = add nsw i32 %45, -4
  br label %201

174:
  %175 = tail call i32 @next() nobuiltin "no-builtins"
  %176 = icmp eq i32 %175, 0
  br i1 %176, label %197, label %177

177:
  br label %178

178:
  %179 = phi i32 [ %196, %195 ], [ 0, %177 ]
  %180 = tail call i32 @next() nobuiltin "no-builtins"
  %181 = tail call i32 @next() nobuiltin "no-builtins"
  %182 = lshr i32 %181, 4
  %183 = and i32 %181, 1
  %184 = or disjoint i32 %183, 2
  br label %185

185:
  %186 = phi i64 [ %194, %193 ], [ 0, %178 ]
  %187 = getelementptr i8, ptr %2, i64 %186
  %188 = load i32, ptr %187, align 4, !tbaa !14
  %189 = icmp eq i32 %188, %180
  br i1 %189, label %190, label %193

190:
  %191 = getelementptr i8, ptr %5, i64 %186
  store i32 %182, ptr %191, align 4, !tbaa !14
  %192 = getelementptr i8, ptr %6, i64 %186
  store i32 %184, ptr %192, align 4, !tbaa !14
  br label %193

193:
  %194 = add nuw nsw i64 %186, 4
  %exitcond12 = icmp eq i64 %194, 12
  br i1 %exitcond12, label %195, label %185, !llvm.loop !27

195:
  %196 = add nuw nsw i32 %179, 1
  %exitcond13 = icmp eq i32 %196, %175
  br i1 %exitcond13, label %197, label %178, !llvm.loop !28

197:
  %198 = tail call i32 @next() nobuiltin "no-builtins"
  %199 = tail call i32 @next() nobuiltin "no-builtins"
  %200 = tail call i32 @next() nobuiltin "no-builtins"
  br label %201

201:
  %202 = phi i32 [ %135, %152 ], [ %31, %171 ], [ %31, %197 ], [ %31, %switch.early.test ], [ %31, %170 ], [ %31, %switch.early.test ], [ %31, %52 ], [ %31, %49 ], [ %31, %70 ], [ %31, %125 ], [ %31, %73 ]
  %203 = phi i32 [ %134, %152 ], [ %30, %171 ], [ %30, %197 ], [ %30, %switch.early.test ], [ %30, %170 ], [ %30, %switch.early.test ], [ %30, %52 ], [ %30, %49 ], [ %30, %70 ], [ %30, %125 ], [ %30, %73 ]
  %204 = phi i32 [ %29, %152 ], [ %172, %171 ], [ %29, %197 ], [ %29, %switch.early.test ], [ %29, %170 ], [ %29, %switch.early.test ], [ %29, %52 ], [ %29, %49 ], [ %29, %70 ], [ %29, %125 ], [ %29, %73 ]
  %205 = phi i1 [ true, %152 ], [ true, %171 ], [ false, %197 ], [ true, %switch.early.test ], [ true, %170 ], [ true, %switch.early.test ], [ true, %52 ], [ true, %49 ], [ true, %70 ], [ true, %125 ], [ true, %73 ]
  %206 = phi i32 [ %136, %152 ], [ %28, %171 ], [ %28, %197 ], [ %28, %switch.early.test ], [ %28, %170 ], [ %28, %switch.early.test ], [ %28, %52 ], [ %28, %49 ], [ %28, %70 ], [ %28, %125 ], [ %28, %73 ]
  %207 = phi i32 [ %156, %152 ], [ %173, %171 ], [ 0, %197 ], [ %46, %switch.early.test ], [ %46, %170 ], [ %46, %switch.early.test ], [ %46, %52 ], [ %46, %49 ], [ %71, %70 ], [ %74, %73 ], [ %127, %125 ]
  %208 = icmp sgt i32 %207, 0
  br i1 %208, label %209, label %26, !llvm.loop !30

209:
  br label %210

210:
  %211 = phi i32 [ %212, %210 ], [ %207, %209 ]
  %212 = add i32 %211, -1
  %213 = tail call i32 @next() nobuiltin "no-builtins"
  %exitcond14 = icmp eq i32 %212, 0
  br i1 %exitcond14, label %26, label %210, !llvm.loop !29

214:
  %215 = phi i32 [ %206, %26 ], [ %28, %27 ]
  %216 = phi i32 [ %204, %26 ], [ %29, %27 ]
  %217 = phi i32 [ %203, %26 ], [ %30, %27 ]
  %218 = phi i32 [ %202, %26 ], [ %31, %27 ]
  %219 = load i32, ptr %3, align 4, !tbaa !14
  %220 = zext i32 %219 to i64
  %221 = ashr i32 %219, 4
  %222 = and i64 %220, 15
  %223 = trunc i64 %222 to i32
  %224 = load i1, ptr @bad, align 4
  %225 = icmp ne i32 %215, 3
  %or.cond6 = select i1 %224, i1 true, i1 %225
  %226 = add nsw i32 %221, -3
  %227 = icmp ult i32 %226, -2
  %228 = select i1 %or.cond6, i1 true, i1 %227
  %229 = icmp eq i32 %223, 0
  %or.cond9 = select i1 %228, i1 true, i1 %229
  %230 = icmp samesign ugt i32 %223, 2
  %or.cond10 = select i1 %or.cond9, i1 true, i1 %230
  %231 = getelementptr inbounds nuw i8, ptr %3, i64 4
  %232 = load i32, ptr %231, align 4
  %233 = icmp ne i32 %232, 17
  %or.cond11 = select i1 %or.cond10, i1 true, i1 %233
  %234 = getelementptr inbounds nuw i8, ptr %3, i64 8
  %235 = load i32, ptr %234, align 4
  %236 = icmp ne i32 %235, 17
  %or.cond12 = select i1 %or.cond11, i1 true, i1 %236
  %237 = add nsw i32 %218, -257
  %238 = icmp ult i32 %237, -256
  %239 = select i1 %or.cond12, i1 true, i1 %238
  %240 = icmp slt i32 %217, 1
  %or.cond15 = select i1 %239, i1 true, i1 %240
  br i1 %or.cond15, label %241, label %242

241:
  tail call fastcc void @message(ptr noundef @.str.8) nobuiltin "no-builtins"
  br label %533

242:
  %243 = shl nsw i32 %221, 3
  %244 = shl nuw nsw i64 %222, 3
  %245 = trunc i64 %244 to i32
  %246 = add nsw i32 %218, -1
  %247 = add nsw i32 %246, %243
  %248 = sdiv i32 %247, %243
  %249 = add nsw i32 %217, -1
  %250 = add nsw i32 %249, %245
  %251 = sdiv i32 %250, %245
  %252 = mul nuw nsw i32 %218, 3
  %253 = add nuw nsw i32 %252, 3
  %254 = and i32 %253, 4092
  %255 = zext nneg i32 %254 to i64
  %256 = zext nneg i32 %217 to i64
  %257 = mul nuw nsw i64 %255, %256
  %258 = add nuw nsw i64 %257, 54
  store i16 19778, ptr @bmp
  tail call fastcc void @put32(i32 noundef 2, i64 noundef %258) nobuiltin "no-builtins"
  tail call fastcc void @put32(i32 noundef 10, i64 noundef 54) nobuiltin "no-builtins"
  tail call fastcc void @put32(i32 noundef 14, i64 noundef 40) nobuiltin "no-builtins"
  %259 = zext nneg i32 %218 to i64
  tail call fastcc void @put32(i32 noundef 18, i64 noundef %259) nobuiltin "no-builtins"
  %260 = sub nsw i64 0, %256
  tail call fastcc void @put32(i32 noundef 22, i64 noundef %260) nobuiltin "no-builtins"
  store i8 1, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 26), align 2, !tbaa !23
  store i8 24, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 28), align 2, !tbaa !23
  tail call fastcc void @put32(i32 noundef 34, i64 noundef %257) nobuiltin "no-builtins"
  tail call fastcc void @put32(i32 noundef 38, i64 noundef 2835) nobuiltin "no-builtins"
  tail call fastcc void @put32(i32 noundef 42, i64 noundef 2835) nobuiltin "no-builtins"
  %261 = tail call i32 @llrm_os_create(ptr noundef @.str.9) nobuiltin "no-builtins"
  %262 = icmp slt i32 %261, 0
  br i1 %262, label %263, label %264

263:
  tail call fastcc void @message(ptr noundef @.str.10) nobuiltin "no-builtins"
  br label %533

264:
  tail call fastcc void @emit(i32 noundef %261, ptr noundef @bmp, i32 noundef 54) nobuiltin "no-builtins"
  br label %275

265:
  %266 = icmp sgt i32 %251, 0
  br i1 %266, label %267, label %525

267:
  %268 = add nsw i32 %221, -1
  %269 = add nsw i32 %223, -1
  %wide.trip.count7 = zext nneg i32 %251 to i64
  %270 = zext i32 %252 to i64
  %wide.trip.count6 = zext nneg i32 %245 to i64
  %wide.trip.count5 = zext i32 %254 to i64
  %271 = getelementptr i8, ptr @coef, i64 512
  %272 = getelementptr i8, ptr @line, i64 2
  %273 = getelementptr i8, ptr @line, i64 %wide.trip.count5
  %274 = sub i64 %270, %wide.trip.count5
  br label %279

275:
  %276 = phi i64 [ %278, %275 ], [ 0, %264 ]
  %277 = getelementptr i8, ptr %7, i64 %276
  store i32 0, ptr %277, align 4, !tbaa !14
  %278 = add nuw nsw i64 %276, 4
  %exitcond15 = icmp eq i64 %278, 12
  br i1 %exitcond15, label %265, label %275, !llvm.loop !33

279:
  %280 = phi i64 [ 0, %267 ], [ %524, %523 ]
  %281 = phi i32 [ %216, %267 ], [ %464, %523 ]
  %282 = load i1, ptr @bad, align 4
  br i1 %282, label %525, label %283

283:
  %284 = icmp sgt i32 %248, 0
  br i1 %284, label %285, label %463

285:
  br label %286

286:
  %287 = phi i32 [ %311, %461 ], [ %281, %285 ]
  %288 = phi i32 [ %462, %461 ], [ 0, %285 ]
  %289 = load i1, ptr @bad, align 4
  br i1 %289, label %463, label %290

290:
  %291 = icmp ne i32 %216, 0
  %292 = icmp eq i32 %287, 0
  %or.cond16 = select i1 %291, i1 %292, i1 false
  br i1 %or.cond16, label %293, label %309

293:
  store i32 0, ptr @bitcnt, align 4, !tbaa !14
  br label %294

294:
  %295 = tail call i32 @next() nobuiltin "no-builtins"
  %296 = icmp eq i32 %295, 255
  br i1 %296, label %299, label %297

297:
  %298 = load i1, ptr @bad, align 4
  br i1 %298, label %299, label %294, !llvm.loop !50

299:
  %300 = tail call i32 @next() nobuiltin "no-builtins"
  %301 = load i1, ptr @bad, align 4
  %not = xor i1 %301, true
  %302 = add nsw i32 %300, -216
  %303 = icmp ult i32 %302, -8
  %or.cond17 = select i1 %not, i1 %303, i1 false
  br i1 %or.cond17, label %294, label %304, !llvm.loop !51

304:
  br label %305

305:
  %306 = phi i64 [ 0, %304 ], [ %308, %305 ]
  %307 = getelementptr i8, ptr %7, i64 %306
  store i32 0, ptr %307, align 4, !tbaa !14
  %308 = add nuw nsw i64 %306, 4
  %exitcond16 = icmp eq i64 %308, 12
  br i1 %exitcond16, label %309, label %305, !llvm.loop !34

309:
  %310 = phi i32 [ %287, %290 ], [ %216, %305 ]
  %311 = add nsw i32 %310, -1
  br label %312

312:
  %313 = phi i64 [ 0, %309 ], [ %460, %459 ]
  %314 = icmp eq i64 %313, 0
  %315 = select i1 %314, i32 %221, i32 1
  %316 = select i1 %314, i32 %223, i32 1
  %317 = icmp eq i32 %316, 0
  br i1 %317, label %459, label %318

318:
  %319 = icmp sgt i32 %315, 0
  %320 = getelementptr inbounds nuw [4096 x i8], ptr @planes, i64 %313
  br i1 %319, label %321, label %459

321:
  %322 = getelementptr inbounds nuw [4 x i8], ptr %5, i64 %313
  %323 = mul nuw nsw i32 %315, %288
  %wide.trip.count4 = zext nneg i32 %316 to i64
  %324 = load i32, ptr %322, align 4, !tbaa !14
  %wide.trip.count3 = zext nneg i32 %315 to i64
  br label %325

325:
  %326 = phi i64 [ 0, %321 ], [ %458, %457 ]
  %327 = shl i64 %326, 11
  %328 = getelementptr i8, ptr %320, i64 %327
  br label %329

329:
  %330 = phi i64 [ 0, %325 ], [ %456, %455 ]
  br label %331

331:
  %332 = phi i64 [ %334, %331 ], [ -512, %329 ]
  %333 = getelementptr i8, ptr %271, i64 %332
  store i64 0, ptr %333, align 8, !tbaa !31
  %334 = add nsw i64 %332, 8
  %exitcond17 = icmp eq i64 %334, 0
  br i1 %exitcond17, label %335, label %331, !llvm.loop !35

335:
  %336 = tail call fastcc i32 @decode(i32 noundef %324) nobuiltin "no-builtins"
  %337 = icmp eq i32 %336, 0
  br i1 %337, label %345, label %338

338:
  %339 = tail call fastcc i32 @receive(i32 noundef %336) nobuiltin "no-builtins"
  %340 = tail call fastcc i32 @extend(i32 noundef %339, i32 noundef %336) nobuiltin "no-builtins"
  %341 = mul i64 %313, 4
  %342 = getelementptr inbounds i8, ptr %7, i64 %341
  %343 = load i32, ptr %342, align 4, !tbaa !14
  %344 = add nsw i32 %343, %340
  store i32 %344, ptr %342, align 4, !tbaa !14
  br label %345

345:
  %346 = mul i64 %313, 4
  %347 = getelementptr inbounds i8, ptr %7, i64 %346
  %348 = load i32, ptr %347, align 4, !tbaa !14
  %349 = sext i32 %348 to i64
  %350 = mul i64 %313, 4
  %351 = getelementptr inbounds i8, ptr %4, i64 %350
  %352 = load i32, ptr %351, align 4, !tbaa !14
  %353 = sext i32 %352 to i64
  %354 = getelementptr inbounds [256 x i8], ptr @quant, i64 %353
  %355 = load i32, ptr %354, align 4, !tbaa !14
  %356 = sext i32 %355 to i64
  %357 = mul nsw i64 %356, %349
  store i64 %357, ptr @coef, align 8, !tbaa !31
  br label %358

358:
  %359 = phi i32 [ 1, %345 ], [ %393, %392 ]
  %360 = icmp slt i32 %359, 64
  br i1 %360, label %361, label %395

361:
  %362 = load i1, ptr @bad, align 4
  br i1 %362, label %395, label %363

363:
  %364 = mul i64 %313, 4
  %365 = getelementptr inbounds i8, ptr %6, i64 %364
  %366 = load i32, ptr %365, align 4, !tbaa !14
  %367 = tail call fastcc i32 @decode(i32 noundef %366) nobuiltin "no-builtins"
  %368 = lshr i32 %367, 4
  %369 = and i32 %367, 15
  %370 = icmp eq i32 %369, 0
  br i1 %370, label %371, label %375

371:
  %372 = icmp ne i32 %368, 15
  %373 = add nsw i32 %359, 16
  %374 = select i1 %372, i32 %359, i32 %373
  br label %392, !llvm.loop !36

375:
  %376 = add nsw i32 %368, %359
  %377 = icmp sgt i32 %376, 63
  br i1 %377, label %392, label %378

378:
  %379 = tail call fastcc i32 @receive(i32 noundef %369) nobuiltin "no-builtins"
  %380 = tail call fastcc i32 @extend(i32 noundef %379, i32 noundef %369) nobuiltin "no-builtins"
  %381 = sext i32 %380 to i64
  %382 = sext i32 %376 to i64
  %383 = getelementptr inbounds [4 x i8], ptr %354, i64 %382
  %384 = load i32, ptr %383, align 4, !tbaa !14
  %385 = sext i32 %384 to i64
  %386 = mul nsw i64 %385, %381
  %387 = getelementptr inbounds i8, ptr @zigzag, i64 %382
  %388 = load i8, ptr %387, align 1, !tbaa !23
  %389 = zext i8 %388 to i64
  %390 = getelementptr inbounds nuw [8 x i8], ptr @coef, i64 %389
  store i64 %386, ptr %390, align 8, !tbaa !31
  %391 = add nsw i32 %376, 1
  br label %392

392:
  %393 = phi i32 [ %391, %378 ], [ %374, %371 ], [ %376, %375 ]
  %394 = phi i1 [ false, %378 ], [ %372, %371 ], [ true, %375 ]
  br i1 %394, label %395, label %358

395:
  %396 = trunc i64 %330 to i32
  %397 = add i32 %323, %396
  %398 = shl nsw i32 %397, 3
  call void @llvm.lifetime.start.p0(i64 -1, ptr %1) nounwind
  br label %399

399:
  %400 = phi ptr [ %425, %423 ], [ @coef, %395 ]
  %401 = phi i64 [ 0, %395 ], [ %424, %423 ]
  %402 = shl nuw nsw i64 %401, 5
  %invariant.gep2 = getelementptr inbounds nuw i8, ptr %1, i64 %402
  br label %405

403:
  %404 = zext nneg i32 %398 to i64
  %invariant.gep5 = getelementptr i8, ptr %328, i64 %404
  br label %426

405:
  %406 = phi ptr [ %422, %420 ], [ @cosine, %399 ]
  %407 = phi i64 [ 0, %399 ], [ %421, %420 ]
  br label %408

408:
  %409 = phi i64 [ 0, %405 ], [ %419, %408 ]
  %410 = phi float [ 0.000000e+00, %405 ], [ %418, %408 ]
  %411 = shl nuw nsw i64 %409, 2
  %412 = getelementptr i8, ptr %406, i64 %411
  %413 = load float, ptr %412, align 4, !tbaa !54
  %414 = shl nuw nsw i64 %409, 3
  %415 = getelementptr i8, ptr %400, i64 %414
  %416 = load i64, ptr %415, align 8, !tbaa !31
  %417 = sitofp i64 %416 to float
  %418 = tail call float @llvm.fmuladd.f32(float %413, float %417, float %410)
  %419 = add nuw nsw i64 %409, 1
  %exitcond1 = icmp eq i64 %419, 8
  br i1 %exitcond1, label %420, label %408, !llvm.loop !56

420:
  %gep2 = getelementptr inbounds nuw [4 x i8], ptr %invariant.gep2, i64 %407
  store float %418, ptr %gep2, align 4, !tbaa !54
  %421 = add nuw nsw i64 %407, 1
  %422 = getelementptr i8, ptr %406, i64 32
  %exitcond11 = icmp eq i64 %421, 8
  br i1 %exitcond11, label %423, label %405, !llvm.loop !57

423:
  %424 = add nuw nsw i64 %401, 1
  %425 = getelementptr i8, ptr %400, i64 64
  %exitcond2 = icmp eq i64 %424, 8
  br i1 %exitcond2, label %403, label %399, !llvm.loop !58

426:
  %427 = phi ptr [ %454, %452 ], [ @cosine, %403 ]
  %428 = phi i64 [ 0, %403 ], [ %453, %452 ]
  %gep5 = getelementptr [256 x i8], ptr %invariant.gep5, i64 %428
  br label %429

429:
  %430 = phi ptr [ %451, %442 ], [ %1, %426 ]
  %431 = phi i64 [ 0, %426 ], [ %450, %442 ]
  br label %432

432:
  %433 = phi ptr [ %441, %432 ], [ %430, %429 ]
  %434 = phi i64 [ %440, %432 ], [ 0, %429 ]
  %435 = phi float [ 0.000000e+00, %429 ], [ %439, %432 ]
  %436 = getelementptr i8, ptr %427, i64 %434
  %437 = load float, ptr %436, align 4, !tbaa !54
  %438 = load float, ptr %433, align 4, !tbaa !54
  %439 = tail call float @llvm.fmuladd.f32(float %437, float %438, float %435)
  %440 = add nuw nsw i64 %434, 4
  %441 = getelementptr i8, ptr %433, i64 32
  %exitcond3 = icmp eq i64 %440, 32
  br i1 %exitcond3, label %442, label %432, !llvm.loop !59

442:
  %443 = fadd float %439, 1.285000e+02
  %444 = fcmp ole float %443, 0.000000e+00
  %445 = fcmp oge float %443, 2.550000e+02
  %446 = select i1 %445, float 2.550000e+02, float %443
  %447 = fptosi float %446 to i32
  %448 = trunc i32 %447 to i8
  %449 = select i1 %444, i8 0, i8 %448
  %gep3 = getelementptr i8, ptr %gep5, i64 %431
  store i8 %449, ptr %gep3, align 1, !tbaa !23
  %450 = add nuw nsw i64 %431, 1
  %451 = getelementptr i8, ptr %430, i64 4
  %exitcond4 = icmp eq i64 %450, 8
  br i1 %exitcond4, label %452, label %429, !llvm.loop !60

452:
  %453 = add nuw nsw i64 %428, 1
  %454 = getelementptr i8, ptr %427, i64 32
  %exitcond5 = icmp eq i64 %453, 8
  br i1 %exitcond5, label %455, label %426, !llvm.loop !61

455:
  call void @llvm.lifetime.end.p0(i64 -1, ptr %1) nounwind
  %456 = add nuw nsw i64 %330, 1
  %exitcond18 = icmp eq i64 %456, %wide.trip.count3
  br i1 %exitcond18, label %457, label %329, !llvm.loop !37

457:
  %458 = add nuw nsw i64 %326, 1
  %exitcond19 = icmp eq i64 %458, %wide.trip.count4
  br i1 %exitcond19, label %459, label %325, !llvm.loop !38

459:
  %460 = add nuw nsw i64 %313, 1
  %exitcond20 = icmp eq i64 %460, 3
  br i1 %exitcond20, label %461, label %312, !llvm.loop !39

461:
  %462 = add nuw nsw i32 %288, 1
  %exitcond21 = icmp eq i32 %462, %248
  br i1 %exitcond21, label %463, label %286, !llvm.loop !40

463:
  %464 = phi i32 [ %281, %283 ], [ %287, %286 ], [ %311, %461 ]
  %465 = mul nuw nsw i64 %280, %244
  br label %466

466:
  %467 = phi ptr [ %522, %520 ], [ @planes, %463 ]
  %468 = phi i64 [ 0, %463 ], [ %521, %520 ]
  %469 = add nuw nsw i64 %468, %465
  %470 = trunc nuw i64 %469 to i32
  %471 = icmp sgt i32 %217, %470
  br i1 %471, label %472, label %523

472:
  %473 = icmp eq i32 %218, 0
  br i1 %473, label %513, label %474

474:
  %475 = trunc nuw nsw i64 %468 to i32
  %476 = lshr i32 %475, %269
  %477 = zext nneg i32 %476 to i64
  %478 = getelementptr inbounds nuw [256 x i8], ptr getelementptr inbounds nuw (i8, ptr @planes, i64 4096), i64 %477
  %479 = getelementptr inbounds nuw [256 x i8], ptr getelementptr inbounds nuw (i8, ptr @planes, i64 8192), i64 %477
  br label %480

480:
  %481 = phi ptr [ %512, %480 ], [ %272, %474 ]
  %482 = phi i64 [ 0, %474 ], [ %511, %480 ]
  %483 = getelementptr i8, ptr %467, i64 %482
  %484 = load i8, ptr %483, align 1, !tbaa !23
  %485 = uitofp i8 %484 to float
  %tmp = trunc i64 %482 to i32
  %486 = lshr i32 %tmp, %268
  %487 = zext nneg i32 %486 to i64
  %488 = getelementptr inbounds nuw i8, ptr %478, i64 %487
  %489 = load i8, ptr %488, align 1, !tbaa !23
  %490 = uitofp i8 %489 to float
  %491 = fadd float %490, -1.280000e+02
  %492 = getelementptr inbounds nuw i8, ptr %479, i64 %487
  %493 = load i8, ptr %492, align 1, !tbaa !23
  %494 = uitofp i8 %493 to float
  %495 = fadd float %494, -1.280000e+02
  %496 = tail call float @llvm.fmuladd.f32(float 0x3FFC5A1CA0000000, float %491, float %485)
  %497 = fadd float %496, 5.000000e-01
  %498 = tail call fastcc i32 @clamp(float noundef %497) nobuiltin "no-builtins"
  %499 = trunc i32 %498 to i8
  %500 = getelementptr i8, ptr %481, i64 -2
  store i8 %499, ptr %500, align 1, !tbaa !23
  %501 = tail call float @llvm.fmuladd.f32(float 0xBFD6065300000000, float %491, float %485)
  %502 = tail call float @llvm.fmuladd.f32(float 0xBFE6DA33C0000000, float %495, float %501)
  %503 = fadd float %502, 5.000000e-01
  %504 = tail call fastcc i32 @clamp(float noundef %503) nobuiltin "no-builtins"
  %505 = trunc i32 %504 to i8
  %506 = getelementptr i8, ptr %481, i64 -1
  store i8 %505, ptr %506, align 1, !tbaa !23
  %507 = tail call float @llvm.fmuladd.f32(float 0x3FF66E9780000000, float %495, float %485)
  %508 = fadd float %507, 5.000000e-01
  %509 = tail call fastcc i32 @clamp(float noundef %508) nobuiltin "no-builtins"
  %510 = trunc i32 %509 to i8
  store i8 %510, ptr %481, align 1, !tbaa !23
  %511 = add nuw nsw i64 %482, 1
  %512 = getelementptr i8, ptr %481, i64 3
  %exitcond = icmp eq i64 %259, %511
  br i1 %exitcond, label %513, label %480, !llvm.loop !62

513:
  %514 = icmp samesign ult i32 %252, %254
  br i1 %514, label %515, label %520

515:
  br label %516

516:
  %517 = phi i64 [ %274, %515 ], [ %math1, %516 ]
  %518 = getelementptr i8, ptr %273, i64 %517
  store i8 0, ptr %518, align 1, !tbaa !23
  %519 = call { i64, i1 } @llvm.uadd.with.overflow.i64(i64 %517, i64 1)
  %math1 = extractvalue { i64, i1 } %519, 0
  %ov1 = extractvalue { i64, i1 } %519, 1
  br i1 %ov1, label %520, label %516, !llvm.loop !41

520:
  tail call fastcc void @emit(i32 noundef %261, ptr noundef @line, i32 noundef %254) nobuiltin "no-builtins"
  %521 = add nuw nsw i64 %468, 1
  %522 = getelementptr i8, ptr %467, i64 256
  %exitcond23 = icmp eq i64 %521, %wide.trip.count6
  br i1 %exitcond23, label %523, label %466, !llvm.loop !42

523:
  %524 = add nuw nsw i64 %280, 1
  %exitcond24 = icmp eq i64 %524, %wide.trip.count7
  br i1 %exitcond24, label %525, label %279, !llvm.loop !43

525:
  %526 = tail call i64 @llrm_os_close(i32 noundef %261) nobuiltin "no-builtins"
  %527 = load i32, ptr @infile, align 4, !tbaa !14
  %528 = tail call i64 @llrm_os_close(i32 noundef %527) nobuiltin "no-builtins"
  %529 = load i64, ptr @adler_b, align 8, !tbaa !31
  %530 = shl i64 %529, 16
  %531 = load i64, ptr @adler_a, align 8, !tbaa !31
  %532 = or i64 %530, %531
  br label %533

533:
  %534 = phi i64 [ 0, %11 ], [ 0, %25 ], [ 0, %241 ], [ 0, %263 ], [ %532, %525 ], [ 0, %64 ], [ 0, %132 ], [ 0, %157 ], [ 0, %163 ], [ 0, %169 ]
  call void @llvm.lifetime.end.p0(i64 -1, ptr %7) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %6) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %5) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %4) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %3) nounwind
  call void @llvm.lifetime.end.p0(i64 -1, ptr %2) nounwind
  ret i64 %534
}

declare void @llvm.lifetime.start.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

declare i32 @llrm_os_open(ptr noundef, i8 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"

define internal fastcc void @message(ptr noundef %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  br label %2

2:
  %3 = phi i64 [ %7, %2 ], [ 0, %1 ]
  %4 = getelementptr i8, ptr %0, i64 %3
  %5 = load i8, ptr %4, align 1, !tbaa !23
  %6 = icmp eq i8 %5, 0
  %7 = add nuw i64 %3, 1
  br i1 %6, label %8, label %2, !llvm.loop !47

8:
  %9 = add i64 %7, -1
  %10 = tail call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef %0, i64 noundef %9) nobuiltin "no-builtins"
  ret void
}

define internal range(i32 0, 256) i32 @next() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %1 = load i32, ptr @inpos, align 4, !tbaa !14
  %2 = load i32, ptr @inlen, align 4, !tbaa !14
  %3 = icmp eq i32 %1, %2
  br i1 %3, label %4, label %9

4:
  %5 = load i32, ptr @infile, align 4, !tbaa !14
  %6 = tail call i32 @llrm_os_read(i32 noundef %5, ptr noundef @inbuf, i64 noundef 512) nobuiltin "no-builtins"
  store i32 %6, ptr @inlen, align 4, !tbaa !14
  store i32 0, ptr @inpos, align 4, !tbaa !14
  %7 = icmp slt i32 %6, 1
  br i1 %7, label %8, label %9

8:
  store i32 0, ptr @inlen, align 4, !tbaa !14
  store i1 true, ptr @bad, align 4
  br label %16

9:
  %10 = load i32, ptr @inpos, align 4, !tbaa !14
  %11 = add nsw i32 %10, 1
  store i32 %11, ptr @inpos, align 4, !tbaa !14
  %12 = sext i32 %10 to i64
  %13 = getelementptr inbounds i8, ptr @inbuf, i64 %12
  %14 = load i8, ptr %13, align 1, !tbaa !23
  %15 = zext i8 %14 to i32
  br label %16

16:
  %17 = phi i32 [ 0, %8 ], [ %15, %9 ]
  ret i32 %17
}

define internal range(i32 0, 65536) i32 @next16() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %1 = tail call i32 @next() nobuiltin "no-builtins"
  %2 = shl nuw nsw i32 %1, 8
  %3 = tail call i32 @next() nobuiltin "no-builtins"
  %4 = or disjoint i32 %2, %3
  ret i32 %4
}

declare void @llvm.lifetime.end.p0(i64 immarg, ptr nocapture) nocallback nofree nosync nounwind willreturn memory(argmem: write)

define internal fastcc void @put32(i32 noundef range(i32 2, 43) %0, i64 noundef range(i64 -65535, 12884705335) %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" memory(write, argmem: none, inaccessiblemem: none) nofree nosync norecurse willreturn  {
  %3 = zext nneg i32 %0 to i64
  %invariant.gep = getelementptr inbounds nuw i8, ptr @bmp, i64 %3
  br label %4

4:
  %5 = phi i64 [ %10, %4 ], [ 0, %2 ]
  %6 = phi ptr [ %9, %4 ], [ %invariant.gep, %2 ]
  %7 = lshr i64 %1, %5
  %8 = trunc i64 %7 to i8
  store i8 %8, ptr %6, align 1, !tbaa !23
  %9 = getelementptr i8, ptr %6, i64 1
  %10 = add nuw nsw i64 %5, 8
  %exitcond = icmp eq i64 %10, 32
  br i1 %exitcond, label %11, label %4, !llvm.loop !48

11:
  ret void
}

declare i32 @llrm_os_create(ptr noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"

define internal fastcc void @emit(i32 noundef range(i32 0, -2147483648) %0, ptr noundef %1, i32 noundef range(i32 0, 196609) %2) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"  {
  %4 = zext nneg i32 %2 to i64
  %5 = tail call i32 @llrm_os_write_file(i32 noundef %0, ptr noundef %1, i64 noundef %4) nobuiltin "no-builtins"
  %6 = icmp eq i32 %2, 0
  br i1 %6, label %27, label %7

7:
  %promoted1 = load i64, ptr @adler_b, align 8
  %promoted = load i64, ptr @adler_a, align 8
  %wide.trip.count = zext nneg i32 %2 to i64
  %8 = getelementptr i8, ptr %1, i64 %wide.trip.count
  %9 = sub i64 0, %wide.trip.count
  br label %10

10:
  %11 = phi i64 [ %math, %10 ], [ %9, %7 ]
  %12 = phi i64 [ %20, %10 ], [ %promoted, %7 ]
  %13 = phi i64 [ %24, %10 ], [ %promoted1, %7 ]
  %14 = getelementptr i8, ptr %8, i64 %11
  %15 = load i8, ptr %14, align 1, !tbaa !23
  %16 = zext i8 %15 to i64
  %17 = add i64 %12, %16
  %18 = icmp ugt i64 %17, 65520
  %19 = add i64 %17, -65521
  %20 = select i1 %18, i64 %19, i64 %17
  %21 = add i64 %20, %13
  %22 = icmp ugt i64 %21, 65520
  %23 = add i64 %21, -65521
  %24 = select i1 %22, i64 %23, i64 %21
  %25 = call { i64, i1 } @llvm.uadd.with.overflow.i64(i64 %11, i64 1)
  %math = extractvalue { i64, i1 } %25, 0
  %ov = extractvalue { i64, i1 } %25, 1
  br i1 %ov, label %26, label %10, !llvm.loop !49

26:
  store i64 %20, ptr @adler_a, align 8, !tbaa !31
  store i64 %24, ptr @adler_b, align 8, !tbaa !31
  br label %27

27:
  ret void
}

define internal fastcc range(i32 0, 256) i32 @decode(i32 noundef %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"  {
  %2 = tail call i32 @getbit() nobuiltin "no-builtins"
  %3 = sext i32 %0 to i64
  %4 = mul nsw i64 %3, 72
  %5 = getelementptr i8, ptr @maxcode, i64 68
  %6 = getelementptr i8, ptr %5, i64 %4
  br label %7

7:
  %8 = phi i64 [ %17, %13 ], [ -64, %1 ]
  %9 = phi i32 [ %2, %1 ], [ %16, %13 ]
  %10 = getelementptr i8, ptr %6, i64 %8
  %11 = load i32, ptr %10, align 4, !tbaa !14
  %12 = icmp sgt i32 %9, %11
  br i1 %12, label %13, label %19

13:
  %14 = shl nsw i32 %9, 1
  %15 = tail call i32 @getbit() nobuiltin "no-builtins"
  %16 = or disjoint i32 %15, %14
  %17 = add nsw i64 %8, 4
  %exitcond = icmp eq i64 %17, 0
  br i1 %exitcond, label %18, label %7, !llvm.loop !52

18:
  store i1 true, ptr @bad, align 4
  br label %37

19:
  %20 = getelementptr inbounds [256 x i8], ptr @huffval, i64 %3
  %21 = mul nsw i64 %3, 68
  %22 = getelementptr i8, ptr @valptr, i64 68
  %23 = getelementptr i8, ptr %22, i64 %21
  %24 = getelementptr i8, ptr %23, i64 %8
  %25 = load i32, ptr %24, align 4, !tbaa !14
  %26 = add nsw i32 %25, %9
  %27 = mul nsw i64 %3, 68
  %28 = getelementptr i8, ptr @mincode, i64 68
  %29 = getelementptr i8, ptr %28, i64 %27
  %30 = getelementptr i8, ptr %29, i64 %8
  %31 = load i32, ptr %30, align 4, !tbaa !14
  %32 = sub i32 %26, %31
  %33 = sext i32 %32 to i64
  %34 = getelementptr inbounds i8, ptr %20, i64 %33
  %35 = load i8, ptr %34, align 1, !tbaa !23
  %36 = zext i8 %35 to i32
  br label %37

37:
  %38 = phi i32 [ 0, %18 ], [ %36, %19 ]
  ret i32 %38
}

define internal fastcc i32 @extend(i32 noundef %0, i32 noundef range(i32 1, 0) %1) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" memory(none) willreturn nofree nosync norecurse {
  %3 = add nsw i32 %1, -1
  %4 = shl nuw i32 1, %3
  %5 = icmp slt i32 %0, %4
  %6 = shl nsw i32 -1, %1
  %7 = add i32 %0, 1
  %8 = add i32 %7, %6
  %9 = select i1 %5, i32 %8, i32 %0
  ret i32 %9
}

define internal fastcc i32 @receive(i32 noundef range(i32 1, 0) %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  br label %2

2:
  %3 = phi i32 [ 0, %1 ], [ %8, %2 ]
  %4 = phi i32 [ %0, %1 ], [ %5, %2 ]
  %5 = add i32 %4, -1
  %6 = shl nsw i32 %3, 1
  %7 = tail call i32 @getbit() nobuiltin "no-builtins"
  %8 = or disjoint i32 %7, %6
  %9 = icmp eq i32 %5, 0
  br i1 %9, label %10, label %2, !llvm.loop !53

10:
  ret i32 %8
}

declare i64 @llrm_os_close(i32 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"

define i32 @main() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %1 = tail call i64 @bench_jpeg_single() nobuiltin "no-builtins"
  tail call void @report(i64 noundef %1) nobuiltin "no-builtins"
  ret i32 0
}

declare void @report(i64 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"

declare i32 @llrm_os_write_file(i32 noundef, ptr noundef, i64 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"

declare i32 @llrm_os_read(i32 noundef, ptr noundef, i64 noundef) "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv"

define internal range(i32 0, 2) i32 @getbit() nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" {
  %1 = load i32, ptr @bitcnt, align 4, !tbaa !14
  %2 = icmp eq i32 %1, 0
  br i1 %2, label %3, label %11

3:
  %4 = tail call i32 @next() nobuiltin "no-builtins"
  store i32 %4, ptr @bitbuf, align 4, !tbaa !14
  %5 = icmp eq i32 %4, 255
  br i1 %5, label %6, label %10

6:
  %7 = tail call i32 @next() nobuiltin "no-builtins"
  %8 = icmp eq i32 %7, 0
  br i1 %8, label %10, label %9

9:
  store i1 true, ptr @bad, align 4
  br label %10

10:
  store i32 8, ptr @bitcnt, align 4, !tbaa !14
  br label %11

11:
  %12 = load i32, ptr @bitcnt, align 4, !tbaa !14
  %13 = add nsw i32 %12, -1
  store i32 %13, ptr @bitcnt, align 4, !tbaa !14
  %14 = load i32, ptr @bitbuf, align 4, !tbaa !14
  %15 = lshr i32 %14, %13
  %16 = and i32 %15, 1
  ret i32 %16
}

declare float @llvm.fmuladd.f32(float, float, float) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define internal fastcc i32 @clamp(float noundef %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" memory(none) willreturn nofree nosync norecurse {
  %2 = fcmp ole float %0, 0.000000e+00
  %3 = fcmp oge float %0, 2.550000e+02
  %4 = select i1 %3, float 2.550000e+02, float %0
  %5 = fptosi float %4 to i32
  %6 = select i1 %2, i32 0, i32 %5
  ret i32 %6
}

declare i32 @llvm.umin.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare { i64, i1 } @llvm.uadd.with.overflow.i64(i64, i64) nocallback nofree nosync nounwind speculatable willreturn memory(none)

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6, !7}
!llvm.ident = !{!8}
!llvm.errno.tbaa = !{!9}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 8, !"PIC Level", i32 2}
!7 = !{i32 7, !"PIE Level", i32 2}
!8 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!9 = !{!10, !11, i64 0}
!10 = !{!"__libc_errno", !11, i64 0}
!11 = !{!"int", !12, i64 0}
!12 = !{!"omnipotent char", !13, i64 0}
!13 = !{!"Simple C/C++ TBAA"}
!14 = !{!11, !11, i64 0}
!15 = distinct !{!15, !16, !17}
!16 = !{!"llvm.loop.mustprogress"}
!17 = !{!"llvm.loop.unroll.disable"}
!18 = distinct !{!18, !16, !17}
!19 = distinct !{!19, !16, !17}
!20 = distinct !{!20, !16, !17}
!21 = distinct !{!21, !16, !17}
!22 = distinct !{!22, !16, !17}
!23 = !{!12, !12, i64 0}
!24 = distinct !{!24, !16, !17}
!25 = distinct !{!25, !16, !17}
!26 = distinct !{!26, !16, !17}
!27 = distinct !{!27, !16, !17}
!28 = distinct !{!28, !16, !17}
!29 = distinct !{!29, !16, !17}
!30 = distinct !{!30, !16, !17}
!31 = !{!32, !32, i64 0}
!32 = !{!"long", !12, i64 0}
!33 = distinct !{!33, !16, !17}
!34 = distinct !{!34, !16, !17}
!35 = distinct !{!35, !16, !17}
!36 = distinct !{!36, !16, !17}
!37 = distinct !{!37, !16, !17}
!38 = distinct !{!38, !16, !17}
!39 = distinct !{!39, !16, !17}
!40 = distinct !{!40, !16, !17}
!41 = distinct !{!41, !16, !17}
!42 = distinct !{!42, !16, !17}
!43 = distinct !{!43, !16, !17}
!44 = !{!45, !45, i64 0}
!45 = !{!"p1 omnipotent char", !46, i64 0}
!46 = !{!"any pointer", !12, i64 0}
!47 = distinct !{!47, !16, !17}
!48 = distinct !{!48, !16, !17}
!49 = distinct !{!49, !16, !17}
!50 = distinct !{!50, !16, !17}
!51 = distinct !{!51, !16, !17}
!52 = distinct !{!52, !16, !17}
!53 = distinct !{!53, !16, !17}
!54 = !{!55, !55, i64 0}
!55 = !{!"float", !12, i64 0}
!56 = distinct !{!56, !16, !17}
!57 = distinct !{!57, !16, !17}
!58 = distinct !{!58, !16, !17}
!59 = distinct !{!59, !16, !17}
!60 = distinct !{!60, !16, !17}
!61 = distinct !{!61, !16, !17}
!62 = distinct !{!62, !16, !17}
