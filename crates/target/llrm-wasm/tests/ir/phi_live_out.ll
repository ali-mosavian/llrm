target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare void @b()
define void @f(i8 %v, i32 %k, i32 %j) {
e:
  %t = icmp eq i32 %k, 0
  br i1 %t, label %a, label %b
a:
  %c = icmp ne i8 %v, -1
  br label %m
b:
  br label %m
m:
  %p = phi i1 [ true, %b ], [ %c, %a ]
  %q = icmp eq i32 %j, 0
  %s = select i1 %p, i1 %q, i1 false
  br i1 %s, label %x, label %y
x:
  call void @b()
  ret void
y:
  ret void
}
