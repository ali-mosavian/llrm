define i32 @f(i32 %x, i32 %a, i32 %b) {
entry:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %then, label %else

then:
  %u = add i32 %a, 1
  ret i32 %u

else:
  %v = add i32 %b, 2
  ret i32 %v
}
