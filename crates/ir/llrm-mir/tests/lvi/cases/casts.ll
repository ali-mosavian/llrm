define i32 @ext(i8 %x) {
entry:
  %c = icmp ult i8 %x, 100
  br i1 %c, label %ok, label %out
ok:
  %z = zext i8 %x to i32
  %s = sext i8 %x to i32
  %r = add i32 %z, %s
  ret i32 %r
out:
  %s2 = sext i8 %x to i32
  ret i32 %s2
}

define i8 @trunc_range(i32 %x) {
entry:
  %c = icmp ult i32 %x, 100
  br i1 %c, label %ok, label %out
ok:
  %t = trunc i32 %x to i8
  %u = trunc nuw i32 %x to i8
  %v = trunc nsw i32 %x to i8
  %r = add i8 %t, %u
  %s = add i8 %r, %v
  ret i8 %s
out:
  ret i8 0
}

define i32 @ptrint(ptr %p) {
entry:
  %i = ptrtoint ptr %p to i64
  %t = trunc i64 %i to i32
  ret i32 %t
}

define i32 @shifts(i32 %x) {
entry:
  %c = icmp ult i32 %x, 1000
  br i1 %c, label %ok, label %out
ok:
  %a = lshr i32 %x, 3
  %b = shl nuw i32 %x, 2
  %d = ashr i32 %x, 1
  %e = and i32 %x, 12
  %f = or i32 %x, 1
  %g = xor i32 %x, 255
  %h = urem i32 %x, 10
  %i = sdiv i32 %x, 7
  %s = add i32 %a, %b
  ret i32 %s
out:
  ret i32 0
}

define i32 @flags(i32 %x, i32 %y) {
entry:
  %a = icmp ult i32 %x, 100
  %b = icmp ult i32 %y, 100
  %c = and i1 %a, %b
  br i1 %c, label %ok, label %out
ok:
  %m = mul nuw i32 %x, %y
  %n = mul nsw i32 %x, %y
  %p = add nuw nsw i32 %x, %y
  %q = sub nuw i32 %x, %y
  %d = or disjoint i32 %x, %y
  %f = freeze i32 %x
  ret i32 %m
out:
  ret i32 0
}
