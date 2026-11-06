target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare i16 @g(i16)
define i16 @f(i16 %a) optsize {
  %x = call i16 @g(i16 %a)
  %y = call i16 @g(i16 %x)
  %r = add i16 %x, %y
  ret i16 %r
}
