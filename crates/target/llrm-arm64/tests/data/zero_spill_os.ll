; ModuleID = 'out/src/jpeg_single.c'
source_filename = "out/src/jpeg_single.c"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

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
@bmp = internal global [54 x i8] zeroinitializer, align 4
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
@cosine = internal unnamed_addr constant [8 x [8 x float]] [[8 x float] [float f0x3EB504F3, float f0x3EFB14BE, float f0x3EEC835E, float f0x3ED4DB31, float f0x3EB504F3, float f0x3E8E39DA, float f0x3E43EF15, float f0x3DC7C5C2], [8 x float] [float f0x3EB504F3, float f0x3ED4DB31, float f0x3E43EF15, float f0xBDC7C5C2, float f0xBEB504F3, float f0xBEFB14BE, float f0xBEEC835E, float f0xBE8E39DA], [8 x float] [float f0x3EB504F3, float f0x3E8E39DA, float f0xBE43EF15, float f0xBEFB14BE, float f0xBEB504F3, float f0x3DC7C5C2, float f0x3EEC835E, float f0x3ED4DB31], [8 x float] [float f0x3EB504F3, float f0x3DC7C5C2, float f0xBEEC835E, float f0xBE8E39DA, float f0x3EB504F3, float f0x3ED4DB31, float f0xBE43EF15, float f0xBEFB14BE], [8 x float] [float f0x3EB504F3, float f0xBDC7C5C2, float f0xBEEC835E, float f0x3E8E39DA, float f0x3EB504F3, float f0xBED4DB31, float f0xBE43EF15, float f0x3EFB14BE], [8 x float] [float f0x3EB504F3, float f0xBE8E39DA, float f0xBE43EF15, float f0x3EFB14BE, float f0xBEB504F3, float f0xBDC7C5C2, float f0x3EEC835E, float f0xBED4DB31], [8 x float] [float f0x3EB504F3, float f0xBED4DB31, float f0x3E43EF15, float f0x3DC7C5C2, float f0xBEB504F3, float f0x3EFB14BE, float f0xBEEC835E, float f0x3E8E39DA], [8 x float] [float f0x3EB504F3, float f0xBEFB14BE, float f0x3EEC835E, float f0xBED4DB31, float f0x3EB504F3, float f0xBE8E39DA, float f0x3E43EF15, float f0xBDC7C5C2]], align 4
@planes = internal unnamed_addr global [3 x [16 x [256 x i8]]] zeroinitializer, align 1

; Function Attrs: noinline nounwind optsize
define dso_local i64 @bench_jpeg_single() local_unnamed_addr #0 {
  %1 = alloca [64 x float], align 4
  %2 = alloca [3 x i32], align 4
  %3 = alloca [3 x i32], align 4
  %4 = alloca [3 x i32], align 4
  %5 = alloca [3 x i32], align 4
  %6 = alloca [3 x i32], align 4
  %7 = alloca [3 x i32], align 4
  %8 = alloca [17 x i32], align 4
  call void @llvm.lifetime.start.p0(ptr nonnull %2) #6
  call void @llvm.lifetime.start.p0(ptr nonnull %3) #6
  call void @llvm.lifetime.start.p0(ptr nonnull %4) #6
  call void @llvm.lifetime.start.p0(ptr nonnull %5) #6
  call void @llvm.lifetime.start.p0(ptr nonnull %6) #6
  call void @llvm.lifetime.start.p0(ptr nonnull %7) #6
  %9 = tail call i32 @llrm_os_open(ptr noundef nonnull @.str, i8 noundef 0) #7
  store i32 %9, ptr @infile, align 4, !tbaa !13
  %10 = icmp slt i32 %9, 0
  br i1 %10, label %11, label %13

11:                                               ; preds = %0
  %12 = tail call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef nonnull @.str.1, i64 noundef 21) #7
  br label %630

13:                                               ; preds = %0
  %14 = tail call fastcc i32 @next() #8
  %15 = icmp eq i32 %14, 255
  br i1 %15, label %16, label %21

16:                                               ; preds = %13
  %17 = tail call fastcc i32 @next() #8
  %18 = icmp eq i32 %17, 216
  br i1 %18, label %19, label %21

19:                                               ; preds = %16
  %20 = load i1, ptr @bad, align 4
  br i1 %20, label %235, label %29

21:                                               ; preds = %16, %13
  %22 = tail call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef nonnull @.str.2, i64 noundef 12) #7
  br label %630

23:                                               ; preds = %230, %217
  %24 = phi i32 [ %221, %217 ], [ %226, %230 ]
  %25 = phi i32 [ %220, %217 ], [ %227, %230 ]
  %26 = phi i32 [ %219, %217 ], [ %228, %230 ]
  %27 = phi i32 [ %218, %217 ], [ %229, %230 ]
  %28 = load i1, ptr @bad, align 4
  br i1 %28, label %235, label %29

29:                                               ; preds = %19, %23
  %30 = phi i32 [ %27, %23 ], [ 0, %19 ]
  %31 = phi i32 [ %26, %23 ], [ 0, %19 ]
  %32 = phi i32 [ %25, %23 ], [ 0, %19 ]
  %33 = phi i32 [ %24, %23 ], [ 0, %19 ]
  br label %39

34:                                               ; preds = %212, %191
  %35 = tail call fastcc i32 @next() #8
  %36 = tail call fastcc i32 @next() #8
  %37 = tail call fastcc i32 @next() #8
  %38 = load i1, ptr @bad, align 4
  br label %235, !llvm.loop !14

39:                                               ; preds = %44, %29
  %40 = tail call fastcc i32 @next() #8
  %41 = icmp eq i32 %40, 255
  br i1 %41, label %45, label %42

42:                                               ; preds = %39
  %43 = load i1, ptr @bad, align 4
  br i1 %43, label %45, label %44

44:                                               ; preds = %42, %215
  br label %39, !llvm.loop !14

45:                                               ; preds = %39, %42
  br label %46

46:                                               ; preds = %45, %46
  %47 = tail call fastcc i32 @next() #8
  %48 = freeze i32 %47
  %49 = icmp eq i32 %48, 255
  br i1 %49, label %46, label %50, !llvm.loop !16

50:                                               ; preds = %46
  %51 = tail call fastcc i32 @next() #8
  %52 = shl nuw nsw i32 %51, 8
  %53 = tail call fastcc i32 @next() #8
  %54 = or disjoint i32 %52, %53
  %55 = add nsw i32 %54, -2
  %56 = trunc nuw i32 %48 to i8
  switch i8 %56, label %171 [
    i8 -37, label %61
    i8 -60, label %59
    i8 -64, label %137
    i8 -62, label %57
  ]

57:                                               ; preds = %50
  %58 = tail call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef nonnull @.str.5, i64 noundef 32) #7
  br label %630

59:                                               ; preds = %50
  %60 = icmp samesign ugt i32 %54, 2
  br i1 %60, label %82, label %215

61:                                               ; preds = %50
  %62 = icmp samesign ugt i32 %54, 2
  br i1 %62, label %63, label %215

63:                                               ; preds = %61, %79
  %64 = phi i32 [ %80, %79 ], [ %55, %61 ]
  %65 = tail call fastcc i32 @next() #8
  %66 = icmp samesign ult i32 %65, 16
  br i1 %66, label %69, label %67

67:                                               ; preds = %63
  %68 = tail call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef nonnull @.str.3, i64 noundef 42) #7
  br label %630

