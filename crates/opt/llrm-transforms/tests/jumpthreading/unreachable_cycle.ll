define i32 @f(i32 %n, ptr %p) {
entry:
  br i1 true, label %exit, label %l1
l1:
  %a = phi i32 [ 0, %entry ], [ %c, %l2 ]
  %t = icmp eq i32 %a, %n
  br i1 %t, label %l2, label %exit
l2:
  %c = add i32 %a, 1
  store i32 %c, ptr %p
  br label %l1
exit:
  ret i32 0
}
