target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
@t = global [100 x i16] zeroinitializer
define i16 @f(i16 %x, i16 %y) {
  %m = mul i16 %x, 36
  %a = getelementptr i8, ptr @t, i16 %m
  %v = load i16, ptr %a
  %w = add i16 %v, %y
  ret i16 %w
}
