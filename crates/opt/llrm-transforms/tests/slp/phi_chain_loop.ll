target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define double @f(ptr %p, i32 %n) {
entry:
  br label %loop

loop:
  %i = phi i32 [ 0, %entry ], [ %i.next, %loop ]
  %a = phi double [ 0.0, %entry ], [ %a.next, %loop ]
  %b = phi double [ 1.0, %entry ], [ %b.next, %loop ]
  %q0 = getelementptr inbounds double, ptr %p, i32 %i
  %x = load double, ptr %q0, align 8
  %q1 = getelementptr inbounds i8, ptr %q0, i64 8
  %y = load double, ptr %q1, align 8
  %a.next = fadd double %a, %x
  %b.next = fadd double %b, %a.next
  %i.next = add nuw nsw i32 %i, 2
  %c = icmp slt i32 %i.next, %n
  br i1 %c, label %loop, label %exit

exit:
  %r = fadd double %a.next, 1.0
  ret double %r
}