69:                                               ; preds = %63
  %70 = and i32 %65, 3
  %71 = zext nneg i32 %70 to i64
  %72 = getelementptr inbounds nuw [256 x i8], ptr @quant, i64 %71
  br label %73

73:                                               ; preds = %69, %73
  %74 = phi i64 [ 0, %69 ], [ %77, %73 ]
  %75 = tail call fastcc i32 @next() #8
  %76 = getelementptr inbounds nuw [4 x i8], ptr %72, i64 %74
  store i32 %75, ptr %76, align 4, !tbaa !13
  %77 = add nuw nsw i64 %74, 1
  %78 = icmp eq i64 %77, 64
  br i1 %78, label %79, label %73, !llvm.loop !17

79:                                               ; preds = %73
  %80 = add nsw i32 %64, -65
  %81 = icmp sgt i32 %64, 65
  br i1 %81, label %63, label %215, !llvm.loop !18

82:                                               ; preds = %59, %133
  %83 = phi i32 [ %135, %133 ], [ %55, %59 ]
  %84 = load i1, ptr @bad, align 4
  br i1 %84, label %224, label %85

85:                                               ; preds = %82
  call void @llvm.lifetime.start.p0(ptr nonnull %8) #6
  %86 = tail call fastcc i32 @next() #8
  br label %96

87:                                               ; preds = %96
  %88 = lshr i32 %86, 3
  %89 = and i32 %88, 30
  %90 = and i32 %86, 1
  %91 = or disjoint i32 %89, %90
  %92 = zext nneg i32 %91 to i64
  %93 = getelementptr inbounds nuw [68 x i8], ptr @valptr, i64 %92
  %94 = getelementptr inbounds nuw [68 x i8], ptr @mincode, i64 %92
  %95 = getelementptr inbounds nuw [72 x i8], ptr @maxcode, i64 %92
  br label %109

96:                                               ; preds = %85, %96
  %97 = phi i64 [ 1, %85 ], [ %102, %96 ]
  %98 = phi i32 [ 0, %85 ], [ %101, %96 ]
  %99 = tail call fastcc i32 @next() #8
  %100 = getelementptr inbounds nuw [4 x i8], ptr %8, i64 %97
  store i32 %99, ptr %100, align 4, !tbaa !13
  %101 = add i32 %99, %98
  %102 = add nuw nsw i64 %97, 1
  %103 = icmp eq i64 %102, 17
  br i1 %103, label %87, label %96, !llvm.loop !19

104:                                              ; preds = %109
  %105 = icmp eq i32 %101, 0
  br i1 %105, label %133, label %106

106:                                              ; preds = %104
  %107 = getelementptr inbounds nuw [256 x i8], ptr @huffval, i64 %92
  %108 = zext i32 %101 to i64
  br label %126

109:                                              ; preds = %87, %109
  %110 = phi i64 [ 1, %87 ], [ %124, %109 ]
  %111 = phi i32 [ 0, %87 ], [ %118, %109 ]
  %112 = phi i32 [ 0, %87 ], [ %123, %109 ]
  %113 = getelementptr inbounds nuw [4 x i8], ptr %93, i64 %110
  store i32 %111, ptr %113, align 4, !tbaa !13
  %114 = getelementptr inbounds nuw [4 x i8], ptr %94, i64 %110
  store i32 %112, ptr %114, align 4, !tbaa !13
  %115 = getelementptr inbounds nuw [4 x i8], ptr %8, i64 %110
  %116 = load i32, ptr %115, align 4, !tbaa !13
  %117 = add nsw i32 %116, %112
  %118 = add nsw i32 %116, %111
  %119 = icmp eq i32 %116, 0
  %120 = add nsw i32 %117, -1
  %121 = select i1 %119, i32 -1, i32 %120
  %122 = getelementptr inbounds nuw [4 x i8], ptr %95, i64 %110
  store i32 %121, ptr %122, align 4, !tbaa !13
  %123 = shl nsw i32 %117, 1
  %124 = add nuw nsw i64 %110, 1
  %125 = icmp eq i64 %124, 17
  br i1 %125, label %104, label %109, !llvm.loop !20

126:                                              ; preds = %106, %126
  %127 = phi i64 [ 0, %106 ], [ %131, %126 ]
  %128 = tail call fastcc i32 @next() #8
  %129 = trunc nuw i32 %128 to i8
  %130 = getelementptr inbounds nuw i8, ptr %107, i64 %127
  store i8 %129, ptr %130, align 1, !tbaa !21
  %131 = add nuw nsw i64 %127, 1
  %132 = icmp eq i64 %131, %108
  br i1 %132, label %133, label %126, !llvm.loop !22

133:                                              ; preds = %126, %104
  %134 = add nsw i32 %83, -17
  %135 = sub i32 %134, %101
  call void @llvm.lifetime.end.p0(ptr nonnull %8) #6
  %136 = icmp sgt i32 %135, 0
  br i1 %136, label %82, label %215, !llvm.loop !23

137:                                              ; preds = %50
  %138 = tail call fastcc i32 @next() #8
  %139 = icmp eq i32 %138, 8
  br i1 %139, label %142, label %140

140:                                              ; preds = %137
  %141 = tail call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef nonnull @.str.4, i64 noundef 34) #7
  br label %630

142:                                              ; preds = %137
  %143 = tail call fastcc i32 @next() #8
  %144 = shl nuw nsw i32 %143, 8
  %145 = tail call fastcc i32 @next() #8
  %146 = or disjoint i32 %144, %145
  %147 = tail call fastcc i32 @next() #8
  %148 = shl nuw nsw i32 %147, 8
  %149 = tail call fastcc i32 @next() #8
  %150 = or disjoint i32 %148, %149
  %151 = tail call fastcc i32 @next() #8
  %152 = tail call i32 @llvm.umin.i32(i32 %151, i32 3)
  %153 = icmp eq i32 %151, 0
  br i1 %153, label %167, label %154

154:                                              ; preds = %142
  %155 = zext nneg i32 %152 to i64
  br label %156

156:                                              ; preds = %154, %156
  %157 = phi i64 [ 0, %154 ], [ %165, %156 ]
  %158 = tail call fastcc i32 @next() #8
  %159 = getelementptr inbounds nuw [4 x i8], ptr %2, i64 %157
  store i32 %158, ptr %159, align 4, !tbaa !13
  %160 = tail call fastcc i32 @next() #8
  %161 = getelementptr inbounds nuw [4 x i8], ptr %3, i64 %157
  store i32 %160, ptr %161, align 4, !tbaa !13
  %162 = tail call fastcc i32 @next() #8
  %163 = and i32 %162, 3
  %164 = getelementptr inbounds nuw [4 x i8], ptr %4, i64 %157
  store i32 %163, ptr %164, align 4, !tbaa !13
  %165 = add nuw nsw i64 %157, 1
  %166 = icmp eq i64 %165, %155
  br i1 %166, label %167, label %156, !llvm.loop !24

167:                                              ; preds = %156, %142
  %168 = mul nsw i32 %152, -3
  %169 = add nsw i32 %54, -8
  %170 = add nsw i32 %169, %168
  br label %217

171:                                              ; preds = %50
  %172 = add nsw i32 %48, -201
  %173 = icmp ult i32 %172, 7
  %174 = icmp ne i32 %48, 204
  %175 = and i1 %174, %173
  br i1 %175, label %176, label %178

176:                                              ; preds = %171
  %177 = tail call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef nonnull @.str.6, i64 noundef 37) #7
  br label %630

