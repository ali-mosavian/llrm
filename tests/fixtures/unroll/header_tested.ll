; unroll b1 count 4 exits b1=4
define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b2 ]
  %go = icmp slt i16 %i, 3
  br i1 %go, label %b2, label %b3

b2:
  %m = mul i16 %acc, %n
  %s = add i16 %m, %i
  %next = add i16 %i, 1
  br label %b1

b3:
  %r = phi i16 [ %acc, %b1 ]
  %k = phi i16 [ %i, %b1 ]
  %u = add i16 %k, %r
  ret i16 %u
}
