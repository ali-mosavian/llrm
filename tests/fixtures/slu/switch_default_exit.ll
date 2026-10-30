define i16 @f(i16 %n, i16 %m) {
b0:
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  switch i16 %n, label %leave [
    i16 1, label %body
    i16 2, label %body
  ]

body:
  %next = sub i16 %i, 1
  %done = icmp eq i16 %next, 0
  br i1 %done, label %exit, label %head

leave:
  br label %out

exit:
  br label %out

out:
  %r = phi i16 [ 0, %leave ], [ 1, %exit ]
  ret i16 %r
}
