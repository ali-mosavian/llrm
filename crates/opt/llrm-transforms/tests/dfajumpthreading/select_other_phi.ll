; The select is unfolded with another phi in the join: the new block gives
; that phi the value it has from the start block.
define i32 @f(i32 %n, i1 %c) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %sel, %body ]
  %acc = phi i32 [ 0, %entry ], [ %acc.next, %body ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %body ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

dispatch:
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
  ]

case0:
  %a0 = add i32 %acc, 3
  br label %body

case1:
  %a1 = mul i32 %acc, 2
  br label %body

case.default:
  br label %body

body:
  %acc.next = phi i32 [ %a0, %case0 ], [ %a1, %case1 ], [ %acc, %case.default ]
  %sel = select i1 %c, i32 1, i32 0
  %i.next = add i32 %i, 1
  br label %loop

done:
  ret i32 %acc
}
