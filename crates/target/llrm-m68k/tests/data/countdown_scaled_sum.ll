target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
define i32 @down(i32 noundef %0) {
  %2 = icmp sgt i32 %0, -1
  br i1 %2, label %3, label %5

3:
  br label %7

4:
  br label %5

5:
  %6 = phi i32 [ 0, %1 ], [ %13, %4 ]
  ret i32 %6

7:
  %8 = phi i32 [ %14, %7 ], [ %0, %3 ]
  %9 = phi i32 [ %13, %7 ], [ 0, %3 ]
  %10 = mul i32 %8, 3
  %11 = lshr i32 %8, 2
  %12 = xor i32 %10, %11
  %13 = add i32 %12, %9
  %14 = add i32 %8, -1
  %15 = icmp eq i32 %8, 0
  br i1 %15, label %4, label %7
}
