define void @sub(i8 %x) {
  %a = sub nuw i8 10, %x
  %b = sub nsw i8 -10, %x
  %c = sub nsw i8 10, %x
  %d = sub nuw nsw i8 10, %x
  %e = sub nuw nsw i8 -2, %x
  %f = sub i8 10, %x
  ret void
}

define void @add(i8 %x) {
  %a = add nuw i8 %x, 10
  %b = add nsw i8 %x, -10
  %c = add nsw i8 %x, 10
  %d = add nuw nsw i8 %x, -2
  %e = add i8 %x, 10
  %f = add nuw i8 %x, 0
  ret void
}

define void @logic(i8 %x, i16 %y) {
  %a = and i8 %x, 12
  %n = sub i8 0, %x
  %b = and i8 %x, %n
  %c = and i8 %n, %x
  %d = or i8 %x, 12
  %e = and i16 %y, -1
  ret void
}

define void @shifts(i8 %x, i8 %n) {
  %a = ashr i8 %x, 3
  %b = ashr i8 -64, %n
  %c = ashr i8 64, %n
  %d = ashr exact i8 -64, %n
  %e = ashr exact i8 0, %n
  %f = ashr i8 %x, 9
  %g = lshr i8 %x, 3
  %h = lshr i8 200, %n
  %i = lshr exact i8 200, %n
  %j = lshr i8 %x, 8
  %k = shl nuw i8 3, %n
  %l = shl nsw i8 -3, %n
  %m = shl nsw i8 3, %n
  %o = shl i8 3, %n
  %p = shl i8 7, %n
  %q = shl i8 %x, 3
  %r = shl i8 %x, 8
  %s = shl nuw i8 0, %n
  %t = shl nsw i8 0, %n
  ret void
}

define void @divs(i8 %x, i8 %n) {
  %a = sdiv i8 %x, -1
  %b = sdiv i8 %x, 3
  %c = sdiv i8 %x, -3
  %d = sdiv i8 %x, 1
  %e = sdiv i8 %x, 0
  %f = sdiv i8 -128, %n
  %g = sdiv i8 100, %n
  %h = sdiv i8 -100, %n
  %i = udiv i8 %x, 7
  %j = udiv i8 %x, 0
  %k = udiv i8 200, %n
  %l = srem i8 %x, 7
  %m = srem i8 %x, -128
  %o = srem i8 -7, %n
  %p = srem i8 7, %n
  %q = urem i8 %x, 10
  %r = urem i8 %x, 0
  %s = urem i8 200, %n
  ret void
}

define void @wide(i64 %x, i128 %y, i1 %b) {
  %a = urem i64 %x, 1000
  %c = shl i128 %y, 100
  %d = and i1 %b, true
  %e = lshr i128 -1, 3
  ret void
}
