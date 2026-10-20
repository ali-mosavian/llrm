define i16 @f(i16 %n, i16 %m, i16 %k) {
b0:
  %z = icmp eq i16 %n, 0
  br label %outer

outer:
  %j = phi i16 [ 0, %b0 ], [ %jn, %olatch ]
  %acc = phi i16 [ 0, %b0 ], [ %r, %olatch ]
  br label %head

head:
  %i = phi i16 [ 0, %outer ], [ %next, %latch ]
  %x = phi i16 [ %acc, %outer ], [ %sum, %latch ]
  br i1 %z, label %then, label %else

then:
  %a = add i16 %x, %i
  br label %latch

else:
  %b = mul i16 %x, 3
  br label %latch

latch:
  %sum = phi i16 [ %a, %then ], [ %b, %else ]
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %m
  br i1 %done, label %olatch, label %head

olatch:
  %r = phi i16 [ %sum, %latch ]
  %jn = add i16 %j, 1
  %odone = icmp eq i16 %jn, %k
  br i1 %odone, label %out, label %outer

out:
  ret i16 %r
}
