define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b3 ]
  %s = add i16 %acc, %i
  %go = icmp slt i16 %s, %n
  br i1 %go, label %b3, label %b2

b3:
  %next = add i16 %i, 1
  br label %b1

b2:
  %r = phi i16 [ %s, %b1 ]
  ret i16 %r
}
