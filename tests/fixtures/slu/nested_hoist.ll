define i16 @f(i16 %n, i16 %m) {
b0:
  %z = icmp eq i16 %n, 0
  br label %outer

outer:
  %o = phi i16 [ 0, %b0 ], [ %o1, %olatch ]
  br label %inner

inner:
  %i = phi i16 [ 0, %outer ], [ %i1, %ibody ]
  br i1 %z, label %leave, label %ibody

ibody:
  %i1 = add i16 %i, 1
  %ic = icmp slt i16 %i1, 4
  br i1 %ic, label %inner, label %olatch

leave:
  br label %done

olatch:
  %o1 = add i16 %o, 1
  %oc = icmp slt i16 %o1, 5
  br i1 %oc, label %outer, label %done2

done:
  br label %end

done2:
  br label %end

end:
  %r = phi i16 [ 1, %done ], [ 2, %done2 ]
  ret i16 %r
}
