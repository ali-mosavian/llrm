define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %next = add i16 %i, 1
  %more = icmp slt i16 %next, 100
  br i1 %more, label %b1, label %b4

b3:
  %r = phi i16 [ %i, %b1 ]
  ret i16 %r

b4:
  %q = phi i16 [ %next, %b2 ]
  ret i16 %q
}
