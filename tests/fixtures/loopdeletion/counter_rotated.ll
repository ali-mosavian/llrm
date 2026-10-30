define i16 @f(i16 %x) {
b0:
  br label %loop

loop:
  %i = phi i16 [ 0, %b0 ], [ %next, %loop ]
  %next = add i16 %i, 1
  %go = icmp slt i16 %next, 10
  br i1 %go, label %loop, label %out

out:
  ret i16 %x
}
