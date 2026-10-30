define i32 @f(i32 %n) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ poison, %latch ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]
  %acc = phi i32 [ 1, %entry ], [ poison, %latch ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

loop.jt0:
  %state.jt0 = phi i32 [ %next.jt0, %latch.jt0 ]
  %i.jt0 = phi i32 [ %i.next.jt0, %latch.jt0 ]
  %acc.jt0 = phi i32 [ %acc.next.jt0, %latch.jt0 ]
  %go.jt0 = icmp slt i32 %i.jt0, %n
  br i1 %go.jt0, label %dispatch.jt0, label %done

loop.jt1:
  %state.jt1 = phi i32 [ %next.jt1, %latch.jt1 ]
  %i.jt1 = phi i32 [ %i.next.jt1, %latch.jt1 ]
  %acc.jt1 = phi i32 [ %acc.next.jt1, %latch.jt1 ]
  %go.jt1 = icmp slt i32 %i.jt1, %n
  br i1 %go.jt1, label %dispatch.jt1, label %done

dispatch:
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
  ]

dispatch.jt0:
  br label %case0

dispatch.jt1:
  br label %case1

case0:
  %acc3 = phi i32 [ %acc.jt0, %dispatch.jt0 ], [ %acc, %dispatch ]
  %i3 = phi i32 [ %i.jt0, %dispatch.jt0 ], [ %i, %dispatch ]
  %a0 = add i32 %acc3, %i3
  br label %latch.jt1

case1:
  %acc2 = phi i32 [ %acc.jt1, %dispatch.jt1 ], [ %acc, %dispatch ]
  %i2 = phi i32 [ %i.jt1, %dispatch.jt1 ], [ %i, %dispatch ]
  %a1 = shl i32 %acc2, 1
  br label %latch.jt0

case.default:
  br label %latch.jt0

latch:
  %i.next = add i32 undef, 1
  br label %loop

latch.jt0:
  %i4 = phi i32 [ %i, %case.default ], [ %i2, %case1 ]
  %next.jt0 = phi i32 [ 0, %case.default ], [ 0, %case1 ]
  %acc.next.jt0 = phi i32 [ %acc, %case.default ], [ %a1, %case1 ]
  %i.next.jt0 = add i32 %i4, 1
  br label %loop.jt0

latch.jt1:
  %next.jt1 = phi i32 [ 1, %case0 ]
  %acc.next.jt1 = phi i32 [ %a0, %case0 ]
  %i.next.jt1 = add i32 %i3, 1
  br label %loop.jt1

done:
  %acc1 = phi i32 [ %acc.jt0, %loop.jt0 ], [ %acc.jt1, %loop.jt1 ], [ %acc, %loop ]
  %i1 = phi i32 [ %i.jt0, %loop.jt0 ], [ %i.jt1, %loop.jt1 ], [ %i, %loop ]
  %r = add i32 %i1, %acc1
  ret i32 %r
}
