define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %head

head:
  %i = phi i16 [ %m, %b0 ], [ %next, %body ]
  %v = icmp eq i16 %i, 7
  %c = or i1 %z, %v
  br i1 %c, label %out, label %body

body:
  %next = sub i16 %i, 1
  %done = icmp eq i16 %next, 0
  br i1 %done, label %exit, label %head

exit:
  br label %end

out:
  br label %end

end:
  %r = phi i16 [ 5, %out ], [ 6, %exit ]
  ret i16 %r
}
