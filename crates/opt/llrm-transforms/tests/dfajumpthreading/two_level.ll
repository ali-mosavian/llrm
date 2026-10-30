; The state is set two blocks above the loop's header: the paths are joined
; from the determinator through an intermediate block.
define i32 @f(i32 %n, i1 %c) {
entry:
  br label %loop

loop:
  %state = phi i32 [ 0, %entry ], [ %next, %mid ]
  %i = phi i32 [ 0, %entry ], [ %i.next, %mid ]
  %go = icmp slt i32 %i, %n
  br i1 %go, label %dispatch, label %done

dispatch:
  switch i32 %state, label %body.default [
    i32 0, label %body0
    i32 1, label %body1
  ]

body0:
  br i1 %c, label %then0, label %else0

then0:
  br label %merge

else0:
  br label %merge

body1:
  br label %merge1

body.default:
  br label %merge1

merge:
  %m = phi i32 [ 1, %then0 ], [ 0, %else0 ]
  br label %mid

merge1:
  br label %mid1

mid1:
  br label %mid

mid:
  %next = phi i32 [ %m, %merge ], [ 0, %mid1 ]
  %i.next = add i32 %i, 1
  br label %loop

done:
  ret i32 %i
}
