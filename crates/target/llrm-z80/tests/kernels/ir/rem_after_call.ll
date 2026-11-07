target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare void @between()
declare void @use(i32, i32)
define void @f(i32 %x) {
  %q = udiv i32 %x, 7
  %r = urem i32 %x, 7
  call void @between()
  call void @use(i32 %q, i32 %r)
  ret void
}
