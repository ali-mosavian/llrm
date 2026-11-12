target datalayout = "e-p:16:8-i16:8-i32:8-i64:8-f32:8-f64:8-a:8-n8:16-S8"
define i32 @g(i32 %x, i32 %m) {
entry:
  br label %l
l:
  %p = phi i32 [ %x, %entry ], [ %s, %l ]
  %a = and i32 %p, %m
  %z = icmp eq i32 %a, 0
  %s = shl i32 %p, 1
  br i1 %z, label %l, label %e
e:
  ret i32 %p
}
