target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare void @report(i32)
define void @ne(i32 %a, i32 %b) {
  %c = icmp ne i32 %a, %b
  br i1 %c, label %t, label %f
t:
  call void @report(i32 1)
  ret void
f:
  ret void
}
define void @eqk(i32 %a) {
  %c = icmp eq i32 %a, 305419896
  br i1 %c, label %t, label %f
t:
  call void @report(i32 1)
  ret void
f:
  ret void
}
define void @zero(i32 %a) {
  %c = icmp eq i32 %a, 0
  br i1 %c, label %t, label %f
t:
  call void @report(i32 1)
  ret void
f:
  ret void
}
