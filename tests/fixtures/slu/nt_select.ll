define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ 0, %b0 ], [ %next, %head ]
  %acc = phi i16 [ 0, %b0 ], [ %sum, %head ]
  %a = add i16 %acc, %i
  %b = mul i16 %acc, 3
  %sum = select i1 %z, i16 %a, i16 %b
  %next = add i16 %i, 1
  %done = icmp eq i16 %next, %m
  br i1 %done, label %out, label %head

out:
  ret i16 %sum
}
