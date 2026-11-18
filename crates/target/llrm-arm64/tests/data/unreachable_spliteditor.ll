; ModuleID = '<bc file>'
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

declare ptr @prog_line()

define void @blok_scanline(ptr %0, i1 %1) {
  br i1 %1, label %3, label %5

3:                                                ; preds = %2
  %4 = call ptr @prog_line()
  unreachable

5:                                                ; preds = %5, %2
  %.0 = phi ptr [ %0, %2 ], [ inttoptr (i64 1 to ptr), %5 ]
  %6 = call ptr @blok_stmt(ptr %.0)
  %.pre = load i8, ptr inttoptr (i64 1 to ptr), align 1
  %7 = icmp eq i8 %.pre, 0
  br i1 %7, label %8, label %5

8:                                                ; preds = %5
  ret void
}

define internal ptr @blok_stmt(ptr %0) {
  %2 = load i8, ptr %0, align 1
  switch i8 %2, label %14 [
    i8 -127, label %10
    i8 -51, label %3
    i8 -52, label %4
    i8 -50, label %5
    i8 -48, label %6
    i8 -49, label %8
    i8 -47, label %9
    i8 -41, label %13
    i8 -42, label %12
    i8 -126, label %11
  ]

3:                                                ; preds = %1
  store i1 false, ptr null, align 1
  ret ptr null

4:                                                ; preds = %1
  ret ptr null

5:                                                ; preds = %1
  store i1 false, ptr null, align 1
  ret ptr null

6:                                                ; preds = %1
  %7 = call ptr @blok_add()
  ret ptr %7

8:                                                ; preds = %1
  ret ptr null

9:                                                ; preds = %1
  ret ptr null

10:                                               ; preds = %1
  store i1 false, ptr null, align 1
  ret ptr null

11:                                               ; preds = %1
  ret ptr null

12:                                               ; preds = %1
  store i16 0, ptr %0, align 8
  ret ptr null

13:                                               ; preds = %1
  ret ptr null

14:                                               ; preds = %1
  store i1 false, ptr null, align 1
  ret ptr null
}

declare ptr @blok_add()
