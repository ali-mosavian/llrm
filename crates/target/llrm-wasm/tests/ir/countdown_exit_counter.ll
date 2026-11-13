target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"

; `n` counts down to 0 while `acc` is multiplied and added to: runtime unrolled
; by 8, then the exit counter of the unrolled loop is the issue.
define i32 @countdown(i32 %n, i32 %acc) {
entry:
  %z = icmp eq i32 %n, 0
  br i1 %z, label %exit, label %loop

loop:
  %i = phi i32 [ %n, %entry ], [ %i.next, %loop ]
  %a = phi i32 [ %acc, %entry ], [ %a.next, %loop ]
  %m = mul nsw i32 %a, 3
  %a.next = add nsw i32 %m, %i
  %i.next = add nsw i32 %i, -1
  %c = icmp eq i32 %i.next, 0
  br i1 %c, label %exit, label %loop

exit:
  %r = phi i32 [ %acc, %entry ], [ %a.next, %loop ]
  ret i32 %r
}
