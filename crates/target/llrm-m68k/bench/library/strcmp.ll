target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(ptr %0, ptr %1) {
  %3 = load i8, ptr %0, align 1
  %4 = icmp eq i8 %3, 0
  br i1 %4, label %20, label %5

5:                                                ; preds = %2, %11
  %6 = phi i8 [ %14, %11 ], [ %3, %2 ]
  %7 = phi ptr [ %13, %11 ], [ %1, %2 ]
  %8 = phi ptr [ %12, %11 ], [ %0, %2 ]
  %9 = load i8, ptr %7, align 1
  %10 = icmp eq i8 %6, %9
  br i1 %10, label %11, label %16

11:                                               ; preds = %5
  %12 = getelementptr i8, ptr %8, i32 1
  %13 = getelementptr i8, ptr %7, i32 1
  %14 = load i8, ptr %12, align 1
  %15 = icmp eq i8 %14, 0
  br i1 %15, label %16, label %5

16:                                               ; preds = %11, %5
  %17 = phi ptr [ %7, %5 ], [ %13, %11 ]
  %18 = phi i8 [ %6, %5 ], [ 0, %11 ]
  %19 = zext i8 %18 to i32
  br label %20

20:                                               ; preds = %16, %2
  %21 = phi ptr [ %1, %2 ], [ %17, %16 ]
  %22 = phi i32 [ 0, %2 ], [ %19, %16 ]
  %23 = load i8, ptr %21, align 1
  %24 = zext i8 %23 to i32
  %25 = sub i32 %22, %24
  ret i32 %25
}
