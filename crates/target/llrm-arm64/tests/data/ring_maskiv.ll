target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

; bench/ring/ring.c at -O2 (raw clang IR, tbaa dropped): the index is i16 arithmetic.
define i64 @ring() {
  %1 = alloca [1024 x i64], align 8
  br label %2

2:
  %3 = phi i64 [ 0, %0 ], [ %7, %2 ]
  %4 = mul nuw nsw i64 %3, 3
  %5 = add nsw i64 %4, -4
  %6 = getelementptr inbounds nuw [8 x i8], ptr %1, i64 %3
  store i64 %5, ptr %6, align 8
  %7 = add nuw nsw i64 %3, 1
  %8 = icmp eq i64 %7, 1024
  br i1 %8, label %9, label %2

9:
  %10 = phi i16 [ %19, %9 ], [ 0, %2 ]
  %11 = phi i64 [ %18, %9 ], [ 0, %2 ]
  %12 = mul nuw nsw i16 %10, 5
  %13 = add nuw nsw i16 %12, 3
  %14 = and i16 %13, 1023
  %15 = zext nneg i16 %14 to i64
  %16 = getelementptr inbounds nuw [8 x i8], ptr %1, i64 %15
  %17 = load i64, ptr %16, align 8
  %18 = add nsw i64 %17, %11
  %19 = add nuw nsw i16 %10, 1
  %20 = icmp samesign ult i16 %10, 5999
  br i1 %20, label %9, label %21

21:
  ret i64 %18
}
