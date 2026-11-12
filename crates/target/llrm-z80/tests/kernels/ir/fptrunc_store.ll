target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define void @g(ptr %q, ptr %p) {
  %x = load double, ptr %q
  %r = fptrunc double %x to float
  store float %r, ptr %p
  ret void
}
