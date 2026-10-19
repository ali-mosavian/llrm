define i16 @f(i16 %n, i16 %m) {
b0:
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  br i1 true, label %body, label %out

body:
  %next = sub i16 %i, 1
  %done = icmp eq i16 %next, 0
  br i1 %done, label %exit, label %head

out:
  ret i16 1

exit:
  ret i16 2
}
