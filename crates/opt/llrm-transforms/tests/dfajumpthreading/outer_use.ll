; %i and %acc are defined in the block that is copied per state and read after
; the loop: the exit gets a phi of the copies.
define i32 @f(i32 %n) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %next, %latch ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]
  %acc = phi i32 [ 1, %entry ], [ %acc.next, %latch ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

dispatch:
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
  ]

case0:
  %a0 = add i32 %acc, %i
  br label %latch

case1:
  %a1 = shl i32 %acc, 1
  br label %latch

case.default:
  br label %latch

latch:
  %next = phi i32 [ 1, %case0 ], [ 0, %case1 ], [ 0, %case.default ]
  %acc.next = phi i32 [ %a0, %case0 ], [ %a1, %case1 ], [ %acc, %case.default ]
  %i.next = add i32 %i, 1
  br label %loop

done:
  %r = add i32 %i, %acc
  ret i32 %r
}
