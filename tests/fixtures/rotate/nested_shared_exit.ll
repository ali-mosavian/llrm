define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %o = phi i16 [ 0, %b0 ], [ %o1, %b4 ]
  %og = icmp slt i16 %o, %n
  br i1 %og, label %b2, label %b5

b2:
  %j = phi i16 [ 0, %b1 ], [ %j1, %b3 ]
  %ig = icmp slt i16 %j, %o
  br i1 %ig, label %b3, label %b4

b3:
  %j1 = add i16 %j, 1
  %stop = icmp eq i16 %j1, %x
  br i1 %stop, label %b5, label %b2

b4:
  %o1 = add i16 %o, 1
  br label %b1

b5:
  %r = phi i16 [ %o, %b1 ], [ %j1, %b3 ]
  ret i16 %r
}
