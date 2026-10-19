define range(i32 5, 110) i32 @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %then, label %else

then:
  %a = add nuw nsw i32 %x, 100
  ret i32 %a

else:
  ret i32 5
}
