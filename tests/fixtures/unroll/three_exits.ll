; unroll b1 count 5 exits b1=5
define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b3 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b3 ]
  %go = icmp slt i16 %i, 4
  br i1 %go, label %b2, label %b5

b2:
  %s = add i16 %acc, %n
  %neg = icmp slt i16 %s, 0
  br i1 %neg, label %b5, label %b3

b3:
  %next = add i16 %i, 1
  %odd = icmp eq i16 %s, 77
  br i1 %odd, label %b5, label %b1

b5:
  %r = phi i16 [ %acc, %b1 ], [ %s, %b2 ], [ %s, %b3 ]
  ret i16 %r
}
