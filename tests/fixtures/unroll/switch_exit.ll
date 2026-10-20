; unroll b1 count 4 exits b1=4
define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b3 ]
  %go = icmp slt i16 %i, 3
  br i1 %go, label %b2, label %b4

b2:
  %s = add i16 %acc, %n
  %sel = and i16 %s, 3
  switch i16 %sel, label %b3 [
    i16 3, label %b4
  ]

b3:
  %next = add i16 %i, 1
  br label %b1

b4:
  %r = phi i16 [ %acc, %b1 ], [ %s, %b2 ]
  ret i16 %r
}
