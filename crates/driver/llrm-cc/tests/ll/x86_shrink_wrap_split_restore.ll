target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

define i1 @decl_sametype(ptr %0, i32 %1, i32 %2, i32 %3, i1 %4, ptr %5, i1 %6, i1 %7, i1 %8) {
  br i1 %4, label %common.ret, label %10

common.ret:                                       ; preds = %15, %13, %10, %9
  %common.ret.op = phi i1 [ false, %13 ], [ false, %9 ], [ %7, %15 ], [ true, %10 ]
  ret i1 %common.ret.op

10:                                               ; preds = %9
  %11 = call ptr @decl_type()
  br i1 %6, label %common.ret, label %12

12:                                               ; preds = %10
  br i1 %7, label %13, label %15

13:                                               ; preds = %12
  %14 = call i1 @decl_sametype(ptr %0, i32 0, i32 0, i32 0, i1 false, ptr null, i1 false, i1 false, i1 false)
  br i1 %8, label %common.ret, label %15

15:                                               ; preds = %13, %12
  br label %common.ret
}

declare ptr @decl_type()
