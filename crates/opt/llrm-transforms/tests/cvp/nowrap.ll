; processBinOp: flags the ranges prove.
define i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 255
  %b = and i32 %y, 255
  %c = add i32 %a, %b
  %d = mul i32 %a, %b
  %e = sub i32 %c, %a
  %g = shl i32 %a, 4
  %h = add i32 %c, %d
  %i = add i32 %h, %e
  %j = add i32 %i, %g
  ret i32 %j
}
