declare i16 @pred(i16)

define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %c = call i16 @pred(i16 %i)
  %go = icmp slt i16 %c, %n
  br i1 %go, label %b2, label %b3

b2:
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %i
}
