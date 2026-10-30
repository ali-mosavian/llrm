define i32 @f(i32 %n) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ poison, %latch ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

loop.jt0:
  %state.jt0 = phi i32 [ %next.jt0, %latch.jt0 ]
  %i.jt0 = phi i32 [ %i.next.jt0, %latch.jt0 ]
  %go.jt0 = icmp slt i32 %i.jt0, %n
  br i1 %go.jt0, label %dispatch.jt0, label %done

loop.jt5:
  %state.jt5 = phi i32 [ %next.jt5, %latch.jt5 ]
  %i.jt5 = phi i32 [ %i.next.jt5, %latch.jt5 ]
  %go.jt5 = icmp slt i32 %i.jt5, %n
  br i1 %go.jt5, label %dispatch.jt5, label %done

dispatch:
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
  ]

dispatch.jt0:
  br label %case0

dispatch.jt5:
  br label %case.default

case0:
  %i2 = phi i32 [ %i.jt0, %dispatch.jt0 ], [ %i, %dispatch ]
  br label %latch.jt5

case1:
  br label %latch.jt5

case.default:
  %i3 = phi i32 [ %i.jt5, %dispatch.jt5 ], [ %i, %dispatch ]
  br label %latch.jt0

latch:
  %i.next = add i32 undef, 1
  br label %loop

latch.jt0:
  %next.jt0 = phi i32 [ 0, %case.default ]
  %i.next.jt0 = add i32 %i3, 1
  br label %loop.jt0

latch.jt5:
  %i4 = phi i32 [ %i, %case1 ], [ %i2, %case0 ]
  %next.jt5 = phi i32 [ 5, %case0 ], [ 7, %case1 ]
  %i.next.jt5 = add i32 %i4, 1
  br label %loop.jt5

done:
  %i1 = phi i32 [ %i.jt0, %loop.jt0 ], [ %i.jt5, %loop.jt5 ], [ %i, %loop ]
  ret i32 %i1
}
