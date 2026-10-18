declare i32 @llvm.ctlz.i32(i32, i1)
declare i8 @llvm.smin.i8(i8, i8)

define i32 @and_neg(i32 %x) {
  %o = or i32 %x, 8
  %n = sub i32 0, %o
  %a = and i32 %o, %n
  ret i32 %a
}

define i32 @and_neg_commuted(i32 %x) {
  %o = or i32 %x, 8
  %n = sub i32 0, %o
  %a = and i32 %n, %o
  ret i32 %a
}

define i32 @xor_dec(i32 %x) {
  %o = or i32 %x, 4
  %d = add i32 %o, -1
  %a = xor i32 %o, %d
  ret i32 %a
}

define i32 @or_odd_step(i32 %x, i32 %y) {
  %o = or i32 %y, 1
  %s = add i32 %x, %o
  %a = and i32 %x, %s
  %b = or i32 %x, %s
  %c = xor i32 %s, %x
  ret i32 %a
}

define i32 @sub_forms(i32 %x, i32 %y) {
  %o = or i32 %y, 1
  %s1 = sub i32 %x, %o
  %a1 = and i32 %s1, %x
  %s2 = sub i32 %o, %x
  %a2 = or i32 %x, %s2
  ret i32 %a1
}

define i32 @mul_self(i32 %x) {
  %o = or i32 %x, 4
  %m = mul i32 %o, %o
  %s = sext i16 7 to i32
  ret i32 %m
}

define i32 @mul_nsw(i32 %x, i32 %y) {
  %a = and i32 %x, 255
  %b = and i32 %y, 255
  %m = mul nsw i32 %a, %b
  %n1 = or i32 %x, -2147483648
  %n2 = or i32 %y, -2147483648
  %m2 = mul nsw i32 %n1, %n2
  %m3 = mul nuw nsw i32 %n1, %b
  %z = or i32 %b, 2
  %m4 = mul nuw nsw i32 %x, %z
  ret i32 %m
}

define i32 @mul_neg_nonneg(i32 %x, i32 %y) {
  %n = or i32 %x, -2147483648
  %p = and i32 %y, 255
  %q = or i32 %p, 1
  %m = mul nsw i32 %n, %q
  %m2 = mul nsw i32 %q, %n
  ret i32 %m
}

define i32 @divs(i32 %x, i32 %y) {
  %a = and i32 %x, 1023
  %b = or i32 %y, 8
  %d = udiv i32 %a, %b
  %e = udiv exact i32 %x, 16
  %f = sdiv i32 %a, 4
  %g = sdiv exact i32 %x, 8
  %h = urem i32 %x, 64
  %i = srem i32 %x, 64
  %j = urem i32 %a, %b
  ret i32 %d
}

define i32 @shifts(i32 %x, i32 %y, i32 %z) {
  %amt = or i32 %y, 1
  %amt2 = and i32 %amt, 15
  %a = shl i32 %x, %amt2
  %b = lshr i32 %x, %amt2
  %c = ashr i32 %x, %amt2
  %d = shl nuw i32 %x, %amt2
  %e = shl nsw i32 %x, 3
  %f = lshr exact i32 %x, 3
  %g = ashr exact i32 %x, 3
  %nz = or i32 %z, 4
  %m = and i32 %nz, 7
  %h = shl i32 %x, %m
  ret i32 %a
}

define i32 @shift_const_lhs(i32 %y) {
  %a = shl i32 24, %y
  %b = lshr i32 24, %y
  %c = ashr i32 -24, %y
  %m = and i32 %y, 3
  %d = shl i32 24, %m
  ret i32 %a
}

define i32 @shl_ctlz(i32 %x) {
  %c = call i32 @llvm.ctlz.i32(i32 %x, i1 true)
  %xr = xor i32 %c, 31
  %s = sub i32 20, %xr
  %r = shl i32 %x, %s
  %s2 = sub i32 31, %xr
  %r2 = shl i32 %x, %s2
  ret i32 %r
}

define i8 @sub_nsw_smin(i8 %x, i8 %y) {
  %m = call i8 @llvm.smin.i8(i8 %x, i8 %y)
  %s = sub nsw i8 %x, %m
  %m2 = call i8 @llvm.smin.i8(i8 %y, i8 %x)
  %s2 = sub nsw i8 %x, %m2
  %s3 = sub i8 %x, %m2
  %s4 = sub nuw i8 %x, %y
  %s5 = sub nuw nsw i8 %x, %y
  ret i8 %s
}

define i8 @add_nowrap(i8 %x, i8 %y) {
  %xa = and i8 %x, 63
  %ya = and i8 %y, 63
  %a1 = add nuw i8 %xa, %ya
  %a2 = add nsw i8 %xa, %ya
  %n = or i8 %x, 128
  %n2 = or i8 %y, 192
  %a3 = add nsw i8 %n, %n2
  %a4 = add nuw nsw i8 %xa, %ya
  ret i8 %a1
}

define i32 @lerp(i32 %a, i32 %b, i32 %c, i32 %d) {
  %a1 = and i32 %a, 255
  %b1 = and i32 %b, 255
  %c1 = and i32 %c, 255
  %d1 = and i32 %d, 255
  %sub = sub nuw i32 %b1, %c1
  %m1 = mul i32 %a1, %sub
  %m2 = mul i32 %c1, %d1
  %r = add i32 %m1, %m2
  ret i32 %r
}

define i32 @lerp_implicit(i32 %b, i32 %c) {
  %b1 = and i32 %b, 255
  %c1 = and i32 %c, 255
  %sub = sub nuw i32 %b1, %c1
  %r = add i32 %sub, %c1
  %r2 = add i32 %c1, %sub
  ret i32 %r
}

define i32 @lerp_xor(i32 %a, i32 %c, i32 %d) {
  %a1 = and i32 %a, 255
  %c1 = and i32 %c, 15
  %d1 = and i32 %d, 255
  %x = xor i32 %c1, 255
  %m1 = mul i32 %a1, %x
  %m2 = mul i32 %c1, %d1
  %r = add i32 %m1, %m2
  %y = xor i32 %c1, 7
  %m3 = mul i32 %a1, %y
  %r2 = add i32 %m3, %m2
  ret i32 %r
}

define i16 @casts(i32 %x, i8 %y, i64 %z) {
  %t = trunc i32 %x to i16
  %zx = zext i8 %y to i16
  %sx = sext i8 %y to i16
  %n = and i8 %y, 127
  %zn = zext nneg i8 %y to i16
  %sn = sext i8 %n to i16
  %pi = ptrtoint ptr null to i16
  %ip = inttoptr i16 %t to ptr
  %fr = freeze i16 %t
  %f2 = freeze i16 %zx
  ret i16 %t
}

define i32 @select_arms(i1 %c, i32 %x, i32 %y) {
  %a = and i32 %x, 15
  %b = or i32 %y, 16
  %s = select i1 %c, i32 %a, i32 %b
  ret i32 %s
}
