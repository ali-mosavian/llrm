target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
define i32 @m3(i32 %n) {
 %r = mul i32 %n, 3
 ret i32 %r
}
