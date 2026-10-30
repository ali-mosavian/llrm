define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %lim = select i1 true, i16 %n, i16 %x
  %sh = shl i16 %i, 1
  %go = icmp ult i16 %sh, %lim
  br i1 %go, label %b2, label %b3

b2:
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %i
}
