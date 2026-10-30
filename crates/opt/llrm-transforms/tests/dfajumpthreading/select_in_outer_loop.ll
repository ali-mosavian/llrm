; The select-fed state machine is the inner of two nested loops, and a second
; loop follows it inside the outer one: the blocks made by the unfolding must
; be in the inner loop and the outer one, and the loop after them keeps its
; own.
define i32 @f(i32 %n, i32 %m, i1 %c) {
entry:
  br label %outer

outer:
  %j = phi i32 [ 0, %entry ], [ %j.next, %outer.latch ]
  %acc = phi i32 [ 0, %entry ], [ %acc.out, %outer.latch ]
  br label %loop

loop:
  %state = phi i32 [ 0, %outer ], [ %sel, %body ]
  %i = phi i32 [ 0, %outer ], [ %i.next, %body ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %inner.done

dispatch:
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
    i32 2, label %case2
  ]

case0:
  br label %body

case1:
  br label %body

case2:
  br label %body

case.default:
  br label %body

body:
  %sel = select i1 %c, i32 1, i32 2
  %i.next = add i32 %i, 1
  br label %loop

inner.done:
  br label %tail

tail:
  %k = phi i32 [ 0, %inner.done ], [ %k.next, %tail ]
  %k.next = add i32 %k, 1
  %kc = icmp slt i32 %k.next, %m
  br i1 %kc, label %tail, label %outer.latch

outer.latch:
  %j.next = add i32 %j, 1
  %acc.out = add i32 %acc, %i
  %oc = icmp slt i32 %j.next, %m
  br i1 %oc, label %outer, label %done

done:
  ret i32 %acc.out
}
