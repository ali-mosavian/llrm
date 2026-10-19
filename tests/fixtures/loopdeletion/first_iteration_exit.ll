@g = global i16 0

define i16 @f(i16 %x) {
b0:
  br label %loop

loop:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  store i16 %x, ptr @g
  %c = icmp eq i16 %i, 0
  br i1 %c, label %out, label %latch

latch:
  %next = add i16 %i, 1
  br label %loop

out:
  ret i16 %x
}
