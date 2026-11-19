target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

define i32 @k(ptr %0, i32 %1, i32 %2) {
  %4 = icmp sgt i32 %2, 0
  br i1 %4, label %5, label %7

5:                                                ; preds = %3
  %6 = trunc i32 %1 to i8
  br label %15

7:                                                ; preds = %15, %3
  %8 = getelementptr i8, ptr %0, i32 %2
  %9 = getelementptr i8, ptr %8, i32 -1
  %10 = load i8, ptr %9, align 1
  %11 = sext i8 %10 to i32
  %12 = load i8, ptr %0, align 1
  %13 = sext i8 %12 to i32
  %14 = add i32 %13, %11
  ret i32 %14

15:                                               ; preds = %5, %15
  %16 = phi i32 [ 0, %5 ], [ %18, %15 ]
  %17 = getelementptr i8, ptr %0, i32 %16
  store i8 %6, ptr %17, align 1
  %18 = add i32 %16, 1
  %19 = icmp eq i32 %18, %2
  br i1 %19, label %7, label %15
}
