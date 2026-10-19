define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  br i1 %z, label %out, label %body

body:
  %next = sub i16 %i, 1
  %done = icmp eq i16 %next, 0
  br i1 %done, label %out, label %head

out:
  %r = phi i16 [ %m, %head ], [ %next, %body ]
  ret i16 %r
}
