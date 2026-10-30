define i32 @f(i32 %n) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ poison, %latch ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]
  %acc = phi i32 [ 0, %entry ], [ poison, %latch ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

loop.jt0:
  %state.jt0 = phi i32 [ %next.jt0, %latch.jt0 ]
  %i.jt0 = phi i32 [ %i.next.jt0, %latch.jt0 ]
  %acc.jt0 = phi i32 [ %acc.next.jt0, %latch.jt0 ]
  %go.jt0 = icmp slt i32 %i.jt0, %n
  br i1 %go.jt0, label %dispatch.jt0, label %done

loop.jt2:
  %state.jt2 = phi i32 [ %next.jt2, %latch.jt2 ]
  %i.jt2 = phi i32 [ %i.next.jt2, %latch.jt2 ]
  %acc.jt2 = phi i32 [ %acc.next.jt2, %latch.jt2 ]
  %go.jt2 = icmp slt i32 %i.jt2, %n
  br i1 %go.jt2, label %dispatch.jt2, label %done

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
    i32 2, label %case2
  ]

dispatch.jt0:
  br label %case0

dispatch.jt2:
  br label %case2

dispatch.jt1:
  br label %case1

case0:
  %acc4 = phi i32 [ %acc.jt0, %dispatch.jt0 ], [ %acc, %dispatch ]
  %i3 = phi i32 [ %i.jt0, %dispatch.jt0 ], [ %i, %dispatch ]
  %a0 = add i32 %acc4, 1
  br label %latch.jt1

case1:
  %acc3 = phi i32 [ %acc.jt1, %dispatch.jt1 ], [ %acc, %dispatch ]
  %i2 = phi i32 [ %i.jt1, %dispatch.jt1 ], [ %i, %dispatch ]
  %a1 = mul i32 %acc3, 3
  br label %latch.jt2

case2:
  %acc2 = phi i32 [ %acc.jt2, %dispatch.jt2 ], [ %acc, %dispatch ]
  %i1 = phi i32 [ %i.jt2, %dispatch.jt2 ], [ %i, %dispatch ]
  %a2 = sub i32 %acc2, 2
  br label %latch.jt0

case.default:
  br label %latch.jt0

latch:
  %i.next = add i32 undef, 1
  br label %loop

latch.jt0:
  %i4 = phi i32 [ %i, %case.default ], [ %i1, %case2 ]
  %next.jt0 = phi i32 [ 0, %case.default ], [ 0, %case2 ]
  %acc.next.jt0 = phi i32 [ %acc, %case.default ], [ %a2, %case2 ]
  %i.next.jt0 = add i32 %i4, 1
  br label %loop.jt0

latch.jt2:
  %next.jt2 = phi i32 [ 2, %case1 ]
  %acc.next.jt2 = phi i32 [ %a1, %case1 ]
  %i.next.jt2 = add i32 %i2, 1
  br label %loop.jt2

latch.jt1:
  %next.jt1 = phi i32 [ 1, %case0 ]
  %acc.next.jt1 = phi i32 [ %a0, %case0 ]
  %i.next.jt1 = add i32 %i3, 1
  br label %loop.jt1

done:
  %acc1 = phi i32 [ %acc.jt0, %loop.jt0 ], [ %acc.jt2, %loop.jt2 ], [ %acc.jt1, %loop.jt1 ], [ %acc, %loop ]
  ret i32 %acc1
}
