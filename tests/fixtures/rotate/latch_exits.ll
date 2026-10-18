define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %dead = phi i16 [ 5, %b0 ], [ %d2, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %d2 = mul i16 %dead, 3
  %next = add i16 %i, 1
  %more = icmp slt i16 %next, 100
  br i1 %more, label %b1, label %b4

b3:
  %r = phi i16 [ %dead, %b1 ]
  ret i16 %r

b4:
  ret i16 0
}
