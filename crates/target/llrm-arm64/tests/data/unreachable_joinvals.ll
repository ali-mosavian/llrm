; ModuleID = '<bc file>'
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define ptr @scop_label(ptr %0, ptr %1, i16 %2) {
  %4 = call i1 @scop_lany(ptr %0, i16 %2)
  br i1 %4, label %5, label %6

5:                                                ; preds = %3
  unreachable

6:                                                ; preds = %3
  %7 = getelementptr i8, ptr %0, i64 80
  %8 = call i1 @grow_room(ptr null, ptr %7)
  %9 = load ptr, ptr %0, align 8
  ret ptr %9
}

define i1 @scop_lany(ptr %0, i16 %1) {
  %.not = icmp sgt i16 %1, 0
  br i1 %.not, label %10, label %3

3:                                                ; preds = %2
  %4 = getelementptr i8, ptr %0, i64 80
  %5 = load i32, ptr %4, align 8
  %.not2 = icmp eq i32 %5, 0
  br i1 %.not2, label %9, label %6

6:                                                ; preds = %3
  %7 = load i32, ptr %0, align 4
  %8 = trunc i32 %7 to i1
  ret i1 %8

9:                                                ; preds = %3
  ret i1 false

10:                                               ; preds = %2
  store i1 false, ptr null, align 1
  ret i1 false
}

declare i1 @grow_room(ptr, ptr)
