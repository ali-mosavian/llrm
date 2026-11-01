target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(i32 %0, ptr initializes((8, 9)) %1) {
  %3 = getelementptr i8, ptr %1, i32 8
  store i8 0, ptr %3, align 1
  br label %4

4:                                                ; preds = %4, %2
  %5 = phi ptr [ %3, %2 ], [ %13, %4 ]
  %6 = phi i32 [ %0, %2 ], [ %14, %4 ]
  %7 = and i32 %6, 15
  %8 = icmp ult i32 %7, 10
  %9 = or disjoint i32 %7, 48
  %10 = add i32 %7, 87
  %11 = select i1 %8, i32 %9, i32 %10
  %12 = trunc i32 %11 to i8
  %13 = getelementptr i8, ptr %5, i32 -1
  store i8 %12, ptr %13, align 1
  %14 = lshr i32 %6, 4
  %15 = icmp ult i32 %6, 16
  br i1 %15, label %16, label %4

16:                                               ; preds = %4
  %17 = getelementptr i8, ptr %5, i32 2
  %18 = load i8, ptr %17, align 1
  %19 = sext i8 %18 to i32
  %20 = ptrtoint ptr %13 to i32
  %21 = ptrtoint ptr %1 to i32
  %22 = sub i32 %20, %21
  %23 = add i32 %22, %11
  %24 = add i32 %23, %19
  ret i32 %24
}
