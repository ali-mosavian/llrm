target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
@v = global i16 0

; a bit field assigned: the low byte masked then or-ed with the new bits
define void @set() {
  %x = load volatile i16, ptr @v
  %m = and i16 %x, -8
  %o = or i16 %m, 5
  store volatile i16 %o, ptr @v
  ret void
}
