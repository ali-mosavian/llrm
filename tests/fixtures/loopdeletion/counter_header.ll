define i16 @f(i16 %x) mustprogress {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %body ]
  %go = icmp slt i16 %i, 10
  br i1 %go, label %body, label %out

body:
  %next = add i16 %i, 1
  br label %head

out:
  ret i16 %x
}
