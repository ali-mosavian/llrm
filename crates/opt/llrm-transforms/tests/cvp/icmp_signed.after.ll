define i1 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 255
  %b = and i32 %y, 127
  %c = icmp samesign ult i32 %a, %b
  ret i1 %c
}
