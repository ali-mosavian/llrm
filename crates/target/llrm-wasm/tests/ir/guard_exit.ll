target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define i32 @guard_exit(i32 %0) {
  %2 = icmp slt i32 %0, 1
  br i1 %2, label %28, label %3

3:
  br label %4

4:
  %5 = phi i32 [ %26, %23 ], [ 1, %3 ]
  %6 = phi i32 [ %25, %23 ], [ 0, %3 ]
  %7 = mul nuw nsw i32 %5, 7
  %8 = add nuw nsw i32 %7, 3
  %9 = mul nuw nsw i32 %5, 5
  %10 = add nuw nsw i32 %9, 2
  br label %11

11:
  %12 = phi i32 [ %14, %11 ], [ %10, %4 ]
  %13 = phi i32 [ %12, %11 ], [ %8, %4 ]
  %14 = srem i32 %13, %12
  %15 = icmp eq i32 %14, 0
  br i1 %15, label %16, label %11

16:
  %17 = add nuw nsw i32 %5, 90
  br label %18

18:
  %19 = phi i32 [ %21, %18 ], [ 360, %16 ]
  %20 = phi i32 [ %19, %18 ], [ %17, %16 ]
  %21 = srem i32 %20, %19
  %22 = icmp eq i32 %21, 0
  br i1 %22, label %23, label %18

23:
  %24 = add i32 %12, %6
  %25 = add i32 %24, %19
  %26 = add nuw i32 %5, 1
  %27 = icmp eq i32 %5, %0
  br i1 %27, label %28, label %4

28:
  %29 = phi i32 [ 0, %1 ], [ %25, %23 ]
  ret i32 %29
}
