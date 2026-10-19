define i32 @f(i32 %n, i1 %c) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %next, %latch ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %latch ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

loop.jt0:
  %state.jt0 = phi i32 [ %next.jt0, %latch.jt0 ]
  %i.jt0 = phi i32 [ %i.next.jt0, %latch.jt0 ]
  %go.jt0 = icmp slt i32 %i.jt0, %n
  br i1 %go.jt0, label %dispatch.jt0, label %done

loop.jt2:
  %state.jt2 = phi i32 [ %next.jt2, %latch.jt2 ]
  %i.jt2 = phi i32 [ %i.next.jt2, %latch.jt2 ]
  %go.jt2 = icmp slt i32 %i.jt2, %n
  br i1 %go.jt2, label %dispatch.jt2, label %done

loop.jt1:
  %state.jt1 = phi i32 [ %next.jt1, %latch.jt1 ]
  %i.jt1 = phi i32 [ %i.next.jt1, %latch.jt1 ]
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
  %i4 = phi i32 [ %i.jt0, %dispatch.jt0 ], [ %i, %dispatch ]
  br label %latch.jt1

case1:
  %i3 = phi i32 [ %i.jt1, %dispatch.jt1 ], [ %i, %dispatch ]
  br i1 %c, label %latch.jt2, label %pick.si.unfold.false.jt0

case2:
  %i2 = phi i32 [ %i.jt2, %dispatch.jt2 ], [ %i, %dispatch ]
  br label %latch.jt1

case.default:
  br label %latch.jt0

pick.si.unfold.false:
  br label %latch

pick.si.unfold.false.jt0:
  %.si.unfold.phi.jt0 = phi i32 [ 0, %case1 ]
  br label %latch.jt0

latch:
  %next = phi i32 [ poison, %pick.si.unfold.false ]
  %i.next = add i32 undef, 1
  br label %loop

latch.jt0:
  %i6 = phi i32 [ %i3, %pick.si.unfold.false.jt0 ], [ %i, %case.default ]
  %next.jt0 = phi i32 [ 0, %case.default ], [ %.si.unfold.phi.jt0, %pick.si.unfold.false.jt0 ]
  %i.next.jt0 = add i32 %i6, 1
  br label %loop.jt0

latch.jt2:
  %next.jt2 = phi i32 [ 2, %case1 ]
  %i.next.jt2 = add i32 %i3, 1
  br label %loop.jt2

latch.jt1:
  %i5 = phi i32 [ %i2, %case2 ], [ %i4, %case0 ]
  %next.jt1 = phi i32 [ 1, %case0 ], [ 1, %case2 ]
  %i.next.jt1 = add i32 %i5, 1
  br label %loop.jt1

done:
  %i1 = phi i32 [ %i.jt0, %loop.jt0 ], [ %i.jt2, %loop.jt2 ], [ %i.jt1, %loop.jt1 ], [ %i, %loop ]
  ret i32 %i1
}
