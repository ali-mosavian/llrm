target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

define i32 @once(i16 %a, i16 %b) {
  %x = zext i16 %a to i32
  %y = zext i16 %b to i32
  %p = mul i32 %x, %y
  ret i32 %p
}
