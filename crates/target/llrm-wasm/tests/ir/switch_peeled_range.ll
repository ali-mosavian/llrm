target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
define i32 @pick(i32 noundef %0, i32 noundef %1) nounwind optsize {
  switch i32 %0, label %7 [
    i32 192, label %8
    i32 194, label %3
    i32 196, label %4
    i32 219, label %5
    i32 195, label %6
    i32 207, label %6
    i32 208, label %6
    i32 209, label %6
    i32 210, label %6
    i32 211, label %6
    i32 212, label %6
    i32 213, label %6
    i32 214, label %6
    i32 215, label %6
    i32 216, label %6
    i32 217, label %6
    i32 218, label %6
  ]

3:
  br label %8
4:
  br label %8
5:
  br label %8
6:
  br label %8
7:
  br label %8
8:
  %9 = phi i32 [ 0, %7 ], [ 4, %3 ], [ 2, %4 ], [ 1, %5 ], [ %1, %6 ], [ 3, %2 ]
  ret i32 %9
}
