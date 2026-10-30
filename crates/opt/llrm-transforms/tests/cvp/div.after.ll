define range(i32 0, 4402) i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 7
  %b = and i32 %y, 3
  %p = or i32 %b, 8
  %r = urem i32 %y, 100
  %s1.cmp = icmp uge i32 %a, 4
  %s1.udiv = zext i1 %s1.cmp to i32
  %u = udiv i32 %x, 1000000
  %w = add nuw nsw i32 0, %r
  %z = add nuw nsw i32 %w, %s1.udiv
  %e = add nuw nsw i32 %z, %a
  %g = add nuw nsw i32 %e, %u
  ret i32 %g
}
