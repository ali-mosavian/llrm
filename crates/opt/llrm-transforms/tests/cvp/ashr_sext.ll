; processAShr and processSExt: a non-negative operand needs neither sign.
define i32 @f(i16 %x, i32 %y) {
entry:
  %a = and i32 %y, 1023
  %b = ashr i32 %a, 3
  %c = and i16 %x, 100
  %d = sext i16 %c to i32
  %e = add i32 %b, %d
  ret i32 %e
}
