define i16 @f(i16 %x, i16 %n) {
b0:
  br label %b1

b1:
  %i = phi i16 [ 0, %b0 ], [ %next, %b2 ]
  %go = icmp slt i16 %i, %n
  br i1 %go, label %b2, label %b3, !prof !0

b2:
  %next = add i16 %i, 1
  br label %b1

b3:
  ret i16 %i
}

!0 = !{!"branch_weights", i32 10, i32 100}
