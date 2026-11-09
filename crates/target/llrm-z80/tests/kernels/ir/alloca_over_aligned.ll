target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare void @use(ptr)
define i16 @f() {
  %a = alloca [64 x i8], align 64
  call void @use(ptr %a)
  %i = ptrtoint ptr %a to i16
  %m = and i16 %i, 63
  ret i16 %m
}
