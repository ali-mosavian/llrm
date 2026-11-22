; ModuleID = '<bc file>'
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

declare ptr @symb_name()

define ptr @args(i1 %.not.i, i8 %0, i1 %1, ptr %2, i1 %3) {
  br i1 %.not.i, label %5, label %arg.exit

5:                                                ; preds = %4
  %6 = call ptr @symb_name()
  unreachable

arg.exit:                                         ; preds = %4
  %7 = load i8, ptr null, align 1
  %8 = icmp eq i8 %0, 0
  br i1 %1, label %9, label %.critedge

9:                                                ; preds = %arg.exit
  %10 = load volatile ptr, ptr null, align 8
  %11 = icmp eq ptr %2, null
  br i1 %3, label %12, label %recarg.exit

.critedge:                                        ; preds = %arg.exit
  br label %12

12:                                               ; preds = %.critedge, %9
  unreachable

recarg.exit:                                      ; preds = %9
  ret ptr %10
}
