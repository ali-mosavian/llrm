target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

define i64 @bench_jpeg_single(ptr %inpos, i64 %0, ptr %planes) {
  br label %2

2:                                                ; preds = %14, %1
  br label %3

3:                                                ; preds = %14, %2
  %4 = phi i64 [ 0, %2 ], [ %16, %14 ]
  %5 = load i8, ptr %planes, align 1
  %6 = uitofp i8 %5 to float
  %7 = fadd float %6, 5.000000e-01
  %8 = fcmp ole float %6, 0.000000e+00
  br i1 %8, label %14, label %9

9:                                                ; preds = %3
  %10 = fcmp oge float %7, 0.000000e+00
  %11 = select i1 %10, float 0.000000e+00, float 1.000000e+00
  %12 = fptosi float %11 to i32
  %13 = trunc i32 %12 to i8
  br label %14

14:                                               ; preds = %9, %3
  %15 = phi i8 [ %13, %9 ], [ 0, %3 ]
  store i8 %15, ptr %inpos, align 1
  %16 = add i64 %4, 1
  %exitcond1 = icmp eq i64 %0, %4
  br i1 %exitcond1, label %2, label %3
}
