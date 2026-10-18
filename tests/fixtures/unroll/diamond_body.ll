; unroll b1 count 6 exits b1=6
define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b5 ]
  %acc = phi i16 [ %x, %b0 ], [ %t, %b5 ]
  %go = icmp slt i16 %i, 5
  br i1 %go, label %b2, label %b6

b2:
  %odd = and i16 %i, 1
  %c = icmp eq i16 %odd, 0
  br i1 %c, label %b3, label %b4

b3:
  %a = add i16 %acc, %n
  br label %b5

b4:
  %d = mul i16 %acc, 3
  br label %b5

b5:
  %t = phi i16 [ %a, %b3 ], [ %d, %b4 ]
  %next = add i16 %i, 1
  br label %b1

b6:
  %r = phi i16 [ %acc, %b1 ]
  ret i16 %r
}
