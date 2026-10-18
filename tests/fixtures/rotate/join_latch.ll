define i16 @f(i16 %a, i16 %n, ptr %p, i1 %d) {
b0:
  br label %b1

b1:
  %base = phi i16 [ %a, %b0 ], [ %base2, %b8 ]
  %i = phi i16 [ 16, %b0 ], [ %next, %b8 ]
  %go = icmp sle i16 %i, 303
  br i1 %go, label %b2, label %b9

b2:
  %x = load i16, ptr %p
  %c = icmp sgt i16 %x, %n
  br i1 %c, label %b8, label %b3

b3:
  br i1 %d, label %b4, label %b5

b4:
  %y = add i16 %i, 640
  br label %b6

b5:
  %z = load i16, ptr %p
  br label %b6

b6:
  %m = phi i16 [ %y, %b4 ], [ %z, %b5 ]
  br label %b8

b8:
  %v = phi i16 [ 0, %b2 ], [ %m, %b6 ]
  store i16 %v, ptr %p
  %next = add i16 %i, 1
  %base2 = add i16 %base, 2
  br label %b1

b9:
  ret i16 %i
}