178:                                              ; preds = %171
  %179 = add i32 %48, -193
  %180 = icmp ult i32 %179, 15
  br i1 %180, label %181, label %184

181:                                              ; preds = %178
  switch i8 %56, label %182 [
    i8 -56, label %217
    i8 -52, label %217
  ]

182:                                              ; preds = %181
  %183 = tail call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef nonnull @.str.7, i64 noundef 33) #7
  br label %630

184:                                              ; preds = %178
  switch i32 %48, label %217 [
    i32 221, label %185
    i32 218, label %191
  ]

185:                                              ; preds = %184
  %186 = tail call fastcc i32 @next() #8
  %187 = shl nuw nsw i32 %186, 8
  %188 = tail call fastcc i32 @next() #8
  %189 = or disjoint i32 %187, %188
  %190 = add nsw i32 %54, -4
  br label %217

191:                                              ; preds = %184
  %192 = tail call fastcc i32 @next() #8
  %193 = icmp eq i32 %192, 0
  br i1 %193, label %34, label %194

194:                                              ; preds = %191, %212
  %195 = phi i32 [ %213, %212 ], [ 0, %191 ]
  %196 = tail call fastcc i32 @next() #8
  %197 = tail call fastcc i32 @next() #8
  %198 = lshr i32 %197, 4
  %199 = and i32 %197, 1
  %200 = or disjoint i32 %199, 2
  br label %201

201:                                              ; preds = %194, %209
  %202 = phi i64 [ 0, %194 ], [ %210, %209 ]
  %203 = getelementptr inbounds nuw [4 x i8], ptr %2, i64 %202
  %204 = load i32, ptr %203, align 4, !tbaa !13
  %205 = icmp eq i32 %204, %196
  br i1 %205, label %206, label %209

206:                                              ; preds = %201
  %207 = getelementptr inbounds nuw [4 x i8], ptr %5, i64 %202
  store i32 %198, ptr %207, align 4, !tbaa !13
  %208 = getelementptr inbounds nuw [4 x i8], ptr %6, i64 %202
  store i32 %200, ptr %208, align 4, !tbaa !13
  br label %209

209:                                              ; preds = %201, %206
  %210 = add nuw nsw i64 %202, 1
  %211 = icmp eq i64 %210, 3
  br i1 %211, label %212, label %201, !llvm.loop !25

212:                                              ; preds = %209
  %213 = add nuw nsw i32 %195, 1
  %214 = icmp eq i32 %213, %192
  br i1 %214, label %34, label %194, !llvm.loop !26

215:                                              ; preds = %133, %79, %61, %59
  %216 = load i1, ptr @bad, align 4
  br i1 %216, label %235, label %44

217:                                              ; preds = %181, %181, %184, %185, %167
  %218 = phi i32 [ %30, %181 ], [ %30, %184 ], [ %30, %181 ], [ %150, %167 ], [ %30, %185 ]
  %219 = phi i32 [ %31, %181 ], [ %31, %184 ], [ %31, %181 ], [ %146, %167 ], [ %31, %185 ]
  %220 = phi i32 [ %32, %181 ], [ %32, %184 ], [ %32, %181 ], [ %32, %167 ], [ %189, %185 ]
  %221 = phi i32 [ %33, %181 ], [ %33, %184 ], [ %33, %181 ], [ %151, %167 ], [ %33, %185 ]
  %222 = phi i32 [ %55, %181 ], [ %55, %184 ], [ %55, %181 ], [ %170, %167 ], [ %190, %185 ]
  %223 = icmp sgt i32 %222, 0
  br i1 %223, label %224, label %23

224:                                              ; preds = %82, %217
  %225 = phi i32 [ %222, %217 ], [ %83, %82 ]
  %226 = phi i32 [ %221, %217 ], [ %33, %82 ]
  %227 = phi i32 [ %220, %217 ], [ %32, %82 ]
  %228 = phi i32 [ %219, %217 ], [ %31, %82 ]
  %229 = phi i32 [ %218, %217 ], [ %30, %82 ]
  br label %230

230:                                              ; preds = %224, %230
  %231 = phi i32 [ %232, %230 ], [ %225, %224 ]
  %232 = add nsw i32 %231, -1
  %233 = tail call fastcc i32 @next() #8
  %234 = icmp samesign ugt i32 %231, 1
  br i1 %234, label %230, label %23, !llvm.loop !27

235:                                              ; preds = %23, %215, %19, %34
  %236 = phi i32 [ %33, %34 ], [ %33, %215 ], [ 0, %19 ], [ %24, %23 ]
  %237 = phi i32 [ %32, %34 ], [ %32, %215 ], [ 0, %19 ], [ %25, %23 ]
  %238 = phi i32 [ %31, %34 ], [ %31, %215 ], [ 0, %19 ], [ %26, %23 ]
  %239 = phi i32 [ %30, %34 ], [ %30, %215 ], [ 0, %19 ], [ %27, %23 ]
  %240 = phi i1 [ %38, %34 ], [ true, %215 ], [ true, %19 ], [ true, %23 ]
  %241 = load i32, ptr %3, align 4, !tbaa !13
  %242 = ashr i32 %241, 4
  %243 = and i32 %241, 15
  %244 = icmp ne i32 %236, 3
  %245 = select i1 %240, i1 true, i1 %244
  %246 = add nsw i32 %242, -3
  %247 = icmp ult i32 %246, -2
  %248 = select i1 %245, i1 true, i1 %247
  %249 = add nsw i32 %243, -3
  %250 = icmp ult i32 %249, -2
  %251 = select i1 %248, i1 true, i1 %250
  %252 = getelementptr inbounds nuw i8, ptr %3, i64 4
  %253 = load i32, ptr %252, align 4
  %254 = icmp ne i32 %253, 17
  %255 = select i1 %251, i1 true, i1 %254
  %256 = getelementptr inbounds nuw i8, ptr %3, i64 8
  %257 = load i32, ptr %256, align 4
  %258 = icmp ne i32 %257, 17
  %259 = select i1 %255, i1 true, i1 %258
  %260 = add nsw i32 %239, -257
  %261 = icmp ult i32 %260, -256
  %262 = select i1 %259, i1 true, i1 %261
  %263 = icmp slt i32 %238, 1
  %264 = select i1 %262, i1 true, i1 %263
  br i1 %264, label %265, label %267

265:                                              ; preds = %235
  %266 = tail call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef nonnull @.str.8, i64 noundef 18) #7
  br label %630

267:                                              ; preds = %235
  %268 = mul nuw nsw i32 %239, 3
  %269 = add nuw nsw i32 %268, 3
  %270 = and i32 %269, 4092
  %271 = zext nneg i32 %270 to i64
  %272 = zext nneg i32 %238 to i64
  %273 = mul nuw nsw i64 %271, %272
  %274 = add nuw nsw i64 %273, 54
  store i8 66, ptr @bmp, align 4, !tbaa !21
  store i8 77, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 1), align 1, !tbaa !21
  br label %275

275:                                              ; preds = %275, %267
  %276 = phi i64 [ 0, %267 ], [ %281, %275 ]
  %277 = shl nuw nsw i64 %276, 3
  %278 = lshr i64 %274, %277
  %279 = trunc i64 %278 to i8
  %280 = getelementptr inbounds nuw i8, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 2), i64 %276
  store i8 %279, ptr %280, align 1, !tbaa !21
  %281 = add nuw nsw i64 %276, 1
  %282 = icmp eq i64 %281, 4
  br i1 %282, label %283, label %275, !llvm.loop !28

