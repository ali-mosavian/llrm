define i16 @f(i16 %x) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b1 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b1 ]
  %m = mul i16 %acc, 3
  %s = add i16 %m, %i
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 200
  br i1 %go, label %b1, label %b2

b2:
  %r = phi i16 [ %s, %b1 ]
  ret i16 %r
}
