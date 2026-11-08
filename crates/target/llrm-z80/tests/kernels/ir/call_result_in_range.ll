target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare range(i32 -128, 128) i32 @small(i16)
declare void @report(i32)
define void @caller(i16 %x) {
  %r = call i32 @small(i16 %x)
  call void @report(i32 %r)
  ret void
}
