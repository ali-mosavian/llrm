define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %o = phi i16 [ 0, %b0 ], [ %o1, %b5 ]
  %t = phi i16 [ %x, %b0 ], [ %t2, %b5 ]
  %og = icmp slt i16 %o, %n
  br i1 %og, label %b2, label %b6

b2:
  br label %b3

b3:
  %j = phi i16 [ 0, %b2 ], [ %j1, %b4 ]
  %u = phi i16 [ %t, %b2 ], [ %u1, %b4 ]
  %ig = icmp slt i16 %j, %o
  br i1 %ig, label %b4, label %b5

b4:
  %u1 = add i16 %u, %j
  %j1 = add i16 %j, 1
  br label %b3

b5:
  %w = phi i16 [ %u, %b3 ]
  %t2 = add i16 %w, 1
  %o1 = add i16 %o, 1
  br label %b1

b6:
  %r = phi i16 [ %t, %b1 ]
  ret i16 %r
}
