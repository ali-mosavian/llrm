; The state also comes from a load in the loop: the early exit heuristic
; refuses the switch.
define i32 @f(ptr %p, i32 %n) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %next, %latch ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
  ]

case0:
  br label %latch

case1:
  %v = load i32, ptr %p
  br label %latch

case.default:
  br label %latch

latch:
  %next = phi i32 [ 1, %case0 ], [ %v, %case1 ], [ 0, %case.default ]
  %i.next = add i32 %i, 1
  %go = icmp slt i32 %i.next, %n
  br i1 %go, label %loop, label %done

done:
  ret i32 %i.next
}
