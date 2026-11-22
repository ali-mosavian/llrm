target triple = "m68k-unknown-linux-gnu"
target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

@b8 = global [1200 x i8] zeroinitializer, align 1
@a8 = global [1200 x i8] zeroinitializer, align 1
@a16 = global [1200 x i16] zeroinitializer, align 2
@b16 = global [1200 x i16] zeroinitializer, align 2
@b32 = global [1200 x i32] zeroinitializer, align 2
@a32 = global [1200 x i32] zeroinitializer, align 2
@t8 = global [1200 x i8] zeroinitializer, align 1
@s8 = global [1200 x i8] zeroinitializer, align 1

define void @reset() {
  br label %2

1:
  ret void

2:
  %3 = phi i32 [ 0, %0 ], [ %16, %2 ]
  %4 = trunc i32 %3 to i8
  %5 = mul i8 %4, 7
  %6 = getelementptr inbounds nuw i8, ptr @b8, i32 %3
  store i8 %5, ptr %6, align 1
  %7 = getelementptr inbounds nuw i8, ptr @a8, i32 %3
  store i8 %5, ptr %7, align 1
  %8 = getelementptr inbounds nuw [2 x i8], ptr @a16, i32 %3
  store i16 -25536, ptr %8, align 2
  %9 = getelementptr inbounds nuw [2 x i8], ptr @b16, i32 %3
  store i16 -25536, ptr %9, align 2
  %10 = mul nuw nsw i32 %3, 31
  %11 = getelementptr inbounds nuw [4 x i8], ptr @b32, i32 %3
  store i32 %10, ptr %11, align 2
  %12 = getelementptr inbounds nuw [4 x i8], ptr @a32, i32 %3
  store i32 %10, ptr %12, align 2
  %13 = mul i8 %4, 5
  %14 = getelementptr inbounds nuw i8, ptr @t8, i32 %3
  store i8 %13, ptr %14, align 1
  %15 = getelementptr inbounds nuw i8, ptr @s8, i32 %3
  store i8 %13, ptr %15, align 1
  %16 = add nuw nsw i32 %3, 1
  %exitcond = icmp eq i32 %16, 1200
  br i1 %exitcond, label %1, label %2
}




