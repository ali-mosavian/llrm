target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define i64 @f(ptr noalias %p, i16 %limit, i16 %c0, i64 %s0) {
entry:
  %n = zext i16 %limit to i64
  br label %loop
loop:
  %i = phi i64 [ 32, %entry ], [ %next, %loop ]
  %sum = phi i64 [ %s0, %entry ], [ %sum.next, %loop ]
  %count = phi i16 [ %c0, %entry ], [ %count.next, %loop ]
  %a = getelementptr inbounds nuw i8, ptr %p, i64 %i
  %x = load i8, ptr %a, align 1
  %z = icmp eq i8 %x, 0
  %zi = zext i1 %z to i16
  %count.next = add i16 %count, %zi
  %pick = select i1 %z, i64 %i, i64 0
  %sum.next = add i64 %pick, %sum
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, %n
  br i1 %done, label %exit, label %loop
exit:
  %c = phi i16 [ %count.next, %loop ]
  %s = phi i64 [ %sum.next, %loop ]
  %ce = zext i16 %c to i64
  %sh = shl nuw nsw i64 %ce, 16
  %r = xor i64 %sh, %s
  ret i64 %r
}
