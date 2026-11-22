target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define internal i32 @leaf(i32 %x, i32 %y) HINT nounwind optsize {
  %a = mul i32 %x, %y
  %b = add i32 %a, %x
  %c = xor i32 %b, %y
  %d = shl i32 %c, 3
  %e = sub i32 %d, %a
  %f1 = and i32 %e, 255
  %g = mul i32 %f1, %x
  %h = add i32 %g, %y
  %i = xor i32 %h, %e
  %j = shl i32 %i, 2
  %k = sub i32 %j, %g
  %l = and i32 %k, 1023
  %m = add i32 %l, %a
  %f = xor i32 %m, %d
  ret i32 %f
}

define i32 @one(i32 %x, i32 %y) nounwind optsize {
  %r = call i32 @leaf(i32 %x, i32 %y)
  ret i32 %r
}

define i32 @two(i32 %x, i32 %y) nounwind optsize {
  %r = call i32 @leaf(i32 %y, i32 %x)
  %s = add i32 %r, 1
  ret i32 %s
}
