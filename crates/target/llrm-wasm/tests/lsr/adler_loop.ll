target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
@.str = private unnamed_addr constant [10 x i8] c"PHOTO.JPG\00", align 1
@infile = internal unnamed_addr global i32 0, align 4
@.str.1 = private unnamed_addr constant [22 x i8] c"PHOTO.JPG not found\0D\0A\00", align 1
@.str.2 = private unnamed_addr constant [13 x i8] c"not a JPEG\0D\0A\00", align 1
@bad = internal unnamed_addr global i1 false, align 4
@.str.3 = private unnamed_addr constant [43 x i8] c"16-bit quantization tables not supported\0D\0A\00", align 1
@quant = internal unnamed_addr global [4 x [64 x i32]] zeroinitializer, align 16
@valptr = internal unnamed_addr global [4 x [17 x i32]] zeroinitializer, align 16
@mincode = internal unnamed_addr global [4 x [17 x i32]] zeroinitializer, align 16
@maxcode = internal unnamed_addr global [4 x [18 x i32]] zeroinitializer, align 16
@huffval = internal unnamed_addr global [4 x [256 x i8]] zeroinitializer, align 16
@.str.4 = private unnamed_addr constant [35 x i8] c"only 8-bit samples are supported\0D\0A\00", align 1
@.str.5 = private unnamed_addr constant [33 x i8] c"progressive JPEG not supported\0D\0A\00", align 1
@.str.6 = private unnamed_addr constant [38 x i8] c"arithmetic-coded JPEG not supported\0D\0A\00", align 1
@.str.7 = private unnamed_addr constant [34 x i8] c"only baseline JPEG is supported\0D\0A\00", align 1
@.str.8 = private unnamed_addr constant [19 x i8] c"unsupported JPEG\0D\0A\00", align 1
@bmp = internal global [54 x i8] zeroinitializer, align 16
@.str.9 = private unnamed_addr constant [8 x i8] c"OUT.BMP\00", align 1
@.str.10 = private unnamed_addr constant [23 x i8] c"cannot write OUT.BMP\0D\0A\00", align 1
@coef = internal unnamed_addr global [64 x i32] zeroinitializer, align 16
@zigzag = internal unnamed_addr constant [64 x i8] c"\00\01\08\10\09\02\03\0A\11\18 \19\12\0B\04\05\0C\13\1A!(0)\22\1B\14\0D\06\07\0E\15\1C#*1892+$\1D\16\0F\17\1E%,3:;4-&\1F'.5<=6/7>?", align 16
@line = internal global [771 x i8] zeroinitializer, align 16
@adler_b = internal unnamed_addr global i32 0, align 4
@adler_a = internal unnamed_addr global i32 1, align 4
@inpos = internal unnamed_addr global i32 0, align 4
@inlen = internal unnamed_addr global i32 0, align 4
@inbuf = internal global [512 x i8] zeroinitializer, align 16
@bitcnt = internal unnamed_addr global i32 0, align 4
@bitbuf = internal unnamed_addr global i32 0, align 4
@cosine = internal unnamed_addr constant [8 x [8 x i32]] [[8 x i32] [i32 362, i32 502, i32 473, i32 426, i32 362, i32 284, i32 196, i32 100], [8 x i32] [i32 362, i32 426, i32 196, i32 -100, i32 -362, i32 -502, i32 -473, i32 -284], [8 x i32] [i32 362, i32 284, i32 -196, i32 -502, i32 -362, i32 100, i32 473, i32 426], [8 x i32] [i32 362, i32 100, i32 -473, i32 -284, i32 362, i32 426, i32 -196, i32 -502], [8 x i32] [i32 362, i32 -100, i32 -473, i32 284, i32 362, i32 -426, i32 -196, i32 502], [8 x i32] [i32 362, i32 -284, i32 -196, i32 502, i32 -362, i32 -100, i32 473, i32 -426], [8 x i32] [i32 362, i32 -426, i32 196, i32 100, i32 -362, i32 502, i32 -473, i32 284], [8 x i32] [i32 362, i32 -502, i32 473, i32 -426, i32 362, i32 -284, i32 196, i32 -100]], align 16
@planes = internal unnamed_addr global [3 x [16 x [256 x i8]]] zeroinitializer, align 16
define internal fastcc void @emit(i32 noundef range(i32 0, -2147483648) %0, ptr noundef %1, i32 noundef range(i32 0, 196609) %2) nounwind      {
  %4 = tail call i32 @llrm_os_write_file(i32 noundef %0, ptr noundef %1, i32 noundef %2) 
  %promoted = load i32, ptr @adler_a, align 4
  %promoted1 = load i32, ptr @adler_b, align 4
  %5 = icmp eq i32 %2, 0
  br i1 %5, label %25, label %6

6:
  br label %7

7:
  %8 = phi i32 [ %23, %7 ], [ 0, %6 ]
  %9 = phi i32 [ %17, %7 ], [ %promoted, %6 ]
  %10 = phi i32 [ %22, %7 ], [ %promoted1, %6 ]
  %11 = getelementptr inbounds nuw i8, ptr %1, i32 %8
  %12 = load i8, ptr %11, align 1
  %13 = zext i8 %12 to i32
  %14 = add i32 %9, %13
  %15 = icmp ugt i32 %14, 65520
  %16 = add i32 %14, -65521
  %17 = select i1 %15, i32 %16, i32 %14
  %18 = select i1 %15, i32 %16, i32 %14
  %19 = add i32 %10, %18
  %20 = icmp ugt i32 %19, 65520
  %21 = add i32 %19, -65521
  %22 = select i1 %20, i32 %21, i32 %19
  %23 = add nuw nsw i32 %8, 1
  %exitcond = icmp eq i32 %23, %2
  br i1 %exitcond, label %24, label %7

24:
  br label %25

25:
  %26 = phi i32 [ %promoted1, %3 ], [ %22, %24 ]
  %27 = phi i32 [ %promoted, %3 ], [ %17, %24 ]
  store i32 %27, ptr @adler_a, align 4
  store i32 %26, ptr @adler_b, align 4
  ret void
}
declare void @llrm_os_write_file(...)