283:                                              ; preds = %275
  store i8 54, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 10), align 2, !tbaa !21
  store i8 0, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 11), align 1, !tbaa !21
  store i8 0, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 12), align 4, !tbaa !21
  store i8 0, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 13), align 1, !tbaa !21
  store i8 40, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 14), align 2, !tbaa !21
  store i8 0, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 15), align 1, !tbaa !21
  store i8 0, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 16), align 4, !tbaa !21
  store i8 0, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 17), align 1, !tbaa !21
  %284 = zext nneg i32 %239 to i64
  br label %285

285:                                              ; preds = %285, %283
  %286 = phi i64 [ 0, %283 ], [ %291, %285 ]
  %287 = shl nuw nsw i64 %286, 3
  %288 = lshr i64 %284, %287
  %289 = trunc i64 %288 to i8
  %290 = getelementptr inbounds nuw i8, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 18), i64 %286
  store i8 %289, ptr %290, align 1, !tbaa !21
  %291 = add nuw nsw i64 %286, 1
  %292 = icmp eq i64 %291, 4
  br i1 %292, label %293, label %285, !llvm.loop !28

293:                                              ; preds = %285
  %294 = sub nsw i64 0, %272
  br label %295

295:                                              ; preds = %295, %293
  %296 = phi i64 [ 0, %293 ], [ %301, %295 ]
  %297 = shl nuw nsw i64 %296, 3
  %298 = lshr i64 %294, %297
  %299 = trunc i64 %298 to i8
  %300 = getelementptr inbounds nuw i8, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 22), i64 %296
  store i8 %299, ptr %300, align 1, !tbaa !21
  %301 = add nuw nsw i64 %296, 1
  %302 = icmp eq i64 %301, 4
  br i1 %302, label %303, label %295, !llvm.loop !28

303:                                              ; preds = %295
  store i8 1, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 26), align 2, !tbaa !21
  store i8 24, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 28), align 4, !tbaa !21
  br label %304

304:                                              ; preds = %304, %303
  %305 = phi i64 [ 0, %303 ], [ %310, %304 ]
  %306 = shl nuw nsw i64 %305, 3
  %307 = lshr i64 %273, %306
  %308 = trunc i64 %307 to i8
  %309 = getelementptr inbounds nuw i8, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 34), i64 %305
  store i8 %308, ptr %309, align 1, !tbaa !21
  %310 = add nuw nsw i64 %305, 1
  %311 = icmp eq i64 %310, 4
  br i1 %311, label %312, label %304, !llvm.loop !28

312:                                              ; preds = %304
  store i8 19, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 38), align 2, !tbaa !21
  store i8 11, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 39), align 1, !tbaa !21
  store i8 0, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 40), align 4, !tbaa !21
  store i8 0, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 41), align 1, !tbaa !21
  store i8 19, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 42), align 2, !tbaa !21
  store i8 11, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 43), align 1, !tbaa !21
  store i8 0, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 44), align 4, !tbaa !21
  store i8 0, ptr getelementptr inbounds nuw (i8, ptr @bmp, i64 45), align 1, !tbaa !21
  %313 = shl nuw nsw i32 %242, 3
  %314 = add nsw i32 %239, -1
  %315 = add nsw i32 %314, %313
  %316 = sdiv i32 %315, %313
  %317 = tail call i32 @llrm_os_create(ptr noundef nonnull @.str.9) #7
  %318 = icmp slt i32 %317, 0
  br i1 %318, label %319, label %321

319:                                              ; preds = %312
  %320 = tail call i32 @llrm_os_write_file(i32 noundef 1, ptr noundef nonnull @.str.10, i64 noundef 22) #7
  br label %630

321:                                              ; preds = %312
  %322 = add nsw i32 %238, -1
  %323 = shl nuw nsw i32 %243, 3
  %324 = add nuw nsw i32 %322, %323
  %325 = udiv i32 %324, %323
  tail call fastcc void @emit(i32 noundef %317, ptr noundef nonnull @bmp, i32 noundef 54) #8
  store i32 0, ptr %7, align 4, !tbaa !13
  %326 = getelementptr inbounds nuw i8, ptr %7, i64 4
  store i32 0, ptr %326, align 4, !tbaa !13
  %327 = getelementptr inbounds nuw i8, ptr %7, i64 8
  store i32 0, ptr %327, align 4, !tbaa !13
  %328 = icmp sgt i32 %316, 0
  %329 = icmp ne i32 %237, 0
  %330 = add nsw i32 %242, -1
  %331 = add nsw i32 %243, -1
  %332 = icmp samesign ult i32 %268, %270
  %333 = zext nneg i32 %268 to i64
  %334 = zext nneg i32 %323 to i64
  %335 = zext nneg i32 %325 to i64
  br label %336

336:                                              ; preds = %321, %619
  %337 = phi i64 [ 0, %321 ], [ %620, %619 ]
  %338 = phi i32 [ %237, %321 ], [ %549, %619 ]
  %339 = load i1, ptr @bad, align 4
  br i1 %339, label %622, label %340

340:                                              ; preds = %336
  br i1 %328, label %343, label %548

341:                                              ; preds = %544
  %342 = load i1, ptr @bad, align 4
  br i1 %342, label %548, label %343

343:                                              ; preds = %340, %341
  %344 = phi i32 [ %546, %341 ], [ 0, %340 ]
  %345 = phi i32 [ %545, %341 ], [ %338, %340 ]
  %346 = icmp eq i32 %345, 0
  %347 = select i1 %329, i1 %346, i1 false
  br i1 %347, label %348, label %362

348:                                              ; preds = %343
  store i32 0, ptr @bitcnt, align 4, !tbaa !13
  br label %349

349:                                              ; preds = %354, %348
  %350 = tail call fastcc i32 @next() #8
  %351 = icmp eq i32 %350, 255
  br i1 %351, label %355, label %352

352:                                              ; preds = %349
  %353 = load i1, ptr @bad, align 4
  br i1 %353, label %355, label %354

354:                                              ; preds = %352, %355
  br label %349, !llvm.loop !29

355:                                              ; preds = %352, %349
  %356 = tail call fastcc i32 @next() #8
  %357 = load i1, ptr @bad, align 4
  %358 = and i32 %356, 248
  %359 = icmp eq i32 %358, 208
  %360 = select i1 %357, i1 true, i1 %359
  br i1 %360, label %361, label %354

361:                                              ; preds = %355
  store i32 0, ptr %7, align 4, !tbaa !13
  store i32 0, ptr %326, align 4, !tbaa !13
  store i32 0, ptr %327, align 4, !tbaa !13
  br label %362

362:                                              ; preds = %361, %343
  %363 = phi i32 [ %345, %343 ], [ %237, %361 ]
  br label %364

364:                                              ; preds = %362, %541
  %365 = phi i64 [ 0, %362 ], [ %542, %541 ]
  %366 = icmp eq i64 %365, 0
  %367 = select i1 %366, i32 %242, i32 1
  %368 = select i1 %366, i32 %243, i32 1
  %369 = icmp eq i32 %368, 0
  br i1 %369, label %541, label %370

370:                                              ; preds = %364
  %371 = icmp sgt i32 %367, 0
  %372 = getelementptr inbounds nuw [4 x i8], ptr %7, i64 %365
  %373 = getelementptr inbounds nuw [4 x i8], ptr %4, i64 %365
  %374 = getelementptr inbounds nuw [4 x i8], ptr %6, i64 %365
  %375 = getelementptr inbounds nuw [4096 x i8], ptr @planes, i64 %365
  br i1 %371, label %376, label %541

