define i16 @narrow(i16 %x) {
  %s = shl i16 %x, 8
  %r = ashr i16 %s, 8
  ret i16 %r
}

define i32 @narrow_wide(i16 %x) {
  %s = shl i16 %x, 8
  %r = ashr i16 %s, 8
  %e = sext i16 %r to i32
  ret i32 %e
}
