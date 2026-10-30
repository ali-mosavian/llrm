define i16 @f(i16 %x, i16 %n) mustprogress {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %body ]
  %go = icmp slt i16 %i, 10
  br i1 %go, label %body, label %out

body:
  %stop = icmp eq i16 %i, %n
  %next = add i16 %i, 1
  br i1 %stop, label %out, label %head

out:
  ret i16 %x
}
