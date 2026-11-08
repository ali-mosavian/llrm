target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

define internal i32 @pow(i32 %b, i32 %e) noinline {
  %z = icmp eq i32 %e, 0
  br i1 %z, label %done, label %rec
rec:
  %e1 = sub i32 %e, 1
  %r = call i32 @pow(i32 %b, i32 %e1)
  %m = mul i32 %r, %b
  %q = urem i32 %m, 30011
  ret i32 %q
done:
  ret i32 1
}

define i32 @sum(i32 %n, i32 %k) {
entry:
  br label %l
l:
  %i = phi i32 [ 0, %entry ], [ %j, %l ]
  %s = phi i32 [ 0, %entry ], [ %t, %l ]
  %v = call i32 @pow(i32 %i, i32 %k)
  %t = add i32 %s, %v
  %j = add i32 %i, 1
  %e = icmp ne i32 %j, %n
  br i1 %e, label %l, label %x
x:
  ret i32 %t
}
