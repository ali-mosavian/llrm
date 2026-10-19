define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  br i1 %z, label %latch, label %body

body:
  br label %latch

latch:
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 10
  br i1 %go, label %head, label %out

out:
  ret i16 %m
}
