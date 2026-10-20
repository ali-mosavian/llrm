define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %latch ]
  br i1 %z, label %then, label %else

then:
  %a = add i16 %acc, %i
  br label %latch

else:
  %b = mul i16 %acc, 3
  br label %latch

latch:
  %sum = phi i16 [ %a, %then ], [ %b, %else ]
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %m
  br i1 %done, label %out, label %head

out:
  ret i16 %sum
}
