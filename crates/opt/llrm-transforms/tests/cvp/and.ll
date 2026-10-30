; processAnd: a mask that keeps every bit of the operand is no mask.
define i32 @f(i32 %x) {
entry:
  %a = and i32 %x, 15
  %b = and i32 %a, 255
  ret i32 %b
}
