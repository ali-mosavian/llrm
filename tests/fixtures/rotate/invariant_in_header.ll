define i16 @f(i16 %x, i16 %n, i16 %k) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b2 ]
  %inv = mul i16 %k, 7
  %c = add i16 %inv, %i
  %go = icmp slt i16 %c, %n
  br i1 %go, label %b2, label %b3

b2:
  %s = add i16 %acc, %c
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = phi i16 [ %acc, %b1 ]
  ret i16 %r
}