376:                                              ; preds = %370
  %377 = mul nuw nsw i32 %367, %344
  %378 = getelementptr inbounds nuw [4 x i8], ptr %5, i64 %365
  %379 = load i32, ptr %378, align 4, !tbaa !13
  %380 = zext nneg i32 %368 to i64
  %381 = zext nneg i32 %367 to i64
  br label %382

382:                                              ; preds = %376, %538
  %383 = phi i64 [ 0, %376 ], [ %539, %538 ]
  %384 = shl i64 %383, 11
  %385 = getelementptr i8, ptr %375, i64 %384
  br label %386

386:                                              ; preds = %382, %535
  %387 = phi i64 [ 0, %382 ], [ %536, %535 ]
  br label %388

388:                                              ; preds = %386, %388
  %389 = phi i64 [ 0, %386 ], [ %391, %388 ]
  %390 = getelementptr inbounds nuw [8 x i8], ptr @coef, i64 %389
  store i64 0, ptr %390, align 8, !tbaa !30
  %391 = add nuw nsw i64 %389, 1
  %392 = icmp eq i64 %391, 64
  br i1 %392, label %393, label %388, !llvm.loop !32

393:                                              ; preds = %388
  %394 = tail call fastcc i32 @decode(i32 noundef %379) #8
  %395 = icmp eq i32 %394, 0
  br i1 %395, label %396, label %398

396:                                              ; preds = %393
  %397 = load i32, ptr %372, align 4, !tbaa !13
  br label %416

398:                                              ; preds = %393, %398
  %399 = phi i32 [ %404, %398 ], [ 0, %393 ]
  %400 = phi i32 [ %401, %398 ], [ %394, %393 ]
  %401 = add nsw i32 %400, -1
  %402 = shl nsw i32 %399, 1
  %403 = tail call fastcc i32 @getbit() #8
  %404 = or disjoint i32 %403, %402
  %405 = icmp eq i32 %401, 0
  br i1 %405, label %406, label %398, !llvm.loop !33

406:                                              ; preds = %398
  %407 = add nsw i32 %394, -1
  %408 = shl nuw i32 1, %407
  %409 = icmp slt i32 %404, %408
  %410 = shl nsw i32 -1, %394
  %411 = add nuw nsw i32 %410, 1
  %412 = select i1 %409, i32 %411, i32 0
  %413 = add i32 %404, %412
  %414 = load i32, ptr %372, align 4, !tbaa !13
  %415 = add nsw i32 %414, %413
  store i32 %415, ptr %372, align 4, !tbaa !13
  br label %416

416:                                              ; preds = %396, %406
  %417 = phi i32 [ %397, %396 ], [ %415, %406 ]
  %418 = sext i32 %417 to i64
  %419 = load i32, ptr %373, align 4, !tbaa !13
  %420 = sext i32 %419 to i64
  %421 = getelementptr inbounds [256 x i8], ptr @quant, i64 %420
  %422 = load i32, ptr %421, align 4, !tbaa !13
  %423 = sext i32 %422 to i64
  %424 = mul nsw i64 %423, %418
  store i64 %424, ptr @coef, align 8, !tbaa !30
  br label %425

425:                                              ; preds = %416, %467
  %426 = phi i32 [ 1, %416 ], [ %468, %467 ]
  %427 = load i1, ptr @bad, align 4
  br i1 %427, label %470, label %428

428:                                              ; preds = %425
  %429 = load i32, ptr %374, align 4, !tbaa !13
  %430 = tail call fastcc i32 @decode(i32 noundef %429) #8
  %431 = lshr i32 %430, 4
  %432 = and i32 %430, 15
  %433 = icmp eq i32 %432, 0
  br i1 %433, label %434, label %437

434:                                              ; preds = %428
  %435 = icmp eq i32 %431, 15
  %436 = add nsw i32 %426, 16
  br i1 %435, label %467, label %470

437:                                              ; preds = %428
  %438 = add nsw i32 %431, %426
  %439 = icmp sgt i32 %438, 63
  br i1 %439, label %470, label %440

440:                                              ; preds = %437, %440
  %441 = phi i32 [ %446, %440 ], [ 0, %437 ]
  %442 = phi i32 [ %443, %440 ], [ %432, %437 ]
  %443 = add nsw i32 %442, -1
  %444 = shl nsw i32 %441, 1
  %445 = tail call fastcc i32 @getbit() #8
  %446 = or disjoint i32 %445, %444
  %447 = icmp eq i32 %443, 0
  br i1 %447, label %448, label %440, !llvm.loop !33

448:                                              ; preds = %440
  %449 = add nsw i32 %432, -1
  %450 = shl nuw nsw i32 1, %449
  %451 = icmp slt i32 %446, %450
  %452 = shl nsw i32 -1, %432
  %453 = or disjoint i32 %452, 1
  %454 = select i1 %451, i32 %453, i32 0
  %455 = add i32 %446, %454
  %456 = sext i32 %455 to i64
  %457 = sext i32 %438 to i64
  %458 = getelementptr inbounds [4 x i8], ptr %421, i64 %457
  %459 = load i32, ptr %458, align 4, !tbaa !13
  %460 = sext i32 %459 to i64
  %461 = mul nsw i64 %460, %456
  %462 = getelementptr inbounds i8, ptr @zigzag, i64 %457
  %463 = load i8, ptr %462, align 1, !tbaa !21
  %464 = zext i8 %463 to i64
  %465 = getelementptr inbounds nuw [8 x i8], ptr @coef, i64 %464
  store i64 %461, ptr %465, align 8, !tbaa !30
  %466 = add nsw i32 %438, 1
  br label %467

467:                                              ; preds = %434, %448
  %468 = phi i32 [ %466, %448 ], [ %436, %434 ]
  %469 = icmp slt i32 %468, 64
  br i1 %469, label %425, label %470

470:                                              ; preds = %434, %437, %467, %425
  call void @llvm.lifetime.start.p0(ptr nonnull %1) #6
  br label %471

471:                                              ; preds = %500, %470
  %472 = phi i64 [ 0, %470 ], [ %501, %500 ]
  %473 = shl nuw nsw i64 %472, 3
  %474 = getelementptr inbounds nuw [8 x i8], ptr @coef, i64 %473
  %475 = getelementptr inbounds nuw [4 x i8], ptr %1, i64 %473
  br label %482

476:                                              ; preds = %500
  %477 = trunc i64 %387 to i32
  %478 = add i32 %377, %477
  %479 = shl nsw i32 %478, 3
  %480 = zext nneg i32 %479 to i64
  %481 = getelementptr i8, ptr %385, i64 %480
  br label %503

482:                                              ; preds = %496, %471
  %483 = phi i64 [ 0, %471 ], [ %498, %496 ]
  %484 = getelementptr inbounds nuw [32 x i8], ptr @cosine, i64 %483
  br label %485

485:                                              ; preds = %485, %482
  %486 = phi i64 [ 0, %482 ], [ %494, %485 ]
  %487 = phi float [ 0.000000e+00, %482 ], [ %493, %485 ]
  %488 = getelementptr inbounds nuw [4 x i8], ptr %484, i64 %486
  %489 = load float, ptr %488, align 4, !tbaa !34
  %490 = getelementptr inbounds nuw [8 x i8], ptr %474, i64 %486
  %491 = load i64, ptr %490, align 8, !tbaa !30
  %492 = sitofp i64 %491 to float
  %493 = tail call float @llvm.fmuladd.f32(float %489, float %492, float %487)
  %494 = add nuw nsw i64 %486, 1
  %495 = icmp eq i64 %494, 8
  br i1 %495, label %496, label %485, !llvm.loop !36

