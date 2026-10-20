define i16 @f(i16 %n, i16 %m) {
b0:
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %latch ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %latch ]
  switch i16 %n, label %d [
    i16 1, label %one
    i16 2, label %two
  ]

one:
  %a = add i16 %acc, %i
  br label %latch

two:
  %b = mul i16 %acc, 3
  br label %latch

d:
  %c = sub i16 %acc, %i
  br label %latch

latch:
  %sum = phi i16 [ %a, %one ], [ %b, %two ], [ %c, %d ]
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %m
  br i1 %done, label %out, label %head

out:
  ret i16 %sum
}
