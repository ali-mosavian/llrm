; The state is a select of two constants whose only use is the state phi,
; taken from the select's own block: the pin unfolds it into control flow and
; threads the paths.
define i32 @f(i32 %n, i1 %c) {
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

done:
  ret i32 %i
}
