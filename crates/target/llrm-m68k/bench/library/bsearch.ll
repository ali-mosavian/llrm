target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(ptr %0, i32 %1, i32 %2) {
  %4 = icmp slt i32 %1, 1
  br i1 %4, label %22, label %5

5:                                                ; preds = %3
  %6 = add i32 %1, -1
  br label %7

7:                                                ; preds = %5, %15
  %8 = phi i32 [ %20, %15 ], [ %6, %5 ]
  %9 = phi i32 [ %19, %15 ], [ 0, %5 ]
  %10 = add i32 %8, %9
  %11 = lshr i32 %10, 1
  %12 = getelementptr i32, ptr %0, i32 %11
  %13 = load i32, ptr %12, align 4
  %14 = icmp eq i32 %13, %2
  br i1 %14, label %22, label %15

15:                                               ; preds = %7
  %16 = icmp slt i32 %13, %2
  %17 = add i32 %11, 1
  %18 = add i32 %11, -1
  %19 = select i1 %16, i32 %17, i32 %9
  %20 = select i1 %16, i32 %8, i32 %18
  %21 = icmp sgt i32 %19, %20
  br i1 %21, label %22, label %7

22:                                               ; preds = %15, %7, %3
  %23 = phi i32 [ -1, %3 ], [ %11, %7 ], [ -1, %15 ]
  ret i32 %23
}
