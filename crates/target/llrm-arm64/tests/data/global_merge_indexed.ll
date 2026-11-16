target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

@ta = internal global [256 x i8] zeroinitializer, align 1
@tb = internal global [256 x i8] zeroinitializer, align 1
@ua = internal global [256 x i8] zeroinitializer, align 1
@ub = internal global [256 x i8] zeroinitializer, align 1

; Both tables are read through a byte the loop loads: no addressing mode takes
; `merged + offset + index`, so merging them costs an `add` per access.
define i64 @data(ptr %p, i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %sum = phi i64 [ 0, %entry ], [ %add, %loop ]
  %at = getelementptr i8, ptr %p, i64 %i
  %c = load volatile i8, ptr %at
  %x = zext i8 %c to i64
  %ga = getelementptr [256 x i8], ptr @ta, i64 0, i64 %x
  %v = load volatile i8, ptr %ga
  %y = zext i8 %v to i64
  %gb = getelementptr [256 x i8], ptr @tb, i64 0, i64 %y
  %w = load volatile i8, ptr %gb
  %z = zext i8 %w to i64
  %add = add i64 %sum, %z
  %next = add i64 %i, 1
  %done = icmp eq i64 %next, %n
  br i1 %done, label %exit, label %loop
exit:
  ret i64 %add
}

; The same two tables indexed by the induction variable: strength reduction
; steps one pointer, and the merged base costs nothing.
define i64 @steps(i64 %n) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %sum = phi i64 [ 0, %entry ], [ %add, %loop ]
  %ga = getelementptr [256 x i8], ptr @ua, i64 0, i64 %i
  %v = load volatile i8, ptr %ga
  %gb = getelementptr [256 x i8], ptr @ub, i64 0, i64 %i
  %w = load volatile i8, ptr %gb
  %y = zext i8 %v to i64
  %z = zext i8 %w to i64
  %s = add i64 %y, %z
  %add = add i64 %sum, %s
  %next = add i64 %i, 1
  %done = icmp eq i64 %next, %n
  br i1 %done, label %exit, label %loop
exit:
  ret i64 %add
}
