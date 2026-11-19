declare i32 @get()
declare void @put(i32)
define i32 @small(i32 noundef %0) {
  switch i32 %0, label %4 [
    i32 1, label %2
    i32 2, label %3
  ]

2:                                                ; preds = %1
  br label %5

3:                                                ; preds = %1
  br label %5

4:                                                ; preds = %1
  br label %5

5:                                                ; preds = %4, %3, %2
  %.0 = phi i32 [ 0, %4 ], [ 20, %3 ], [ 10, %2 ]
  ret i32 %.0
}

define i32 @comp(i32 noundef %0) {
  %2 = add nsw i32 %0, 1
  switch i32 %2, label %6 [
    i32 1, label %3
    i32 2, label %4
    i32 3, label %5
  ]

3:                                                ; preds = %1
  br label %7

4:                                                ; preds = %1
  br label %7

5:                                                ; preds = %1
  br label %7

6:                                                ; preds = %1
  br label %7

7:                                                ; preds = %6, %5, %4, %3
  %.0 = phi i32 [ 0, %6 ], [ 30, %5 ], [ 20, %4 ], [ 10, %3 ]
  ret i32 %.0
}

define i32 @sparse(i32 noundef %0) {
  %2 = call i32 @get()
  switch i32 %2, label %7 [
    i32 1, label %3
    i32 100, label %4
    i32 1000, label %5
    i32 10000, label %6
  ]

3:                                                ; preds = %1
  br label %8

4:                                                ; preds = %1
  br label %8

5:                                                ; preds = %1
  br label %8

6:                                                ; preds = %1
  br label %8

7:                                                ; preds = %1
  br label %8

8:                                                ; preds = %7, %6, %5, %4, %3
  %.0 = phi i32 [ 0, %7 ], [ 40, %6 ], [ 30, %5 ], [ 20, %4 ], [ 10, %3 ]
  ret i32 %.0
}

define i32 @merge(i32 noundef %0) {
  %2 = call i32 @get()
  switch i32 %2, label %7 [
    i32 0, label %3
    i32 1, label %4
    i32 2, label %5
    i32 3, label %6
  ]

3:                                                ; preds = %1
  br label %8

4:                                                ; preds = %1
  br label %8

5:                                                ; preds = %1
  br label %8

6:                                                ; preds = %1
  br label %8

7:                                                ; preds = %1
  br label %8

8:                                                ; preds = %7, %6, %5, %4, %3
  %.0 = phi i32 [ 9, %7 ], [ %0, %6 ], [ 3, %5 ], [ 2, %4 ], [ 1, %3 ]
  ret i32 %.0
}

define i32 @loop(i32 noundef %0) {
  br label %2

2:                                                ; preds = %15, %1
  %.01 = phi i32 [ 0, %1 ], [ %.1, %15 ]
  %.0 = phi i32 [ 0, %1 ], [ %16, %15 ]
  %3 = icmp slt i32 %.0, %0
  br i1 %3, label %4, label %17

4:                                                ; preds = %2
  %5 = and i32 %.0, 3
  switch i32 %5, label %12 [
    i32 0, label %6
    i32 1, label %8
    i32 2, label %10
  ]

6:                                                ; preds = %4
  %7 = add nsw i32 %.01, 1
  br label %14

8:                                                ; preds = %4
  %9 = add nsw i32 %.01, 2
  br label %14

10:                                               ; preds = %4
  %11 = add nsw i32 %.01, 3
  br label %14

12:                                               ; preds = %4
  %13 = add nsw i32 %.01, 4
  br label %14

14:                                               ; preds = %12, %10, %8, %6
  %.1 = phi i32 [ %13, %12 ], [ %11, %10 ], [ %9, %8 ], [ %7, %6 ]
  br label %15

15:                                               ; preds = %14
  %16 = add nsw i32 %.0, 1
  br label %2

17:                                               ; preds = %2
  ret i32 %.01
}

define void @calls(i32 noundef %0) {
  %2 = call i32 @get()
  switch i32 %2, label %7 [
    i32 0, label %3
    i32 1, label %4
    i32 2, label %5
    i32 3, label %6
  ]

3:                                                ; preds = %1
  call void @put(i32 noundef 0)
  br label %8

4:                                                ; preds = %1
  call void @put(i32 noundef 1)
  br label %8

5:                                                ; preds = %1
  call void @put(i32 noundef 2)
  br label %6

6:                                                ; preds = %5, %1
  call void @put(i32 noundef 3)
  br label %8

7:                                                ; preds = %1
  call void @put(i32 noundef 9)
  br label %8

8:                                                ; preds = %7, %6, %4, %3
  ret void
}

define i32 @two(i32 noundef %0) {
  %2 = call i32 @get()
  switch i32 %2, label %5 [
    i32 5, label %3
    i32 6, label %4
  ]

3:                                                ; preds = %1
  br label %6

4:                                                ; preds = %1
  br label %6

5:                                                ; preds = %1
  br label %6

6:                                                ; preds = %5, %4, %3
  %.0 = phi i32 [ 0, %5 ], [ 2, %4 ], [ 1, %3 ]
  ret i32 %.0
}

define i32 @one(i32 noundef %0) {
  %2 = call i32 @get()
  switch i32 %2, label %4 [
    i32 5, label %3
  ]

3:                                                ; preds = %1
  br label %5

4:                                                ; preds = %1
  br label %5

5:                                                ; preds = %4, %3
  %.0 = phi i32 [ 0, %4 ], [ 1, %3 ]
  ret i32 %.0
}

define i32 @nodef(i32 noundef %0) {
  %2 = call i32 @get()
  switch i32 %2, label %5 [
    i32 0, label %3
    i32 1, label %3
    i32 2, label %3
    i32 3, label %3
    i32 4, label %4
    i32 5, label %4
  ]

3:                                                ; preds = %1, %1, %1, %1
  br label %6

4:                                                ; preds = %1, %1
  br label %6

5:                                                ; preds = %1
  br label %6

6:                                                ; preds = %5, %4, %3
  %.0 = phi i32 [ 0, %5 ], [ 8, %4 ], [ 7, %3 ]
  ret i32 %.0
}
