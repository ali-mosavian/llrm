define i32 @f(i32 %n, ptr %p) {
exit:
  ret i32 0

l1:
  %c = add i32 %c, 1
  store i32 %c, ptr %p
  %t = icmp eq i32 %c, %n
  br label %l1
}