496:                                              ; preds = %485
  %497 = getelementptr inbounds nuw [4 x i8], ptr %475, i64 %483
  store float %493, ptr %497, align 4, !tbaa !34
  %498 = add nuw nsw i64 %483, 1
  %499 = icmp eq i64 %498, 8
  br i1 %499, label %500, label %482, !llvm.loop !37

500:                                              ; preds = %496
  %501 = add nuw nsw i64 %472, 1
  %502 = icmp eq i64 %501, 8
  br i1 %502, label %476, label %471, !llvm.loop !38

503:                                              ; preds = %532, %476
  %504 = phi i64 [ 0, %476 ], [ %533, %532 ]
  %505 = getelementptr inbounds nuw [32 x i8], ptr @cosine, i64 %504
  %506 = getelementptr [256 x i8], ptr %481, i64 %504
  br label %507

507:                                              ; preds = %521, %503
  %508 = phi i64 [ 0, %503 ], [ %530, %521 ]
  %509 = getelementptr inbounds nuw [4 x i8], ptr %1, i64 %508
  br label %510

510:                                              ; preds = %510, %507
  %511 = phi i64 [ 0, %507 ], [ %519, %510 ]
  %512 = phi float [ 0.000000e+00, %507 ], [ %518, %510 ]
  %513 = getelementptr inbounds nuw [4 x i8], ptr %505, i64 %511
  %514 = load float, ptr %513, align 4, !tbaa !34
  %515 = shl nuw nsw i64 %511, 5
  %516 = getelementptr inbounds nuw i8, ptr %509, i64 %515
  %517 = load float, ptr %516, align 4, !tbaa !34
  %518 = tail call float @llvm.fmuladd.f32(float %514, float %517, float %512)
  %519 = add nuw nsw i64 %511, 1
  %520 = icmp eq i64 %519, 8
  br i1 %520, label %521, label %510, !llvm.loop !39

521:                                              ; preds = %510
  %522 = fadd float %518, 1.285000e+02
  %523 = fcmp ugt float %522, 0.000000e+00
  %524 = fcmp oge float %522, 2.550000e+02
  %525 = select i1 %524, float 2.550000e+02, float %522
  %526 = fptosi float %525 to i32
  %527 = trunc i32 %526 to i8
  %528 = select i1 %523, i8 %527, i8 0
  %529 = getelementptr i8, ptr %506, i64 %508
  store i8 %528, ptr %529, align 1, !tbaa !21
  %530 = add nuw nsw i64 %508, 1
  %531 = icmp eq i64 %530, 8
  br i1 %531, label %532, label %507, !llvm.loop !40

532:                                              ; preds = %521
  %533 = add nuw nsw i64 %504, 1
  %534 = icmp eq i64 %533, 8
  br i1 %534, label %535, label %503, !llvm.loop !41

535:                                              ; preds = %532
  call void @llvm.lifetime.end.p0(ptr nonnull %1) #6
  %536 = add nuw nsw i64 %387, 1
  %537 = icmp eq i64 %536, %381
  br i1 %537, label %538, label %386, !llvm.loop !42

538:                                              ; preds = %535
  %539 = add nuw nsw i64 %383, 1
  %540 = icmp eq i64 %539, %380
  br i1 %540, label %541, label %382, !llvm.loop !43

541:                                              ; preds = %538, %370, %364
  %542 = add nuw nsw i64 %365, 1
  %543 = icmp eq i64 %542, 3
  br i1 %543, label %544, label %364, !llvm.loop !44

544:                                              ; preds = %541
  %545 = add nsw i32 %363, -1
  %546 = add nuw nsw i32 %344, 1
  %547 = icmp eq i32 %546, %316
  br i1 %547, label %548, label %341, !llvm.loop !45

548:                                              ; preds = %341, %544, %340
  %549 = phi i32 [ %338, %340 ], [ %545, %544 ], [ %545, %341 ]
  %550 = mul nuw nsw i64 %337, %334
  br label %551

551:                                              ; preds = %548, %616
  %552 = phi i64 [ 0, %548 ], [ %617, %616 ]
  %553 = add nuw nsw i64 %552, %550
  %554 = trunc nuw i64 %553 to i32
  %555 = icmp sgt i32 %238, %554
  br i1 %555, label %556, label %619

556:                                              ; preds = %551
  %557 = getelementptr inbounds nuw [256 x i8], ptr @planes, i64 %552
  %558 = trunc nuw nsw i64 %552 to i32
  %559 = lshr i32 %558, %331
  %560 = zext nneg i32 %559 to i64
  %561 = getelementptr inbounds nuw [256 x i8], ptr getelementptr inbounds nuw (i8, ptr @planes, i64 4096), i64 %560
  %562 = getelementptr inbounds nuw [256 x i8], ptr getelementptr inbounds nuw (i8, ptr @planes, i64 8192), i64 %560
  br label %563

563:                                              ; preds = %563, %556
  %564 = phi i64 [ 0, %556 ], [ %608, %563 ]
  %565 = getelementptr inbounds nuw i8, ptr %557, i64 %564
  %566 = load i8, ptr %565, align 1, !tbaa !21
  %567 = uitofp i8 %566 to float
  %568 = trunc nuw nsw i64 %564 to i32
  %569 = lshr i32 %568, %330
  %570 = zext nneg i32 %569 to i64
  %571 = getelementptr inbounds nuw i8, ptr %561, i64 %570
  %572 = load i8, ptr %571, align 1, !tbaa !21
  %573 = uitofp i8 %572 to float
  %574 = fadd float %573, -1.280000e+02
  %575 = getelementptr inbounds nuw i8, ptr %562, i64 %570
  %576 = load i8, ptr %575, align 1, !tbaa !21
  %577 = uitofp i8 %576 to float
  %578 = fadd float %577, -1.280000e+02
  %579 = tail call float @llvm.fmuladd.f32(float %574, float 1.772000e+00, float %567)
  %580 = fadd float %579, 5.000000e-01
  %581 = fcmp ugt float %580, 0.000000e+00
  %582 = fcmp oge float %580, 2.550000e+02
  %583 = select i1 %582, float 2.550000e+02, float %580
  %584 = fptosi float %583 to i32
  %585 = trunc i32 %584 to i8
  %586 = select i1 %581, i8 %585, i8 0
  %587 = mul nuw nsw i64 %564, 3
  %588 = getelementptr inbounds nuw i8, ptr @line, i64 %587
  store i8 %586, ptr %588, align 1, !tbaa !21
  %589 = tail call float @llvm.fmuladd.f32(float %574, float -3.441360e-01, float %567)
  %590 = tail call float @llvm.fmuladd.f32(float %578, float -7.141360e-01, float %589)
  %591 = fadd float %590, 5.000000e-01
  %592 = fcmp ugt float %591, 0.000000e+00
  %593 = fcmp oge float %591, 2.550000e+02
  %594 = select i1 %593, float 2.550000e+02, float %591
  %595 = fptosi float %594 to i32
  %596 = trunc i32 %595 to i8
  %597 = select i1 %592, i8 %596, i8 0
  %598 = getelementptr inbounds nuw i8, ptr %588, i64 1
  store i8 %597, ptr %598, align 1, !tbaa !21
  %599 = tail call float @llvm.fmuladd.f32(float %578, float 1.402000e+00, float %567)
  %600 = fadd float %599, 5.000000e-01
  %601 = fcmp ugt float %600, 0.000000e+00
  %602 = fcmp oge float %600, 2.550000e+02
  %603 = select i1 %602, float 2.550000e+02, float %600
  %604 = fptosi float %603 to i32
  %605 = trunc i32 %604 to i8
  %606 = select i1 %601, i8 %605, i8 0
  %607 = getelementptr inbounds nuw i8, ptr %588, i64 2
  store i8 %606, ptr %607, align 1, !tbaa !21
  %608 = add nuw nsw i64 %564, 1
  %609 = icmp eq i64 %608, %284
  br i1 %609, label %610, label %563, !llvm.loop !46

