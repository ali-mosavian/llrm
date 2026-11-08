target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
define i16 @f(i16 %x) {
  %m = mul i16 %x, 36
  ret i16 %m
}
define i16 @g(i16 %x, ptr %p) {
  %m = mul i16 %x, 20
  %a = getelementptr i8, ptr %p, i16 %m
  %v = load i16, ptr %a
  ret i16 %v
}
