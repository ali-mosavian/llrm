define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %a = phi i16 [ %x, %b0 ], [ %b, %b2 ]
  %b = phi i16 [ 1, %b0 ], [ %a, %b2 ]
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = sub i16 %a, %b
  ret i16 %r
}
