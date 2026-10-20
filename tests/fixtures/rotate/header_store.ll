define i16 @f(i16 %x, i16 %n, ptr %p) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  store i16 %i, ptr %p
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %q = load i16, ptr %p
  %next = add i16 %q, 1
  br label %b1

b3:
  ret i16 %i
}
