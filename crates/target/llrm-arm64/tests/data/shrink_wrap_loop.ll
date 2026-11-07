target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; recpow's `power` after the mid-end: a loop that needs no frame, laid out
; before the join that holds the recursive call.
define internal i64 @power(i64 %x, i32 %n) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %ret, label %pre
pre:
  %b = and i32 %n, 1
  %e = icmp eq i32 %b, 0
  br i1 %e, label %loop, label %call
ret:
  %r = phi i64 [ %s, %call ], [ 1, %entry ]
  ret i64 %r
call:
  %n.c = phi i32 [ %n, %pre ], [ %n.next, %loop ]
  %x.c = phi i64 [ %x, %pre ], [ %sq.r, %loop ]
  %nm = add nsw i32 %n.c, -1
  %p = tail call i64 @power(i64 %x.c, i32 %nm)
  %m = mul nsw i64 %p, %x.c
  %s = srem i64 %m, 30011
  br label %ret
loop:
  %x.l = phi i64 [ %sq.r, %loop ], [ %x, %pre ]
  %n.l = phi i32 [ %n.next, %loop ], [ %n, %pre ]
  %sq = mul nsw i64 %x.l, %x.l
  %sq.r = urem i64 %sq, 30011
  %n.next = ashr exact i32 %n.l, 1
  %t = and i32 %n.l, 2
  %again = icmp eq i32 %t, 0
  br i1 %again, label %loop, label %call
}

define i64 @entry_point(i64 %x, i32 %n) {
  %r = call i64 @power(i64 %x, i32 %n)
  ret i64 %r
}
