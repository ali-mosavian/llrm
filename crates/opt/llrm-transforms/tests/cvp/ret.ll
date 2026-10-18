; runImpl: a returned value that is known is the constant, and the returns'
; union is the range of the function.
define i32 @f(i32 %x) {
entry:
  %c = icmp ult i32 %x, 10
  br i1 %c, label %then, label %else

then:
  %a = add i32 %x, 100
  ret i32 %a

else:
  ret i32 5
}
