target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx"

; sum of 0..n-1
define i32 @sum_to(i32 %n) {
entry:
  br label %h
h:
  %i = phi i32 [0, %entry], [%i2, %h]
  %s = phi i32 [0, %entry], [%s2, %h]
  %s2 = add i32 %s, %i
  %i2 = add i32 %i, 1
  %c = icmp slt i32 %i2, %n
  br i1 %c, label %h, label %x
x:
  ret i32 %s2
}

; the compare codes of a and b, a weight each (select and add: no shifts yet)
define i32 @masks(i32 %a, i32 %b) {
  %c0 = icmp eq i32 %a, %b
  %z0 = select i1 %c0, i32 3, i32 0
  %c1 = icmp ne i32 %a, %b
  %z1 = select i1 %c1, i32 5, i32 0
  %c2 = icmp slt i32 %a, %b
  %z2 = select i1 %c2, i32 6, i32 0
  %c3 = icmp sle i32 %a, %b
  %z3 = select i1 %c3, i32 7, i32 0
  %c4 = icmp sgt i32 %a, %b
  %z4 = select i1 %c4, i32 9, i32 0
  %c5 = icmp sge i32 %a, %b
  %z5 = select i1 %c5, i32 10, i32 0
  %c6 = icmp ult i32 %a, %b
  %z6 = select i1 %c6, i32 11, i32 0
  %c7 = icmp ule i32 %a, %b
  %z7 = select i1 %c7, i32 12, i32 0
  %c8 = icmp ugt i32 %a, %b
  %z8 = select i1 %c8, i32 13, i32 0
  %c9 = icmp uge i32 %a, %b
  %z9 = select i1 %c9, i32 14, i32 0
  %r1 = add i32 %z0, %z1
  %r2 = add i32 %r1, %z2
  %r3 = add i32 %r2, %z3
  %r4 = add i32 %r3, %z4
  %r5 = add i32 %r4, %z5
  %r6 = add i32 %r5, %z6
  %r7 = add i32 %r6, %z7
  %r8 = add i32 %r7, %z8
  %r9 = add i32 %r8, %z9
  ret i32 %r9
}

define i64 @clamp(i64 %x, i64 %lo, i64 %hi) {
  %a = icmp slt i64 %x, %lo
  %b = select i1 %a, i64 %lo, i64 %x
  %c = icmp sgt i64 %b, %hi
  %d = select i1 %c, i64 %hi, i64 %b
  ret i64 %d
}

; gcd by subtraction (a diamond inside a loop)
define i32 @gcd(i32 %x, i32 %y) {
entry:
  br label %h
h:
  %a = phi i32 [%x, %entry], [%a2, %latch]
  %b = phi i32 [%y, %entry], [%b2, %latch]
  %done = icmp eq i32 %a, %b
  br i1 %done, label %out, label %body
body:
  %gt = icmp ugt i32 %a, %b
  br i1 %gt, label %big, label %small
big:
  %d = sub i32 %a, %b
  br label %latch
small:
  %e = sub i32 %b, %a
  br label %latch
latch:
  %a2 = phi i32 [%d, %big], [%a, %small]
  %b2 = phi i32 [%b, %big], [%e, %small]
  br label %h
out:
  ret i32 %a
}

; the signed/unsigned sign test and a small constant compare
define i32 @signs(i32 %x) {
entry:
  %neg = icmp slt i32 %x, 0
  br i1 %neg, label %n, label %p
n:
  ret i32 -1
p:
  %big = icmp ugt i32 %x, 100
  %r = select i1 %big, i32 2, i32 1
  ret i32 %r
}

define i32 @shifts(i32 %a, i32 %b) {
  %s = and i32 %b, 31
  %l = shl i32 %a, %s
  %r = lshr i32 %a, %s
  %q = ashr i32 %a, %s
  %r3 = mul i32 %r, 3
  %q5 = mul i32 %q, 5
  %t = add i32 %l, %r3
  %u = add i32 %t, %q5
  ret i32 %u
}

define i32 @logic(i32 %a, i32 %b) {
  %x = and i32 %a, %b
  %y = or i32 %a, 61680
  %z = xor i32 %a, %b
  %n = xor i32 %b, -1
  %m = mul i32 %a, %b
  %ng = sub i32 0, %a
  %t1 = add i32 %x, %y
  %t2 = add i32 %t1, %z
  %t3 = add i32 %t2, %n
  %t4 = add i32 %t3, %m
  %t5 = add i32 %t4, %ng
  ret i32 %t5
}

define i32 @divs(i32 %a, i32 %b) {
  %q = sdiv i32 %a, %b
  %u = udiv i32 %a, %b
  %r = srem i32 %a, %b
  %t = add i32 %q, %u
  %v = add i32 %t, %r
  ret i32 %v
}

define i64 @widen(i32 %a) {
  %b = trunc i32 %a to i8
  %s8 = sext i8 %b to i64
  %z8 = zext i8 %b to i64
  %s = sext i32 %a to i64
  %z = zext i32 %a to i64
  %t = add i64 %s8, %z8
  %u = add i64 %t, %s
  %v = add i64 %u, %z
  %w = shl i64 %v, 3
  ret i64 %w
}
