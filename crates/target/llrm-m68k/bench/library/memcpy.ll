target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(ptr %0, ptr %1, i32 %2) {
  %4 = icmp sgt i32 %2, 0
  br i1 %4, label %5, label %14

5:                                                ; preds = %3, %5
  %6 = phi ptr [ %12, %5 ], [ %0, %3 ]
  %7 = phi i32 [ %9, %5 ], [ %2, %3 ]
  %8 = phi ptr [ %10, %5 ], [ %1, %3 ]
  %9 = add i32 %7, -1
  %10 = getelementptr i8, ptr %8, i32 1
  %11 = load i8, ptr %8, align 1
  %12 = getelementptr i8, ptr %6, i32 1
  store i8 %11, ptr %6, align 1
  %13 = icmp ugt i32 %7, 1
  br i1 %13, label %5, label %14

14:                                               ; preds = %5, %3
  %15 = getelementptr i8, ptr %0, i32 12
  %16 = load i8, ptr %15, align 1
  %17 = sext i8 %16 to i32
  ret i32 %17
}
