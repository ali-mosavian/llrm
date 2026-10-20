define i32 @f(i32 %n) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ poison, %latch ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
  ]

loop.jt0:
  %state.jt0 = phi i32 [ %next.jt0, %latch.jt0 ]
  %i.jt0 = phi i32 [ %i.next.jt0, %latch.jt0 ]
  br label %case0

loop.jt1:
  %state.jt1 = phi i32 [ %next.jt1, %latch.jt1 ]
  %i.jt1 = phi i32 [ %i.next.jt1, %latch.jt1 ]
  br label %case1

case0:
  %i2 = phi i32 [ %i.jt0, %loop.jt0 ], [ %i, %loop ]
  call void @sink(i32 0)
  br label %latch.jt1

case1:
  %i1 = phi i32 [ %i.jt1, %loop.jt1 ], [ %i, %loop ]
  call void @sink(i32 1)
  br label %latch.jt0

case.default:
  br label %latch.jt0

latch:
  %i.next = add i32 undef, 1
  %go = icmp slt i32 %i.next, %n
  br i1 %go, label %loop, label %done

latch.jt0:
  %i3 = phi i32 [ %i, %case.default ], [ %i1, %case1 ]
  %next.jt0 = phi i32 [ 0, %case.default ], [ 0, %case1 ]
  %i.next.jt0 = add i32 %i3, 1
  %go.jt0 = icmp slt i32 %i.next.jt0, %n
  br i1 %go.jt0, label %loop.jt0, label %done

latch.jt1:
  %next.jt1 = phi i32 [ 1, %case0 ]
  %i.next.jt1 = add i32 %i2, 1
  %go.jt1 = icmp slt i32 %i.next.jt1, %n
  br i1 %go.jt1, label %loop.jt1, label %done

done:
  %i.next1 = phi i32 [ %i.next.jt0, %latch.jt0 ], [ %i.next.jt1, %latch.jt1 ], [ %i.next, %latch ]
  ret i32 %i.next1
}

declare void @sink(i32)
