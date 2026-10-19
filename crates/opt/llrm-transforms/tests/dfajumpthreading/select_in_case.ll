; One case sets the state with a select, the others with constants: only the
; select's path is unfolded, the state phi is in the join after the cases.
define i32 @f(i32 %n, i1 %c) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %next, %latch ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

dispatch:
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
    i32 2, label %case2
  ]

case0:
  br label %latch

case1:
  %pick = select i1 %c, i32 2, i32 0
  br label %latch

case2:
  br label %latch

case.default:
  br label %latch

latch:
  %next = phi i32 [ 1, %case0 ], [ %pick, %case1 ], [ 1, %case2 ], [ 0, %case.default ]
  %i.next = add i32 %i, 1
  br label %loop

done:
  ret i32 %i
}
