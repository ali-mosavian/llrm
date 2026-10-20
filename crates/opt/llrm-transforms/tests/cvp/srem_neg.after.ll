define range(i32 -159, 1) i32 @f(i32 %x, i32 %y) {
entry:
  %a = or i32 %x, -128
  %b = and i32 %y, 31
  %c = add nuw nsw i32 %b, 1
  %a.nonneg = sub i32 0, %a
  %r1.lhs.trunc = trunc i32 %a.nonneg to i8
  %r1.rhs.trunc = trunc i32 %c to i8
  %r11 = urem i8 %r1.lhs.trunc, %r1.rhs.trunc
  %r1.zext = zext i8 %r11 to i32
  %r1.neg = sub i32 0, %r1.zext
  %a.nonneg1 = sub i32 0, %a
  %d1.lhs.trunc = trunc i32 %a.nonneg1 to i8
  %d1.rhs.trunc = trunc i32 %c to i8
  %d11 = udiv i8 %d1.lhs.trunc, %d1.rhs.trunc
  %d1.zext = zext i8 %d11 to i32
  %d1.neg = sub i32 0, %d1.zext
  %s = add nsw i32 %r1.neg, %d1.neg
  ret i32 %s
}
