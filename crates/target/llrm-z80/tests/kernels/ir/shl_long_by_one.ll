target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
define i32 @f(i32 %n) {
  %c = shl i32 %n, 1
  ret i32 %c
}
define i32 @g(i32 %n) {
  %c = mul i32 %n, 3
  ret i32 %c
}
