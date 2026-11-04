target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
@g = global i32 0
@buf = global [16 x i8] zeroinitializer
declare void @a()
declare void @b()
declare i32 @rd()
define void @f(i32 %n) {
e:
  br label %loop
loop:
  %i = phi i32 [ 0, %e ], [ %i2, %m ]
  %r = call i32 @rd()
  %c = icmp slt i32 %r, 5
  br i1 %c, label %x, label %y
x:
  %p = getelementptr i8, ptr @buf, i32 %r
  %v = load i8, ptr %p
  %k = icmp ne i8 %v, -1
  br label %m
y:
  br label %m
m:
  %phi = phi i1 [ true, %y ], [ %k, %x ]
  %l = load i32, ptr @g
  %t = icmp eq i32 %l, 0
  %s = select i1 %phi, i1 %t, i1 false
  %i2 = add i32 %i, 1
  br i1 %s, label %loop, label %out
out:
  call void @b()
  ret void
}
