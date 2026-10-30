define i32 @f(i32 %n, i1 %c) {
entry:
  br label %loop

sel.si.unfold.false:
  br label %loop

sel.si.unfold.false.jt2:
  %.si.unfold.phi.jt2 = phi i32 [ 2, %body ]
  br label %loop.jt2

loop:
  %state = phi i32 [ 0, %entry ], [ poison, %sel.si.unfold.false ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %sel.si.unfold.false ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

loop.jt2:
  %state.jt2 = phi i32 [ %.si.unfold.phi.jt2, %sel.si.unfold.false.jt2 ]
  %i.jt2 = phi i32 [ %i.next, %sel.si.unfold.false.jt2 ]
  %go.jt2 = icmp slt i32 %i.jt2, %n
  br i1 %go.jt2, label %dispatch.jt2, label %done

loop.jt1:
  %state.jt1 = phi i32 [ 1, %body ]
  %i.jt1 = phi i32 [ %i.next, %body ]
  %go.jt1 = icmp slt i32 %i.jt1, %n
  br i1 %go.jt1, label %dispatch.jt1, label %done

dispatch:
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
    i32 2, label %case2
  ]

dispatch.jt2:
  br label %case2

dispatch.jt1:
  br label %case1

case0:
  br label %body

case1:
  %i3 = phi i32 [ %i.jt1, %dispatch.jt1 ], [ %i, %dispatch ]
  br label %body

case2:
  %i2 = phi i32 [ %i.jt2, %dispatch.jt2 ], [ %i, %dispatch ]
  br label %body

case.default:
  br label %body

body:
  %i4 = phi i32 [ %i, %case.default ], [ %i2, %case2 ], [ %i3, %case1 ], [ %i, %case0 ]
  %i.next = add i32 %i4, 1
  br i1 %c, label %loop.jt1, label %sel.si.unfold.false.jt2

done:
  %i1 = phi i32 [ %i.jt2, %loop.jt2 ], [ %i.jt1, %loop.jt1 ], [ %i, %loop ]
  ret i32 %i1
}
