; unroll b1 count 4 exits b2=4
define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %acc = phi i16 [ %x, %b0 ], [ %s, %b2 ]
  %m = mul i16 %acc, %n
  %s = add i16 %m, %i
  br label %b2

b2:
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 4
  br i1 %go, label %b1, label %b3

b3:
  %r = phi i16 [ %s, %b2 ]
  %k = phi i16 [ %next, %b2 ]
  %u = add i16 %k, %r
  ret i16 %u
}
