define i32 @f(i32 %n, i1 %c) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %sel, %latch ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]
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
  %sel = select i1 %c, i32 1, i32 0
  %i.next = add i32 %i, %sel
  %go = icmp slt i32 %i.next, %n
  br i1 %go, label %loop, label %done

done:
  ret i32 %i.next
}
