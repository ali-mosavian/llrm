define i32 @f(i32 %x, i32 %y) {
entry:
  %isz = icmp eq i32 %x, 0
  br i1 %isz, label %join, label %body

body:
  br label %join

join:
  %q = add i32 %x, %y
  ret i32 %q
}
