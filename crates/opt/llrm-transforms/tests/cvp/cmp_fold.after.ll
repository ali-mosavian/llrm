define i1 @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %then, label %else

then:
  ret i1 true

else:
  ret i1 false
}
