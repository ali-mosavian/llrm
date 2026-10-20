define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b3 ]
  %go = icmp slt i16 %i, 6
  br i1 %go, label %b2, label %b4

b2:
  %s = add i16 %acc, %n
  %big = icmp sgt i16 %s, 12
  br i1 %big, label %b4, label %b3

b3:
  %next = add i16 %i, 1
  br label %b1

b4:
  %r = phi i16 [ %acc, %b1 ], [ %s, %b2 ]
  ret i16 %r
}
