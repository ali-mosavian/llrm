define i16 @f(i16 %x) mustprogress {
b0:
  br label %outer

outer:
  %o = phi i16 [ 0, %b0 ], [ %o1, %olatch ]
  br label %inner

inner:
  %j = phi i16 [ 0, %outer ], [ %j1, %inner ]
  %j1 = add i16 %j, 1
  %ic = icmp slt i16 %j1, 4
  br i1 %ic, label %inner, label %olatch

olatch:
  %o1 = add i16 %o, 1
  %oc = icmp slt i16 %o1, 5
  br i1 %oc, label %outer, label %out

out:
  ret i16 %x
}
