; runImpl: a value the branch fixes is returned as the constant.
define i32 @f(i32 %x) {
entry:
  %c = icmp eq i32 %x, 4
  br i1 %c, label %then, label %else

then:
  ret i32 %x

else:
  ret i32 0
}
