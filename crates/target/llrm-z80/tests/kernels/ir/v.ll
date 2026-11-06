target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
define i16 @shv(i16 %a, i8 %n) {
  %c = zext i8 %n to i16
  %r = shl i16 %a, %c
  ret i16 %r
}
define i16 @dv(i16 %a, i16 %b) {
  %q = sdiv i16 %a, %b
  %r = srem i16 %a, %b
  %s = add i16 %q, %r
  ret i16 %s
}
