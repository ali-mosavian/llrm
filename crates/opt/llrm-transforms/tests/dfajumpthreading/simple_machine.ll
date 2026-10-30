; A loop whose switch runs on a state each case sets to a constant: every
; path from a case to the switch is copied and enters the next case directly.
define i32 @f(i32 %n) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %next, %latch ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]
  %acc = phi i32 [ 0, %entry ], [ %acc.next, %latch ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

dispatch:
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
    i32 2, label %case2
  ]

case0:
  %a0 = add i32 %acc, 1
  br label %latch

case1:
  %a1 = mul i32 %acc, 3
  br label %latch

case2:
  %a2 = sub i32 %acc, 2
  br label %latch

case.default:
  br label %latch

latch:
  %next = phi i32 [ 1, %case0 ], [ 2, %case1 ], [ 0, %case2 ], [ 0, %case.default ]
  %acc.next = phi i32 [ %a0, %case0 ], [ %a1, %case1 ], [ %a2, %case2 ], [ %acc, %case.default ]
  %i.next = add i32 %i, 1
  br label %loop

done:
  ret i32 %acc
}
