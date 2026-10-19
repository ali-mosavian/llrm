define i16 @f(i16 %n, i16 %m) {
b0:
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  %c = icmp eq i16 %i, 7
  br i1 %c, label %out, label %body

body:
  %next = sub i16 %i, 1
  %done = icmp eq i16 %next, 0
  br i1 %done, label %out, label %head

out:
  %r = phi i16 [ %i, %head ], [ %next, %body ]
  ret i16 %r
}
