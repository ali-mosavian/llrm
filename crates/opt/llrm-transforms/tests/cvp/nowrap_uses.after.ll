define range(i32 0, 101) i32 @f(i32 %x) {
entry:
  %y = add nuw nsw i32 %x, 1
  %c = icmp ult i32 %x, 100
  br i1 %c, label %then, label %else

then:
  ret i32 %y

else:
  ret i32 0
}
