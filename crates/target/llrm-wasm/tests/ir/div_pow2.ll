define i32 @a(i32 %x) {
  %r = udiv i32 %x, 8
  ret i32 %r
}
define i64 @b(i64 %x) {
  %r = udiv i64 %x, 16
  ret i64 %r
}
define i8 @c(i8 %x) {
  %r = udiv i8 %x, 4
  ret i8 %r
}
define i32 @d(i32 %x) {
  %r = sdiv i32 %x, 4
  ret i32 %r
}
define i16 @e(i16 %x) {
  %r = sdiv i16 %x, 2
  ret i16 %r
}
define i32 @f(i32 %x) {
  %r = urem i32 %x, 7
  ret i32 %r
}
