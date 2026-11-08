target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare void @a()
define void @f(i16 %x, ptr %p) {
  %v = load i16, ptr %p
  %c = icmp sgt i16 %x, %v
  br i1 %c, label %t, label %e
t:
  call void @a()
  ret void
e:
  ret void
}
