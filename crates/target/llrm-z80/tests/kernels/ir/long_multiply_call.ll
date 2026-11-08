target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
define i32 @f(i32 %a, i32 %b) {
  %m = mul i32 %a, %b
  ret i32 %m
}
define i32 @g(i32 %a, i32 %b, i32 %c) {
  %m = mul i32 %a, %b
  %n = mul i32 %m, %c
  ret i32 %n
}
