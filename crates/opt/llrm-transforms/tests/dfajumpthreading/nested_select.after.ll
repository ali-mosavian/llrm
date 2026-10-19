define i32 @f(i32 %n, i1 %c, i1 %d) {
entry:
  br label %loop

inner.si.unfold.true:
  br i1 %d, label %sel.si.unfold.false, label %inner.si.unfold.false.jt2

inner.si.unfold.true.jt3:
  %.si.unfold.phi.jt3 = phi i32 [ 3, %body ]
  br i1 %d, label %sel.si.unfold.false.jt3, label %inner.si.unfold.false.jt2

inner.si.unfold.false:
  br label %sel.si.unfold.false

inner.si.unfold.false.jt2:
  %.si.unfold.phi1.jt2 = phi i32 [ 2, %inner.si.unfold.true ], [ 2, %inner.si.unfold.true.jt3 ]
  br label %sel.si.unfold.false.jt2

sel.si.unfold.false:
  %inner.si.unfold.phi = phi i32 [ poison, %inner.si.unfold.false ], [ poison, %inner.si.unfold.true ]
  br label %loop

sel.si.unfold.false.jt3:
  %inner.si.unfold.phi.jt3 = phi i32 [ %.si.unfold.phi.jt3, %inner.si.unfold.true.jt3 ]
  br label %loop.jt3

sel.si.unfold.false.jt2:
  %inner.si.unfold.phi.jt2 = phi i32 [ %.si.unfold.phi1.jt2, %inner.si.unfold.false.jt2 ]
  br label %loop.jt2

loop:
  %state = phi i32 [ 0, %entry ], [ %inner.si.unfold.phi, %sel.si.unfold.false ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %sel.si.unfold.false ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

loop.jt3:
  %state.jt3 = phi i32 [ %inner.si.unfold.phi.jt3, %sel.si.unfold.false.jt3 ]
  %i.jt3 = phi i32 [ %i.next, %sel.si.unfold.false.jt3 ]
  %go.jt3 = icmp slt i32 %i.jt3, %n
  br i1 %go.jt3, label %dispatch.jt3, label %done

loop.jt2:
  %state.jt2 = phi i32 [ %inner.si.unfold.phi.jt2, %sel.si.unfold.false.jt2 ]
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
    i32 3, label %case3
  ]

dispatch.jt3:
  br label %case3

dispatch.jt2:
  br label %case2

dispatch.jt1:
  br label %case1

case0:
  br label %body

case1:
  %i4 = phi i32 [ %i.jt1, %dispatch.jt1 ], [ %i, %dispatch ]
  br label %body

case2:
  %i2 = phi i32 [ %i.jt2, %dispatch.jt2 ], [ %i, %dispatch ]
  br label %body

case3:
  %i3 = phi i32 [ %i.jt3, %dispatch.jt3 ], [ %i, %dispatch ]
  br label %body

case.default:
  br label %body

body:
  %i5 = phi i32 [ %i, %case.default ], [ %i3, %case3 ], [ %i2, %case2 ], [ %i4, %case1 ], [ %i, %case0 ]
  %i.next = add i32 %i5, 1
  br i1 %c, label %loop.jt1, label %inner.si.unfold.true.jt3

done:
  %i1 = phi i32 [ %i.jt3, %loop.jt3 ], [ %i.jt2, %loop.jt2 ], [ %i.jt1, %loop.jt1 ], [ %i, %loop ]
  ret i32 %i1
}
