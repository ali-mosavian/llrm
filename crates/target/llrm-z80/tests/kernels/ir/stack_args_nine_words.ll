target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
declare i16 @g(i16, i16, i16, i16, i16, i16, i16, i16, i16, i16, i16, i16)
define i16 @f(i16 %a) {
  %r = call i16 @g(i16 %a, i16 1, i16 2, i16 3, i16 4, i16 5, i16 6, i16 7, i16 8, i16 9, i16 10, i16 11)
  ret i16 %r
}