610:                                              ; preds = %563
  br i1 %332, label %611, label %616

611:                                              ; preds = %610, %611
  %612 = phi i64 [ %614, %611 ], [ %333, %610 ]
  %613 = getelementptr inbounds nuw i8, ptr @line, i64 %612
  store i8 0, ptr %613, align 1, !tbaa !21
  %614 = add nuw nsw i64 %612, 1
  %615 = icmp eq i64 %614, %271
  br i1 %615, label %616, label %611, !llvm.loop !47

616:                                              ; preds = %611, %610
  tail call fastcc void @emit(i32 noundef %317, ptr noundef nonnull @line, i32 noundef %270) #8
  %617 = add nuw nsw i64 %552, 1
  %618 = icmp eq i64 %617, %334
  br i1 %618, label %619, label %551, !llvm.loop !48

619:                                              ; preds = %551, %616
  %620 = add nuw nsw i64 %337, 1
  %621 = icmp samesign ult i64 %620, %335
  br i1 %621, label %336, label %622, !llvm.loop !49

622:                                              ; preds = %336, %619
  %623 = tail call i64 @llrm_os_close(i32 noundef %317) #7
  %624 = load i32, ptr @infile, align 4, !tbaa !13
  %625 = tail call i64 @llrm_os_close(i32 noundef %624) #7
  %626 = load i64, ptr @adler_b, align 8, !tbaa !30
  %627 = shl i64 %626, 16
  %628 = load i64, ptr @adler_a, align 8, !tbaa !30
  %629 = or i64 %627, %628
  br label %630

630:                                              ; preds = %622, %319, %265, %182, %176, %57, %140, %67, %21, %11
  %631 = phi i64 [ 0, %11 ], [ 0, %21 ], [ 0, %265 ], [ 0, %319 ], [ %629, %622 ], [ 0, %67 ], [ 0, %140 ], [ 0, %57 ], [ 0, %176 ], [ 0, %182 ]
  call void @llvm.lifetime.end.p0(ptr nonnull %7) #6
  call void @llvm.lifetime.end.p0(ptr nonnull %6) #6
  call void @llvm.lifetime.end.p0(ptr nonnull %5) #6
  call void @llvm.lifetime.end.p0(ptr nonnull %4) #6
  call void @llvm.lifetime.end.p0(ptr nonnull %3) #6
  call void @llvm.lifetime.end.p0(ptr nonnull %2) #6
  ret i64 %631
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.start.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local i32 @llrm_os_open(ptr noundef, i8 noundef) local_unnamed_addr #2

; Function Attrs: nounwind optsize
define internal fastcc range(i32 0, 256) i32 @next() unnamed_addr #3 {
  %1 = load i32, ptr @inpos, align 4, !tbaa !13
  %2 = load i32, ptr @inlen, align 4, !tbaa !13
  %3 = icmp eq i32 %1, %2
  br i1 %3, label %4, label %9

4:                                                ; preds = %0
  %5 = load i32, ptr @infile, align 4, !tbaa !13
  %6 = tail call i32 @llrm_os_read(i32 noundef %5, ptr noundef nonnull @inbuf, i64 noundef 512) #7
  store i32 %6, ptr @inlen, align 4, !tbaa !13
  store i32 0, ptr @inpos, align 4, !tbaa !13
  %7 = icmp slt i32 %6, 1
  br i1 %7, label %8, label %9

8:                                                ; preds = %4
  store i32 0, ptr @inlen, align 4, !tbaa !13
  store i1 true, ptr @bad, align 4
  br label %16

9:                                                ; preds = %4, %0
  %10 = phi i32 [ 0, %4 ], [ %1, %0 ]
  %11 = add nsw i32 %10, 1
  store i32 %11, ptr @inpos, align 4, !tbaa !13
  %12 = sext i32 %10 to i64
  %13 = getelementptr inbounds i8, ptr @inbuf, i64 %12
  %14 = load i8, ptr %13, align 1, !tbaa !21
  %15 = zext i8 %14 to i32
  br label %16

16:                                               ; preds = %9, %8
  %17 = phi i32 [ 0, %8 ], [ %15, %9 ]
  ret i32 %17
}

; Function Attrs: mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite)
declare void @llvm.lifetime.end.p0(ptr captures(none)) #1

; Function Attrs: optsize
declare dso_local i32 @llrm_os_create(ptr noundef) local_unnamed_addr #2

; Function Attrs: nounwind optsize
define internal fastcc void @emit(i32 noundef range(i32 0, -2147483648) %0, ptr noundef %1, i32 noundef range(i32 0, 196609) %2) unnamed_addr #3 {
  %4 = zext nneg i32 %2 to i64
  %5 = tail call i32 @llrm_os_write_file(i32 noundef %0, ptr noundef %1, i64 noundef %4) #7
  %6 = icmp eq i32 %2, 0
  br i1 %6, label %28, label %7

7:                                                ; preds = %3
  %8 = load i64, ptr @adler_b, align 8
  %9 = load i64, ptr @adler_a, align 8
  br label %10

10:                                               ; preds = %7, %10
  %11 = phi i64 [ 0, %7 ], [ %25, %10 ]
  %12 = phi i64 [ %9, %7 ], [ %20, %10 ]
  %13 = phi i64 [ %8, %7 ], [ %24, %10 ]
  %14 = getelementptr inbounds nuw i8, ptr %1, i64 %11
  %15 = load i8, ptr %14, align 1, !tbaa !21
  %16 = zext i8 %15 to i64
  %17 = add i64 %12, %16
  %18 = icmp ugt i64 %17, 65520
  %19 = add i64 %17, -65521
  %20 = select i1 %18, i64 %19, i64 %17
  %21 = add i64 %20, %13
  %22 = icmp ugt i64 %21, 65520
  %23 = add i64 %21, -65521
  %24 = select i1 %22, i64 %23, i64 %21
  %25 = add nuw nsw i64 %11, 1
  %26 = icmp eq i64 %25, %4
  br i1 %26, label %27, label %10, !llvm.loop !50

27:                                               ; preds = %10
  store i64 %20, ptr @adler_a, align 8, !tbaa !30
  store i64 %24, ptr @adler_b, align 8, !tbaa !30
  br label %28

28:                                               ; preds = %27, %3
  ret void
}

; Function Attrs: nounwind optsize
define internal fastcc range(i32 0, 256) i32 @decode(i32 noundef %0) unnamed_addr #3 {
  %2 = tail call fastcc i32 @getbit() #8
  %3 = sext i32 %0 to i64
  %4 = getelementptr inbounds [72 x i8], ptr @maxcode, i64 %3
  br label %5

