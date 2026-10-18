; processSelect: the condition is known where the select is used.
define i32 @f(i32 %x, i32 %a, i32 %b) {
entry:
  %c = icmp ult i32 %x, 10
  %s = select i1 %c, i32 %a, i32 %b
  br i1 %c, label %then, label %else

then:
  %u = add i32 %s, 1
  ret i32 %u

else:
  %v = add i32 %s, 2
  ret i32 %v
}
