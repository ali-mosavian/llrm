define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %body ]
  br i1 %z, label %out, label %test

test:
  %done = icmp eq i16 %i, 0
  br i1 %done, label %out, label %body

body:
  %next = sub i16 %i, 1
  %sum = add i16 %acc, %i
  br label %head

out:
  %r = phi i16 [ %acc, %head ], [ %acc, %test ]
  ret i16 %r
}
