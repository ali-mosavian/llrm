; inferNoWrapFromUses: the uses are all under a check.
define i32 @f(i32 %x) {
entry:
  %y = add i32 %x, 1
  %c = icmp ult i32 %x, 100
  br i1 %c, label %then, label %else

then:
  ret i32 %y

else:
  ret i32 0
}
