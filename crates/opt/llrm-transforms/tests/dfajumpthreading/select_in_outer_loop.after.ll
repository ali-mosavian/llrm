define i32 @f(i32 %n, i32 %m, i1 %c) {
entry:
  br label %outer

outer:
  %j = phi i32 [ 0, %entry ], [ %j.next, %outer.latch ]
  %acc = phi i32 [ 0, %entry ], [ %acc.out, %outer.latch ]
  br label %loop.jt0

sel.si.unfold.false:
  br label %loop

sel.si.unfold.false.jt2:
  %.si.unfold.phi.jt2 = phi i32 [ 2, %body ]
  br label %loop.jt2

loop:
  %state = phi i32 [ poison, %sel.si.unfold.false ]
  %i = phi i32 [ %i.next, %sel.si.unfold.false ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %inner.done

loop.jt2:
  %state.jt2 = phi i32 [ %.si.unfold.phi.jt2, %sel.si.unfold.false.jt2 ]
  %i.jt2 = phi i32 [ %i.next, %sel.si.unfold.false.jt2 ]
  %go.jt2 = icmp slt i32 %i.jt2, %n
  br i1 %go.jt2, label %dispatch.jt2, label %inner.done

loop.jt1:
  %state.jt1 = phi i32 [ 1, %body ]
  %i.jt1 = phi i32 [ %i.next, %body ]
  %go.jt1 = icmp slt i32 %i.jt1, %n
  br i1 %go.jt1, label %dispatch.jt1, label %inner.done

loop.jt0:
  %state.jt0 = phi i32 [ 0, %outer ]
  %i.jt0 = phi i32 [ 0, %outer ]
  %go.jt0 = icmp slt i32 %i.jt0, %n
  br i1 %go.jt0, label %dispatch.jt0, label %inner.done

dispatch:
  switch i32 undef, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
    i32 2, label %case2
  ]

dispatch.jt2:
  br label %case2

dispatch.jt1:
  br label %case1

dispatch.jt0:
  br label %case0

case0:
  br label %body

case1:
  br label %body

case2:
  br label %body

case.default:
  br label %body

body:
  %i2 = phi i32 [ undef, %case.default ], [ %i.jt2, %case2 ], [ %i.jt1, %case1 ], [ %i.jt0, %case0 ]
  %i.next = add i32 %i2, 1
  br i1 %c, label %loop.jt1, label %sel.si.unfold.false.jt2

inner.done:
  %i1 = phi i32 [ %i.jt2, %loop.jt2 ], [ %i.jt1, %loop.jt1 ], [ %i.jt0, %loop.jt0 ], [ %i, %loop ]
  br label %tail

tail:
  %k = phi i32 [ 0, %inner.done ], [ %k.next, %tail ]
  %k.next = add i32 %k, 1
  %kc = icmp slt i32 %k.next, %m
  br i1 %kc, label %tail, label %outer.latch

outer.latch:
  %j.next = add i32 %j, 1
  %acc.out = add i32 %acc, %i1
  %oc = icmp slt i32 %j.next, %m
  br i1 %oc, label %outer, label %done

done:
  ret i32 %acc.out
}
