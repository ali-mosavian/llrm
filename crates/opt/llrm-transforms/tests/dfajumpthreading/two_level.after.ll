define i32 @f(i32 %n, i1 %c) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %next, %mid ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %mid ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

loop.jt0:
  %state.jt0 = phi i32 [ %next.jt0, %mid.jt0 ]
  %i.jt0 = phi i32 [ %i.next.jt0, %mid.jt0 ]
  %go.jt0 = icmp slt i32 %i.jt0, %n
  br i1 %go.jt0, label %dispatch.jt0, label %done

loop.jt1:
  %state.jt1 = phi i32 [ %next.jt1, %mid.jt1 ]
  %i.jt1 = phi i32 [ %i.next.jt1, %mid.jt1 ]
  %go.jt1 = icmp slt i32 %i.jt1, %n
  br i1 %go.jt1, label %dispatch.jt1, label %done

dispatch:
  switch i32 %state, label %body.default [
    i32 0, label %body0
    i32 1, label %body1
  ]

dispatch.jt0:
  br label %body0

dispatch.jt1:
  br label %body1

body0:
  %i3 = phi i32 [ %i.jt0, %dispatch.jt0 ], [ %i, %dispatch ]
  br i1 %c, label %then0, label %else0

then0:
  br label %merge.jt1

else0:
  br label %merge.jt0

body1:
  %i2 = phi i32 [ %i.jt1, %dispatch.jt1 ], [ %i, %dispatch ]
  br label %merge1

body.default:
  br label %merge1

merge:
  br label %mid

merge.jt0:
  %m.jt0 = phi i32 [ 0, %else0 ]
  br label %mid.jt0

merge.jt1:
  %m.jt1 = phi i32 [ 1, %then0 ]
  br label %mid.jt1

merge1:
  %i4 = phi i32 [ %i, %body.default ], [ %i2, %body1 ]
  br label %mid1

mid1:
  br label %mid.jt0

mid:
  %next = phi i32 [ poison, %merge ]
  %i.next = add i32 undef, 1
  br label %loop

mid.jt0:
  %i5 = phi i32 [ %i4, %mid1 ], [ %i3, %merge.jt0 ]
  %next.jt0 = phi i32 [ 0, %mid1 ], [ %m.jt0, %merge.jt0 ]
  %i.next.jt0 = add i32 %i5, 1
  br label %loop.jt0

mid.jt1:
  %next.jt1 = phi i32 [ %m.jt1, %merge.jt1 ]
  %i.next.jt1 = add i32 %i3, 1
  br label %loop.jt1

done:
  %i1 = phi i32 [ %i.jt0, %loop.jt0 ], [ %i.jt1, %loop.jt1 ], [ %i, %loop ]
  ret i32 %i1
}
