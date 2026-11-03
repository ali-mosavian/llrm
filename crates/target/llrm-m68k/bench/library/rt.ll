target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"

; The runtime the libcalls of a 68000 name (libgcc's `__mulsi3` and the
; others), as shift-and-add and shift-and-subtract loops that use no
; multiply or divide themselves. Only for the run tests.

define i32 @__mulsi3(i32 %a, i32 %b) {
entry:
  br label %loop
loop:
  %acc = phi i32 [ 0, %entry ], [ %acc2, %loop ]
  %x = phi i32 [ %a, %entry ], [ %x2, %loop ]
  %y = phi i32 [ %b, %entry ], [ %y2, %loop ]
  %bit = and i32 %y, 1
  %mask = sub i32 0, %bit
  %add = and i32 %x, %mask
  %acc2 = add i32 %acc, %add
  %x2 = shl i32 %x, 1
  %y2 = lshr i32 %y, 1
  %more = icmp ne i32 %y2, 0
  br i1 %more, label %loop, label %done
done:
  ret i32 %acc2
}

define i32 @__udivsi3(i32 %n, i32 %d) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 32, %entry ], [ %i2, %loop ]
  %q = phi i32 [ 0, %entry ], [ %q2, %loop ]
  %r = phi i32 [ 0, %entry ], [ %r3, %loop ]
  %m = phi i32 [ %n, %entry ], [ %m2, %loop ]
  %top = lshr i32 %m, 31
  %r1 = shl i32 %r, 1
  %r2 = or i32 %r1, %top
  %m2 = shl i32 %m, 1
  %ge = icmp uge i32 %r2, %d
  %diff = sub i32 %r2, %d
  %r3 = select i1 %ge, i32 %diff, i32 %r2
  %q1 = shl i32 %q, 1
  %bit = zext i1 %ge to i32
  %q2 = or i32 %q1, %bit
  %i2 = add i32 %i, -1
  %more = icmp ne i32 %i2, 0
  br i1 %more, label %loop, label %done
done:
  ret i32 %q2
}

define i32 @__umodsi3(i32 %n, i32 %d) {
entry:
  br label %loop
loop:
  %i = phi i32 [ 32, %entry ], [ %i2, %loop ]
  %r = phi i32 [ 0, %entry ], [ %r3, %loop ]
  %m = phi i32 [ %n, %entry ], [ %m2, %loop ]
  %top = lshr i32 %m, 31
  %r1 = shl i32 %r, 1
  %r2 = or i32 %r1, %top
  %m2 = shl i32 %m, 1
  %ge = icmp uge i32 %r2, %d
  %diff = sub i32 %r2, %d
  %r3 = select i1 %ge, i32 %diff, i32 %r2
  %i2 = add i32 %i, -1
  %more = icmp ne i32 %i2, 0
  br i1 %more, label %loop, label %done
done:
  ret i32 %r3
}

define i32 @__divsi3(i32 %a, i32 %b) {
  %an = icmp slt i32 %a, 0
  %bn = icmp slt i32 %b, 0
  %na = sub i32 0, %a
  %nb = sub i32 0, %b
  %ua = select i1 %an, i32 %na, i32 %a
  %ub = select i1 %bn, i32 %nb, i32 %b
  %q = call i32 @__udivsi3(i32 %ua, i32 %ub)
  %flip = xor i1 %an, %bn
  %nq = sub i32 0, %q
  %r = select i1 %flip, i32 %nq, i32 %q
  ret i32 %r
}

define i32 @__modsi3(i32 %a, i32 %b) {
  %an = icmp slt i32 %a, 0
  %bn = icmp slt i32 %b, 0
  %na = sub i32 0, %a
  %nb = sub i32 0, %b
  %ua = select i1 %an, i32 %na, i32 %a
  %ub = select i1 %bn, i32 %nb, i32 %b
  %m = call i32 @__umodsi3(i32 %ua, i32 %ub)
  %nm = sub i32 0, %m
  %r = select i1 %an, i32 %nm, i32 %m
  ret i32 %r
}
