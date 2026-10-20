; processUDivOrURem and processSDivOrSRem: expanded, narrowed or made unsigned.
define i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 7
  %b = and i32 %y, 3
  %p = or i32 %b, 8
  %d = udiv i32 %a, %p
  %r = urem i32 %y, 100
  %s = sdiv i32 %a, 4
  %t = srem i32 %a, %p
  %u = udiv i32 %x, 1000000
  %w = add i32 %d, %r
  %z = add i32 %w, %s
  %e = add i32 %z, %t
  %g = add i32 %e, %u
  ret i32 %g
}
