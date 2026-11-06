target datalayout = "e-m:o-i64:64-i128:128-n32:64-S128"

define i32 @k(ptr %0, ptr %1, i64 %2) {
  %4 = icmp sgt i64 %2, 0
  br i1 %4, label %5, label %14
5:
  %6 = phi ptr [ %12, %5 ], [ %0, %3 ]
  %7 = phi i64 [ %9, %5 ], [ %2, %3 ]
  %8 = phi ptr [ %10, %5 ], [ %1, %3 ]
  %9 = add i64 %7, -1
  %10 = getelementptr i8, ptr %8, i64 1
  %11 = load i8, ptr %8, align 1
  %12 = getelementptr i8, ptr %6, i64 1
  store i8 %11, ptr %6, align 1
  %13 = icmp ugt i64 %7, 1
  br i1 %13, label %5, label %14
14:
  ret i32 0
}
