define void @f(i16 %b, i16 %a, i16 %x) {
entry:
  br label %h
h:
  %iv = phi i16 [ %b, %entry ], [ %iv.next, %h ]
  %iv.next = add i16 %iv, %a
  %c = icmp ult i16 %iv.next, %x
  br i1 %c, label %h, label %exit
exit:
  ret void
}
