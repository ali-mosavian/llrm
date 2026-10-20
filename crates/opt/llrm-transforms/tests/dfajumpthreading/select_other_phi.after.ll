define i32 @f(i32 %n, i1 %c) {
entry:
  br label %loop

sel.si.unfold.false:
  br label %loop

sel.si.unfold.false.jt0:
  %.si.unfold.phi.jt0 = phi i32 [ 0, %body ]
  br label %loop.jt0

loop:
  %state = phi i32 [ 0, %entry ], [ poison, %sel.si.unfold.false ]
  %acc = phi i32 [ 0, %entry ], [ %acc.next, %sel.si.unfold.false ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %sel.si.unfold.false ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

loop.jt0:
  %state.jt0 = phi i32 [ %.si.unfold.phi.jt0, %sel.si.unfold.false.jt0 ]
  %acc.jt0 = phi i32 [ %acc.next, %sel.si.unfold.false.jt0 ]
  %i.jt0 = phi i32 [ %i.next, %sel.si.unfold.false.jt0 ]
  %go.jt0 = icmp slt i32 %i.jt0, %n
  br i1 %go.jt0, label %dispatch.jt0, label %done

loop.jt1:
  %state.jt1 = phi i32 [ 1, %body ]
  %acc.jt1 = phi i32 [ %acc.next, %body ]
  %i.jt1 = phi i32 [ %i.next, %body ]
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
  %i1 = phi i32 [ %i.jt0, %dispatch.jt0 ], [ %i, %dispatch ]
  %acc2 = phi i32 [ %acc.jt0, %dispatch.jt0 ], [ %acc, %dispatch ]
  %a0 = add i32 %acc2, 3
  br label %body

case1:
  %i2 = phi i32 [ %i.jt1, %dispatch.jt1 ], [ %i, %dispatch ]
  %acc3 = phi i32 [ %acc.jt1, %dispatch.jt1 ], [ %acc, %dispatch ]
  %a1 = mul i32 %acc3, 2
  br label %body

case.default:
  br label %body

body:
  %i3 = phi i32 [ %i, %case.default ], [ %i2, %case1 ], [ %i1, %case0 ]
  %acc.next = phi i32 [ %a0, %case0 ], [ %a1, %case1 ], [ %acc, %case.default ]
  %i.next = add i32 %i3, 1
  br i1 %c, label %loop.jt1, label %sel.si.unfold.false.jt0

done:
  %acc1 = phi i32 [ %acc.jt0, %loop.jt0 ], [ %acc.jt1, %loop.jt1 ], [ %acc, %loop ]
  ret i32 %acc1
}
