define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 10, %b0 ], [ %next, %b2 ]
  %go = icmp slt i16 %i, 5
  br i1 %go, label %b2, label %b3

b2:
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = phi i16 [ %i, %b1 ]
  ret i16 %r
}
