define i16 @f(i16 %x, i16 %n, ptr %p) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %v1 = load i16, ptr %p
  %a1 = add i16 %v1, %i
  %a2 = mul i16 %a1, 3
  %a3 = add i16 %a2, %x
  %a4 = mul i16 %a3, 5
  %a5 = add i16 %a4, %i
  %a6 = mul i16 %a5, 7
  %a7 = add i16 %a6, %x
  %a8 = mul i16 %a7, 9
  %a9 = add i16 %a8, %i
  %b1v = mul i16 %a9, 11
  %b2v = add i16 %b1v, %x
  %b3v = mul i16 %b2v, 13
  %b4v = add i16 %b3v, %i
  %b5v = mul i16 %b4v, 15
  %b6v = add i16 %b5v, %x
  %b7v = mul i16 %b6v, 17
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3

b2:
  %next = add i16 %i, 1
  store i16 %b7v, ptr %p
  br label %b1

b3:
  ret i16 %i
}
