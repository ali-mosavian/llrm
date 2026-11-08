target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare i32 @next()
declare void @s1()
declare void @s2()
declare void @s3()
declare void @s4()
define void @f() {
entry:
  %w = call i32 @next()
  %s = and i32 %w, 7
  %c3 = icmp eq i32 %s, 3
  br i1 %c3, label %a3, label %b
b:
  %c2 = icmp eq i32 %s, 2
  br i1 %c2, label %a2, label %c
c:
  %c1 = icmp eq i32 %s, 1
  br i1 %c1, label %a1, label %d
a3:
  call void @s3()
  ret void
a2:
  call void @s2()
  ret void
a1:
  call void @s1()
  ret void
d:
  call void @s4()
  ret void
}
