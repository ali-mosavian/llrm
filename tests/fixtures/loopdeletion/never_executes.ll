@g = global i16 0

define i16 @f(i16 %x, i16 %n) {
b0:
  br i1 false, label %pre, label %out

pre:
  br label %loop

loop:
  %i = phi i16 [ 0, %pre ], [ %next, %loop ]
  store i16 %i, ptr @g
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, %n
  br i1 %go, label %loop, label %exit

exit:
  br label %out

out:
  %r = phi i16 [ %x, %b0 ], [ %x, %exit ]
  ret i16 %r
}
