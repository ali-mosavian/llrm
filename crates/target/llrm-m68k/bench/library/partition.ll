target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(ptr %0, i32 %1) {
  %3 = add i32 %1, -1
  %4 = getelementptr i32, ptr %0, i32 %3
  %5 = load i32, ptr %4, align 4
  %6 = icmp sgt i32 %1, 1
  br i1 %6, label %17, label %9

7:                                                ; preds = %27
  %8 = load i32, ptr %4, align 4
  br label %9

9:                                                ; preds = %7, %2
  %10 = phi i32 [ %5, %2 ], [ %8, %7 ]
  %11 = phi i32 [ 0, %2 ], [ %28, %7 ]
  %12 = getelementptr i32, ptr %0, i32 %11
  %13 = load i32, ptr %12, align 4
  store i32 %10, ptr %12, align 4
  store i32 %13, ptr %4, align 4
  %14 = load i32, ptr %0, align 4
  %15 = shl i32 %14, 4
  %16 = add i32 %15, %11
  ret i32 %16

17:                                               ; preds = %2, %27
  %18 = phi i32 [ %28, %27 ], [ 0, %2 ]
  %19 = phi i32 [ %29, %27 ], [ 0, %2 ]
  %20 = getelementptr i32, ptr %0, i32 %19
  %21 = load i32, ptr %20, align 4
  %22 = icmp slt i32 %21, %5
  br i1 %22, label %23, label %27

23:                                               ; preds = %17
  %24 = getelementptr i32, ptr %0, i32 %18
  %25 = load i32, ptr %24, align 4
  store i32 %21, ptr %24, align 4
  store i32 %25, ptr %20, align 4
  %26 = add i32 %18, 1
  br label %27

27:                                               ; preds = %17, %23
  %28 = phi i32 [ %26, %23 ], [ %18, %17 ]
  %29 = add i32 %19, 1
  %30 = icmp eq i32 %29, %3
  br i1 %30, label %7, label %17
}
