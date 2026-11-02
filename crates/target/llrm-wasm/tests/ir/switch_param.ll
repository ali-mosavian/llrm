declare i32 @get()
define i32 @dense(i32 noundef %0) {
  switch i32 %0, label %9 [
    i32 0, label %2
    i32 1, label %3
    i32 2, label %4
    i32 3, label %5
    i32 4, label %6
    i32 5, label %7
  ]

2:                                                ; preds = %1
  br label %10

3:                                                ; preds = %1
  br label %10

4:                                                ; preds = %1
  br label %10

5:                                                ; preds = %1
  br label %10

6:                                                ; preds = %1
  br label %10

7:                                                ; preds = %1
  %8 = call i32 @get()
  br label %10

9:                                                ; preds = %1
  br label %10

10:                                               ; preds = %9, %7, %6, %5, %4, %3, %2
  %.0 = phi i32 [ -1, %9 ], [ %8, %7 ], [ 13, %6 ], [ 11, %5 ], [ 9, %4 ], [ 7, %3 ], [ 5, %2 ]
  ret i32 %.0
}

define i32 @fall(i32 noundef %0) {
  switch i32 %0, label %14 [
    i32 0, label %2
    i32 1, label %4
    i32 2, label %6
    i32 7, label %8
    i32 9, label %10
    i32 10, label %12
  ]

2:                                                ; preds = %1
  %3 = add nsw i32 0, 1
  br label %4

4:                                                ; preds = %2, %1
  %.0 = phi i32 [ 0, %1 ], [ %3, %2 ]
  %5 = add nsw i32 %.0, 2
  br label %16

6:                                                ; preds = %1
  %7 = add nsw i32 0, 4
  br label %16

8:                                                ; preds = %1
  %9 = add nsw i32 0, 8
  br label %16

10:                                               ; preds = %1
  %11 = add nsw i32 0, 16
  br label %16

12:                                               ; preds = %1
  %13 = add nsw i32 0, 32
  br label %16

14:                                               ; preds = %1
  %15 = add nsw i32 0, 64
  br label %16

16:                                               ; preds = %14, %12, %10, %8, %6, %4
  %.1 = phi i32 [ %15, %14 ], [ %13, %12 ], [ %11, %10 ], [ %9, %8 ], [ %7, %6 ], [ %5, %4 ]
  ret i32 %.1
}
