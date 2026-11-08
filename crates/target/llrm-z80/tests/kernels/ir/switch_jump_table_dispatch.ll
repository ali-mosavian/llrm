target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16"
declare void @a()
declare void @b()
declare void @c()
define void @f(i16 %x) {
  switch i16 %x, label %d [ i16 0, label %A i16 1, label %B i16 2, label %C i16 3, label %A i16 4, label %B i16 5, label %C i16 6, label %A i16 7, label %B ]
A: call void @a()
  ret void
B: call void @b()
  ret void
C: call void @c()
  ret void
d: ret void
}
