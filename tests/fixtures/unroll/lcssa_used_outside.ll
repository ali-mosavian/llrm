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
  %s = mul i16 %acc, %n
  %next = add i16 %i, 1
  br label %b1

b3:
  %ra = phi i16 [ %acc, %b1 ]
  %ri = phi i16 [ %i, %b1 ]
  %c = icmp sgt i16 %ra, 100
  br i1 %c, label %b4, label %b5

b4:
  %big = add i16 %ra, %ri
  br label %b6

b5:
  %small = sub i16 %ra, %ri
  br label %b6

b6:
  %out = phi i16 [ %big, %b4 ], [ %small, %b5 ]
  %w = add i16 %out, %ri
  ret i16 %w
}
