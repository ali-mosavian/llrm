; A select whose false value is another select: the second is put on the
; work list when the first is unfolded, and is unfolded from the block the
; first one made.
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
  %inner = select i1 %d, i32 3, i32 2
  %sel = select i1 %c, i32 1, i32 %inner
  %i.next = add i32 %i, 1
  br label %loop

done:
  ret i32 %i
}
