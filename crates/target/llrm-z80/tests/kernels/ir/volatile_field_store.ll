target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
@v = global i32 0
define void @store_low_bits() {
  %x = load volatile i32, ptr @v, align 1
  %a = and i32 %x, -8
  %b = or i32 %a, 5
  store volatile i32 %b, ptr @v, align 1
  ret void
}
