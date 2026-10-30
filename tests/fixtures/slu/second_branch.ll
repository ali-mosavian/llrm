define i16 @f(i16 %n, i16 %m, i16 %k) {
b0:
  %z = icmp eq i16 %n, 0
  %y = icmp eq i16 %k, 0
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body2 ]
  br i1 %z, label %leave1, label %body

body:
  br i1 %y, label %leave2, label %body2

body2:
  %next = sub i16 %i, 1
  %done = icmp eq i16 %next, 0
  br i1 %done, label %leave3, label %head

leave1:
  br label %out

leave2:
  br label %out

leave3:
  br label %out

out:
  %r = phi i16 [ 1, %leave1 ], [ 2, %leave2 ], [ 3, %leave3 ]
  ret i16 %r
}
