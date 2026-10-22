target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

%S = type { i32, i16, i8 }

@g1 = global i32 0
@h = global i16 0
@arr = global [16 x i32] zeroinitializer
@s = global %S zeroinitializer

define void @f() {
b0:
  %a = load i32, ptr @g1
  %b = load i16, ptr @h
  br label %b1
b1:
  %n = icmp slt i32 %a, 7
  %m = zext i1 %n to i32
  %big = icmp eq i32 %a, 0
  %s = add nsw i32 %a, %m
  %c = icmp ult i32 %s, 10
  %d = select i1 %c, i32 %s, i32 %a
  %w = sext i16 %b to i32
  %x = shl i32 %d, %w
  %p = getelementptr [16 x i32], ptr @arr, i32 0, i32 %x
  %q = getelementptr %S, ptr @s, i32 0, i32 1
  store i32 %d, ptr %p
  store i16 %b, ptr %q
  %t = trunc i32 %x to i8
  %z = and i8 %t, 15
  store i8 %z, ptr @g1
  br i1 %big, label %b2, label %b3
b2:
  %r = phi i32 [ 5, %b1 ], [ %a, %b3 ]
  %u = udiv i32 %r, 3
  store i32 %u, ptr @g1
  unreachable
b3:
  %v = xor i32 %a, -1
  %o = or i32 %v, %s
  store volatile i32 %o, ptr @g1
  %l = load i32, ptr @g1
  store i32 %l, ptr @g1
  br label %b2
}
