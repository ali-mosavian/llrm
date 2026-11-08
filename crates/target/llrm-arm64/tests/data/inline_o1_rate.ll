target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define i32 @mid(i32 %x, i32 %y) {
  %v0 = add i32 %x, %x
  %v1 = xor i32 %v0, %y
  %v2 = mul i32 %v1, %x
  %v3 = sub i32 %v2, %y
  %v4 = or i32 %v3, %x
  %v5 = and i32 %v4, %y
  %v6 = add i32 %v5, %x
  %v7 = xor i32 %v6, %y
  %v8 = mul i32 %v7, %x
  %v9 = sub i32 %v8, %y
  %v10 = or i32 %v9, %x
  %v11 = and i32 %v10, %y
  %v12 = add i32 %v11, %x
  %v13 = xor i32 %v12, %y
  ret i32 %v13
}
define i32 @run(i32 %n) {
entry:
  br label %l
l:
  %i = phi i32 [ 0, %entry ], [ %j, %l ]
  %s = phi i32 [ 0, %entry ], [ %t2, %l ]
  %a = call i32 @mid(i32 %i, i32 %n)
  %b = add i32 %i, 5
  %c = call i32 @mid(i32 %b, i32 %n)
  %t = add i32 %s, %a
  %t2 = add i32 %t, %c
  %j = add i32 %i, 1
  %e = icmp ne i32 %j, %n
  br i1 %e, label %l, label %x
x:
  ret i32 %t2
}
