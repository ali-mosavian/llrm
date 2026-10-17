define void @adds(i8 %x, i8 %y, i16 %w) {
  %a = add i8 %x, %y
  %zx = zext i8 %x to i16
  %zy = zext i8 %y to i16
  %b = add i16 %zx, %zy
  %sx = sext i8 %x to i16
  %sy = sext i8 %y to i16
  %c = add i16 %sx, %sy
  %mx = and i8 %x, 63
  %my = and i8 %y, 63
  %d = add i8 %mx, %my
  %e = add nsw i8 %x, %y
  %f = add i8 %mx, 100
  %g = add i8 %x, 200
  %h = or i8 %x, 128
  %i = add i8 %h, %h
  %j = add i8 %mx, 5
  ret void
}

define void @subs(i8 noundef %x, i8 noundef %y) {
  %a = sub i8 %x, %y
  %u = urem i8 %x, %y
  %b = sub i8 %x, %u
  %c = sub nuw i8 %x, %y
  %d = sub i8 %x, %c
  %s = srem i8 %x, %y
  %e = sub i8 %x, %s
  %n = sub nsw i8 %x, %y
  %f = sub i8 %x, %n
  %mx = and i8 %x, 15
  %my = or i8 %y, 16
  %g = sub i8 %mx, %my
  %h = sub i8 %my, %mx
  %sx = sext i8 %x to i16
  %sy = sext i8 %y to i16
  %i = sub i16 %sx, %sy
  ret void
}

define void @muls(i8 %x, i8 %y) {
  %a = mul i8 %x, %y
  %mx = and i8 %x, 15
  %my = and i8 %y, 15
  %b = mul i8 %mx, %my
  %c = mul nsw i8 %mx, %my
  %zx = zext i8 %x to i16
  %zy = zext i8 %y to i16
  %d = mul i16 %zx, %zy
  %sx = sext i8 %x to i16
  %sy = sext i8 %y to i16
  %e = mul i16 %sx, %sy
  %hx = or i8 %x, 192
  %hy = or i8 %y, 192
  %f = mul i8 %hx, %hy
  %g = mul i8 %hx, %mx
  %h = mul i8 %mx, 20
  %i = mul nsw i8 %x, 3
  ret void
}

define i8 @dominated(i8 %x, i8 %y) {
  %c = icmp uge i8 %x, %y
  br i1 %c, label %yes, label %no
yes:
  %a = sub i8 %x, %y
  ret i8 %a
no:
  %b = sub i8 %x, %y
  ret i8 %b
}
