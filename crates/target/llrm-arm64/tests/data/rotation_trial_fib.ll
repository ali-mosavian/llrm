target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define i32 @fib(i32 noundef %0) nounwind optsize {
  br label %tailrecurse

tailrecurse:
  %accumulator.tr = phi i32 [ 0, %1 ], [ %7, %4 ]
  %2 = phi i32 [ %0, %1 ], [ %8, %4 ]
  %3 = icmp slt i32 %2, 2
  br i1 %3, label %9, label %4

4:
  %5 = add i32 %2, -1
  %6 = tail call i32 @fib(i32 noundef %5) nobuiltin optsize
  %7 = add i32 %6, %accumulator.tr
  %8 = add i32 %5, -1
  br label %tailrecurse

9:
  %accumulator.ret.tr = add i32 %2, %accumulator.tr
  ret i32 %accumulator.ret.tr
}
