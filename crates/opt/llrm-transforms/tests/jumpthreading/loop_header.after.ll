define i32 @f(i32 %n) {
entry:
  br label %head

head:
  %c = phi i1 [ true, %entry ], [ false, %body ]
  %i = phi i32 [ 0, %entry ], [ %next, %body ]
  br i1 %c, label %body, label %exit

body:
  %next = add i32 %i, 1
  call void @sink(i32 %next)
  br label %head

exit:
  ret i32 %i
}

declare void @sink(i32)
