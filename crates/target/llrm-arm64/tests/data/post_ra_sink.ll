target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; rectwo's `paths` after the mid-end: the second argument lives across the
; recursive call, so it sits in a callee-saved register.
define internal i64 @paths(i32 %n, i32 %m) {
entry:
  %a = icmp eq i32 %n, 0
  %b = icmp eq i32 %m, 0
  %c = or i1 %a, %b
  br i1 %c, label %exit, label %pre
pre:
  %n1 = add i32 %n, -1
  br label %loop
loop:
  %m.i = phi i32 [ %m, %pre ], [ %m.next, %loop ]
  %acc = phi i64 [ 0, %pre ], [ %acc.next, %loop ]
  %r = tail call i64 @paths(i32 %n1, i32 %m.i)
  %m.next = add i32 %m.i, -1
  %acc.next = add i64 %r, %acc
  %d = icmp eq i32 %m.next, 0
  br i1 %d, label %fin, label %loop
fin:
  %t = add i64 %acc.next, 1
  br label %exit
exit:
  %res = phi i64 [ 1, %entry ], [ %t, %fin ]
  ret i64 %res
}

define i64 @entry_point(i32 %n) {
  %r = call i64 @paths(i32 %n, i32 %n)
  ret i64 %r
}
