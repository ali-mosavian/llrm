define i32 @f(i32 %n, i1 %c, i1 %d) {
entry:
  br label %loop

sel.si.unfold.false:
  br label %loop

sel.si.unfold.false.jt2:
  %.si.unfold.phi.jt2 = phi i32 [ 2, %body ]
  br label %loop.jt2

inner.si.unfold.true:
  br i1 %d, label %loop, label %inner.si.unfold.false.jt3

inner.si.unfold.true.jt1:
  %.si.unfold.phi1.jt1 = phi i32 [ 1, %body ]
  br i1 %d, label %loop.jt1, label %inner.si.unfold.false.jt3

inner.si.unfold.false:
  br label %loop

inner.si.unfold.false.jt3:
  %.si.unfold.phi2.jt3 = phi i32 [ 3, %inner.si.unfold.true ], [ 3, %inner.si.unfold.true.jt1 ]
  br label %loop.jt3

loop:
  %state = phi i32 [ 0, %entry ], [ poison, %inner.si.unfold.false ], [ poison, %sel.si.unfold.false ], [ poison, %inner.si.unfold.true ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %inner.si.unfold.false ], [ %i.next, %sel.si.unfold.false ], [ %i.next, %inner.si.unfold.true ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

loop.jt1:
  %state.jt1 = phi i32 [ %.si.unfold.phi1.jt1, %inner.si.unfold.true.jt1 ]
  %i.jt1 = phi i32 [ %i.next, %inner.si.unfold.true.jt1 ]
  %go.jt1 = icmp slt i32 %i.jt1, %n
  br i1 %go.jt1, label %dispatch.jt1, label %done

loop.jt2:
  %state.jt2 = phi i32 [ %.si.unfold.phi.jt2, %sel.si.unfold.false.jt2 ]
  %i.jt2 = phi i32 [ %i.next, %sel.si.unfold.false.jt2 ]
  %go.jt2 = icmp slt i32 %i.jt2, %n
  br i1 %go.jt2, label %dispatch.jt2, label %done

loop.jt3:
  %state.jt3 = phi i32 [ %.si.unfold.phi2.jt3, %inner.si.unfold.false.jt3 ]
  %i.jt3 = phi i32 [ %i.next, %inner.si.unfold.false.jt3 ]
  %go.jt3 = icmp slt i32 %i.jt3, %n
  br i1 %go.jt3, label %dispatch.jt3, label %done

dispatch:
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
    i32 2, label %case2
    i32 3, label %case3
  ]

dispatch.jt1:
  br label %case1

dispatch.jt2:
  br label %case2

dispatch.jt3:
  br label %case3

case0:
  br label %body

case1:
  %i4 = phi i32 [ %i.jt1, %dispatch.jt1 ], [ %i, %dispatch ]
  br label %body

case2:
  %i3 = phi i32 [ %i.jt2, %dispatch.jt2 ], [ %i, %dispatch ]
  br label %body

case3:
  %i2 = phi i32 [ %i.jt3, %dispatch.jt3 ], [ %i, %dispatch ]
  br label %body

case.default:
  br label %body

body:
  %i5 = phi i32 [ %i, %case.default ], [ %i2, %case3 ], [ %i3, %case2 ], [ %i4, %case1 ], [ %i, %case0 ]
  %i.next = add i32 %i5, 1
  br i1 %c, label %inner.si.unfold.true.jt1, label %sel.si.unfold.false.jt2

done:
  %i1 = phi i32 [ %i.jt1, %loop.jt1 ], [ %i.jt2, %loop.jt2 ], [ %i.jt3, %loop.jt3 ], [ %i, %loop ]
  ret i32 %i1
}
