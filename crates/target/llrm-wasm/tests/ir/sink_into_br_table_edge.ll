target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare i32 @next()
declare void @report(i32)
define i32 @scan() {
  br label %1

1:
  %2 = tail call i32 @next()
  switch i32 %2, label %5 [
    i32 196, label %16
    i32 192, label %3
    i32 194, label %18
  ]

3:
  %4 = tail call i32 @next()
  br label %16

5:
  %6 = add i32 %2, -201
  %7 = icmp ult i32 %6, 7
  %8 = icmp ne i32 %2, 204
  %9 = and i1 %8, %7
  br i1 %9, label %18, label %10

10:
  %11 = add i32 %2, -193
  %12 = icmp ult i32 %11, 15
  br i1 %12, label %switch.early.test, label %15

switch.early.test:
  %13 = trunc nuw i32 %2 to i8
  %14 = zext i8 %13 to i32
  switch i32 %14, label %18 [
    i32 200, label %1
    i32 204, label %1
  ]

15:
  switch i32 %2, label %1 [
    i32 221, label %16
    i32 217, label %19
  ]

16:
  %17 = tail call i32 @next()
  br label %1

18:
  %sink = phi i32 [ 112, %1 ], [ 97, %5 ], [ 98, %switch.early.test ]
  tail call void @report(i32 noundef %sink)
  br label %19

19:
  %20 = phi i32 [ 0, %18 ], [ 1, %15 ]
  ret i32 %20
}
