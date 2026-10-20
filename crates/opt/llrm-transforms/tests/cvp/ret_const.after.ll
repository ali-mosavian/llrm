define range(i32 0, 5) i32 @f(i32 %x) {
entry:
  %c = icmp eq i32 %x, 4
  br i1 %c, label %then, label %else

then:
  ret i32 4

else:
  ret i32 0
}
