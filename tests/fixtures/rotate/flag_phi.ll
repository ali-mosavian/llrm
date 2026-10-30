define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %flag = phi i1 [ true, %b0 ], [ %again, %b2 ]
  br i1 %flag, label %b2, label %b3

b2:
  %next = add i16 %i, %x
  %again = icmp slt i16 %next, %n
  br label %b1

b3:
  %r = phi i16 [ %i, %b1 ]
  ret i16 %r
}
