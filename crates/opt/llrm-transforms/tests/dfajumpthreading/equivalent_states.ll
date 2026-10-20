; 5 and 7 both reach the default: the paths that set them share one copy.
define i32 @f(i32 %n) {
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
  ]

case0:
  br label %latch

case1:
  br label %latch

case.default:
  br label %latch

latch:
  %next = phi i32 [ 5, %case0 ], [ 7, %case1 ], [ 0, %case.default ]
  %i.next = add i32 %i, 1
  br label %loop

done:
  ret i32 %i
}
