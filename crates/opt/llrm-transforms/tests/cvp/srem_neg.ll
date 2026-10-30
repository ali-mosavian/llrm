; processSRem and processSDiv: both operands' signs are known.
define i32 @f(i32 %x, i32 %y) {
entry:
  %a = or i32 %x, -128
  %b = and i32 %y, 31
  %c = add i32 %b, 1
  %r = srem i32 %a, %c
  %d = sdiv i32 %a, %c
  %s = add i32 %r, %d
  ret i32 %s
}
