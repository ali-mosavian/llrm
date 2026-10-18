; processTrunc.
define i8 @f(i32 %x) {
entry:
  %a = and i32 %x, 127
  %b = trunc i32 %a to i8
  ret i8 %b
}
