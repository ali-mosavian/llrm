; compiler-rt lib/fp/f32/float_si.c after the mid-end: x >> 31 of a long kept across a call (the sign byte and its mask share a).
target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
declare i16 @__clzsi2(i32)
define float @__floatsisf(i32 %0) nounwind "no-builtins" "no-trapping-math"="true" "stack-protector-buffer-size"="8" optsize {
  %2 = ashr i32 %0, 31
  %3 = and i32 %2, -2147483648
  %4 = xor i32 %2, %0
  %5 = sub i32 %4, %2
  %6 = icmp eq i32 %0, 0
  br i1 %6, label %38, label %7

7:
  %8 = tail call i16 @__clzsi2(i32 %5) 
  %9 = icmp slt i16 %8, 5
  br i1 %9, label %10, label %22

10:
  %11 = sub i16 5, %8
  %12 = icmp samesign ugt i16 %11, 31
  br i1 %12, label %26, label %13

13:
  %14 = zext nneg i16 %11 to i32
  %15 = lshr i32 %5, %14
  %16 = add nsw i16 %8, 27
  %17 = zext nneg i16 %16 to i32
  %18 = shl i32 %5, %17
  %19 = icmp ne i32 %18, 0
  %20 = zext i1 %19 to i32
  %21 = or i32 %15, %20
  br label %26

22:
  %23 = add nsw i16 %8, -5
  %24 = zext nneg i16 %23 to i32
  %25 = shl i32 %5, %24
  br label %26

26:
  %27 = phi i32 [ %25, %22 ], [ %21, %13 ], [ 1, %10 ]
  %28 = add i32 %27, 3
  %29 = lshr i32 %27, 3
  %30 = and i32 %29, 1
  %31 = add i32 %28, %30
  %32 = lshr i32 %31, 3
  %33 = sub i16 157, %8
  %34 = zext i16 %33 to i32
  %35 = shl i32 %34, 23
  %36 = add i32 %35, %3
  %37 = add i32 %36, %32
  br label %38

38:
  %39 = phi i32 [ %37, %26 ], [ 0, %1 ]
  %40 = bitcast i32 %39 to float
  ret float %40
}
