target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(ptr %0, i32 %1) {
  %3 = icmp eq i32 %1, 0
  br i1 %3, label %27, label %4

4:                                                ; preds = %2, %13
  %5 = phi i32 [ %8, %13 ], [ %1, %2 ]
  %6 = phi i32 [ %15, %13 ], [ 65535, %2 ]
  %7 = phi ptr [ %14, %13 ], [ %0, %2 ]
  %8 = add i32 %5, -1
  %9 = load i8, ptr %7, align 1
  %10 = zext i8 %9 to i32
  %11 = shl i32 %10, 8
  %12 = xor i32 %11, %6
  br label %17

13:                                               ; preds = %17
  %14 = getelementptr i8, ptr %7, i32 1
  %15 = and i32 %24, 65535
  %16 = icmp eq i32 %8, 0
  br i1 %16, label %27, label %4

17:                                               ; preds = %4, %17
  %18 = phi i32 [ 0, %4 ], [ %25, %17 ]
  %19 = phi i32 [ %12, %4 ], [ %24, %17 ]
  %20 = and i32 %19, 32768
  %21 = icmp eq i32 %20, 0
  %22 = shl i32 %19, 1
  %23 = xor i32 %22, 4129
  %24 = select i1 %21, i32 %22, i32 %23
  %25 = add i32 %18, 1
  %26 = icmp eq i32 %25, 8
  br i1 %26, label %13, label %17

27:                                               ; preds = %13, %2
  %28 = phi i32 [ 65535, %2 ], [ %15, %13 ]
  %29 = lshr i32 %28, 8
  %30 = and i32 %28, 255
  %31 = xor i32 %29, %30
  ret i32 %31
}
