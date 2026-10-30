define i16 @f(i16 %x, i16 %n, i1 %c) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b4 ]
  %acc = phi i16 [ %x, %b0 ], [ %merged, %b4 ]
  %go = icmp ult i16 %i, %n
  br i1 %go, label %b2, label %b5

b2:
  br i1 %c, label %b3, label %b4

b3:
  %t = add i16 %acc, %i
  br label %b4

b4:
  %merged = phi i16 [ %acc, %b2 ], [ %t, %b3 ]
  %next = add i16 %i, 1
  br label %b1

b5:
  %r = phi i16 [ %acc, %b1 ]
  ret i16 %r
}
