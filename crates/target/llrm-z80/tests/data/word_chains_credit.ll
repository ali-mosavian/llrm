target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"

define i16 @by_hundred(i16 %x) {
  %p = mul i16 %x, 100
  ret i16 %p
}

define i16 @by_eleven(i16 %x) {
  %p = mul i16 %x, 11
  ret i16 %p
}