5:                                                ; preds = %11, %1
  %6 = phi i64 [ %15, %11 ], [ 1, %1 ]
  %7 = phi i32 [ %14, %11 ], [ %2, %1 ]
  %8 = getelementptr inbounds nuw [4 x i8], ptr %4, i64 %6
  %9 = load i32, ptr %8, align 4, !tbaa !13
  %10 = icmp sgt i32 %7, %9
  br i1 %10, label %11, label %18

11:                                               ; preds = %5
  %12 = shl nsw i32 %7, 1
  %13 = tail call fastcc i32 @getbit() #8
  %14 = or disjoint i32 %13, %12
  %15 = add nuw nsw i64 %6, 1
  %16 = icmp eq i64 %15, 17
  br i1 %16, label %17, label %5, !llvm.loop !51

17:                                               ; preds = %11
  store i1 true, ptr @bad, align 4
  br label %32

18:                                               ; preds = %5
  %19 = getelementptr inbounds [256 x i8], ptr @huffval, i64 %3
  %20 = getelementptr inbounds [68 x i8], ptr @valptr, i64 %3
  %21 = getelementptr inbounds nuw [4 x i8], ptr %20, i64 %6
  %22 = load i32, ptr %21, align 4, !tbaa !13
  %23 = add nsw i32 %22, %7
  %24 = getelementptr inbounds [68 x i8], ptr @mincode, i64 %3
  %25 = getelementptr inbounds nuw [4 x i8], ptr %24, i64 %6
  %26 = load i32, ptr %25, align 4, !tbaa !13
  %27 = sub i32 %23, %26
  %28 = sext i32 %27 to i64
  %29 = getelementptr inbounds i8, ptr %19, i64 %28
  %30 = load i8, ptr %29, align 1, !tbaa !21
  %31 = zext i8 %30 to i32
  br label %32

32:                                               ; preds = %18, %17
  %33 = phi i32 [ 0, %17 ], [ %31, %18 ]
  ret i32 %33
}

; Function Attrs: optsize
declare dso_local i64 @llrm_os_close(i32 noundef) local_unnamed_addr #2

; Function Attrs: nounwind optsize
define dso_local noundef i32 @main() local_unnamed_addr #3 {
  %1 = tail call i64 @bench_jpeg_single() #8
  tail call void @report(i64 noundef %1) #7
  ret i32 0
}

; Function Attrs: optsize
declare dso_local void @report(i64 noundef) local_unnamed_addr #2

; Function Attrs: optsize
declare dso_local i32 @llrm_os_write_file(i32 noundef, ptr noundef, i64 noundef) local_unnamed_addr #2

; Function Attrs: optsize
declare dso_local i32 @llrm_os_read(i32 noundef, ptr noundef, i64 noundef) local_unnamed_addr #2

; Function Attrs: nounwind optsize
define internal fastcc range(i32 0, 2) i32 @getbit() unnamed_addr #3 {
  %1 = load i32, ptr @bitcnt, align 4, !tbaa !13
  %2 = icmp eq i32 %1, 0
  br i1 %2, label %3, label %10

3:                                                ; preds = %0
  %4 = tail call fastcc i32 @next() #8
  store i32 %4, ptr @bitbuf, align 4, !tbaa !13
  %5 = icmp eq i32 %4, 255
  br i1 %5, label %6, label %10

6:                                                ; preds = %3
  %7 = tail call fastcc i32 @next() #8
  %8 = icmp eq i32 %7, 0
  br i1 %8, label %10, label %9

9:                                                ; preds = %6
  store i1 true, ptr @bad, align 4
  br label %10

10:                                               ; preds = %3, %6, %9, %0
  %11 = phi i32 [ %1, %0 ], [ 8, %9 ], [ 8, %6 ], [ 8, %3 ]
  %12 = add nsw i32 %11, -1
  store i32 %12, ptr @bitcnt, align 4, !tbaa !13
  %13 = load i32, ptr @bitbuf, align 4, !tbaa !13
  %14 = lshr i32 %13, %12
  %15 = and i32 %14, 1
  ret i32 %15
}

; Function Attrs: mustprogress nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare float @llvm.fmuladd.f32(float, float, float) #4

; Function Attrs: nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none)
declare i32 @llvm.umin.i32(i32, i32) #5

attributes #0 = { noinline nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #1 = { mustprogress nocallback nofree nosync nounwind willreturn memory(argmem: readwrite) }
attributes #2 = { optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #3 = { nounwind optsize "frame-pointer"="non-leaf-no-reserve" "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" "target-cpu"="generic" "target-features"="+fp-armv8,+neon,+outline-atomics,+v8a,-fmv" }
attributes #4 = { mustprogress nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #5 = { nocallback nocreateundeforpoison nofree nosync nounwind speculatable willreturn memory(none) }
attributes #6 = { nounwind }
attributes #7 = { nobuiltin nounwind optsize "no-builtins" }
attributes #8 = { nobuiltin optsize "no-builtins" }

!llvm.module.flags = !{!0, !1, !2, !3, !4, !5, !6}
!llvm.ident = !{!7}
!llvm.errno.tbaa = !{!8}

!0 = !{i32 1, !"ptrauth-elf-got", i32 0}
!1 = !{i32 1, !"ptrauth-init-fini", i32 0}
!2 = !{i32 1, !"ptrauth-init-fini-address-discrimination", i32 0}
!3 = !{i32 1, !"ptrauth-sign-personality", i32 0}
!4 = !{i32 1, !"aarch64-elf-pauthabi-platform", i32 268435458}
!5 = !{i32 1, !"aarch64-elf-pauthabi-version", i32 0}
!6 = !{i32 7, !"frame-pointer", i32 4}
!7 = !{!"clang version 24.0.0git (https://github.com/llvm/llvm-project.git d1106deb71cc81016406a1917d0508d2df296266)"}
!8 = !{!9, !10, i64 0}
!9 = !{!"__libc_errno", !10, i64 0}
!10 = !{!"int", !11, i64 0}
!11 = !{!"omnipotent char", !12, i64 0}
!12 = !{!"Simple C/C++ TBAA"}
!13 = !{!10, !10, i64 0}
!14 = distinct !{!14, !15}
!15 = !{!"llvm.loop.mustprogress"}
!16 = distinct !{!16, !15}
!17 = distinct !{!17, !15}
!18 = distinct !{!18, !15}
!19 = distinct !{!19, !15}
!20 = distinct !{!20, !15}
!21 = !{!11, !11, i64 0}
!22 = distinct !{!22, !15}
!23 = distinct !{!23, !15}
!24 = distinct !{!24, !15}
!25 = distinct !{!25, !15}
!26 = distinct !{!26, !15}
!27 = distinct !{!27, !15}
!28 = distinct !{!28, !15}
!29 = distinct !{!29, !15}
!30 = !{!31, !31, i64 0}
!31 = !{!"long", !11, i64 0}
!32 = distinct !{!32, !15}
!33 = distinct !{!33, !15}
!34 = !{!35, !35, i64 0}
!35 = !{!"float", !11, i64 0}
!36 = distinct !{!36, !15}
!37 = distinct !{!37, !15}
!38 = distinct !{!38, !15}
!39 = distinct !{!39, !15}
!40 = distinct !{!40, !15}
!41 = distinct !{!41, !15}
!42 = distinct !{!42, !15}
!43 = distinct !{!43, !15}
!44 = distinct !{!44, !15}
!45 = distinct !{!45, !15}
!46 = distinct !{!46, !15}
!47 = distinct !{!47, !15}
!48 = distinct !{!48, !15}
!49 = distinct !{!49, !15}
!50 = distinct !{!50, !15}
!51 = distinct !{!51, !15}
