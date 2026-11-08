target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define weak i32 @leaf(i32 %x) noinline {
  %a = add i32 %x, 1
  %b = mul i32 %a, 3
  ret i32 %b
}

define i32 @sum(i32 %n, i32 %k) {
entry:
  br label %l
l:
  %i = phi i32 [ 0, %entry ], [ %j, %l ]
  %s = phi i32 [ 0, %entry ], [ %t, %l ]
  %v = call i32 @leaf(i32 %i)
  %m = mul i32 %v, %k
  %t = add i32 %s, %m
  %j = add i32 %i, 1
  %e = icmp ne i32 %j, %n
  br i1 %e, label %l, label %x
x:
  ret i32 %t
}
