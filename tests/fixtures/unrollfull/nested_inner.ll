define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %o = phi i16 [ 0, %b0 ], [ %o1, %b4 ]
  %t = phi i16 [ %x, %b0 ], [ %t2, %b4 ]
  br label %b2

b2:
  %j = phi i16 [ 0, %b1 ], [ %j1, %b2 ]
  %u = phi i16 [ %t, %b1 ], [ %u1, %b2 ]
  %u1 = add i16 %u, %n
  %j1 = add i16 %j, 1
  %more = icmp slt i16 %j1, 3
  br i1 %more, label %b2, label %b3

b3:
  %w = phi i16 [ %u1, %b2 ]
  br label %b4

b4:
  %t2 = add i16 %w, %o
  %o1 = add i16 %o, 1
  %again = icmp slt i16 %o1, %n
  br i1 %again, label %b1, label %b5

b5:
  %r = phi i16 [ %t2, %b4 ]
  ret i16 %r
}
