; A select whose true value is another select: the inner select is unfolded
; from the block the outer one made, its phi taking the outer one's place.
define i32 @f(i32 %n, i1 %c, i1 %d) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %sel, %body ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %body ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

dispatch:
  switch i32 %state, label %case.default [
    i32 0, label %case0
    i32 1, label %case1
    i32 2, label %case2
    i32 3, label %case3
  ]

case0:
  br label %body

case1:
  br label %body

case2:
  br label %body

case3:
  br label %body

case.default:
  br label %body

body:
  %inner = select i1 %d, i32 1, i32 3
  %sel = select i1 %c, i32 %inner, i32 2
  %i.next = add i32 %i, 1
  br label %loop

done:
  ret i32 %i
}
