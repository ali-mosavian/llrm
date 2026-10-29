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

declare i64 @c_many(i64, i64, i64, i64, i64, i64, i64, i64, i64, i8, i32)
declare i32 @c_twice(i32)

define i64 @call_many(i64 %a) {
  %b = add i64 %a, 1
  %c = add i64 %a, 2
  %d = add i64 %a, 3
  %e = add i64 %a, 4
  %f = add i64 %a, 5
  %g = add i64 %a, 6
  %h = add i64 %a, 7
  %i = add i64 %a, 8
  %r = call i64 @c_many(i64 %a, i64 %b, i64 %c, i64 %d, i64 %e, i64 %f, i64 %g, i64 %h, i64 %i, i8 -3, i32 4)
  %s = add i64 %r, %a
  ret i64 %s
}

define i64 @take_many(i64 %a, i64 %b, i64 %c, i64 %d, i64 %e, i64 %f, i64 %g, i64 %h, i64 %i, i8 %j, i32 %k) {
  %jx = sext i8 %j to i64
  %kx = zext i32 %k to i64
  %s = add i64 %a, %i
  %t = mul i64 %s, 3
  %u = add i64 %t, %jx
  %v = add i64 %u, %kx
  ret i64 %v
}

define i32 @twice(i32 %x) {
  %r = call i32 @c_twice(i32 %x)
  %s = call i32 @c_twice(i32 %r)
  %t = add i32 %r, %s
  %u = add i32 %t, %x
  ret i32 %u
}

define i32 @indirect(ptr %fp, i32 %x) {
  %r = call i32 %fp(i32 %x)
  ret i32 %r
}

declare double @c_fmix(i32, double, float, i64, double, double, double, double, double, double, double, float, i8)
declare float @c_half(float)

define double @fops(double %a, double %b) {
  %s = fadd double %a, %b
  %d = fsub double %a, %b
  %m = fmul double %s, %d
  %q = fdiv double %m, %b
  %n = fneg double %q
  ret double %n
}

define i32 @fcmps(double %a, double %b) {
  %c1 = fcmp olt double %a, %b
  %c2 = fcmp ole double %a, %b
  %c3 = fcmp ogt double %a, %b
  %c4 = fcmp oge double %a, %b
  %c5 = fcmp oeq double %a, %b
  %c6 = fcmp one double %a, %b
  %c7 = fcmp ueq double %a, %b
  %c8 = fcmp une double %a, %b
  %c9 = fcmp uno double %a, %b
  %c10 = fcmp ord double %a, %b
  %c11 = fcmp ult double %a, %b
  %c12 = fcmp uge double %a, %b
  %z1 = zext i1 %c1 to i32
  %z2 = zext i1 %c2 to i32
  %z3 = zext i1 %c3 to i32
  %z4 = zext i1 %c4 to i32
  %z5 = zext i1 %c5 to i32
  %z6 = zext i1 %c6 to i32
  %z7 = zext i1 %c7 to i32
  %z8 = zext i1 %c8 to i32
  %z9 = zext i1 %c9 to i32
  %z10 = zext i1 %c10 to i32
  %z11 = zext i1 %c11 to i32
  %z12 = zext i1 %c12 to i32
  %s2 = shl i32 %z2, 1
  %s3 = shl i32 %z3, 2
  %s4 = shl i32 %z4, 3
  %s5 = shl i32 %z5, 4
  %s6 = shl i32 %z6, 5
  %s7 = shl i32 %z7, 6
  %s8 = shl i32 %z8, 7
  %s9 = shl i32 %z9, 8
  %s10 = shl i32 %z10, 9
  %s11 = shl i32 %z11, 10
  %s12 = shl i32 %z12, 11
  %t1 = or i32 %z1, %s2
  %t2 = or i32 %t1, %s3
  %t3 = or i32 %t2, %s4
  %t4 = or i32 %t3, %s5
  %t5 = or i32 %t4, %s6
  %t6 = or i32 %t5, %s7
  %t7 = or i32 %t6, %s8
  %t8 = or i32 %t7, %s9
  %t9 = or i32 %t8, %s10
  %t10 = or i32 %t9, %s11
  %t11 = or i32 %t10, %s12
  ret i32 %t11
}

define double @fmaxd(double %a, double %b) {
  %c = fcmp ogt double %a, %b
  %r = select i1 %c, double %a, double %b
  ret double %r
}

define double @convert(i32 %a, i64 %b, float %f) {
  %x = sitofp i32 %a to double
  %y = uitofp i64 %b to double
  %z = fpext float %f to double
  %s = fadd double %x, %y
  %t = fadd double %s, %z
  %u = fptrunc double %t to float
  %v = fpext float %u to double
  %i = fptosi double %v to i32
  %j = sitofp i32 %i to double
  ret double %j
}

define double @sum_array(ptr %p, i32 %n) {
entry:
  br label %h
h:
  %i = phi i32 [0, %entry], [%i2, %h]
  %s = phi double [0.0, %entry], [%s2, %h]
  %q = getelementptr double, ptr %p, i32 %i
  %v = load double, ptr %q
  %s2 = fadd double %s, %v
  %i2 = add i32 %i, 1
  %c = icmp ult i32 %i2, %n
  br i1 %c, label %h, label %x
x:
  ret double %s2
}

define void @scale(ptr %p, float %k) {
  %v = load float, ptr %p
  %w = fmul float %v, %k
  store float %w, ptr %p
  ret void
}

define double @fcalls(double %a, i32 %n) {
  %f = sitofp i32 %n to float
  %g = call float @c_half(float %f)
  %r = call double @c_fmix(i32 %n, double %a, float %g, i64 7, double %a, double %a, double %a, double %a, double %a, double %a, double %a, float %g, i8 5)
  %s = fadd double %r, %a
  ret double %s
}

define i32 @fbits(float %a) {
  %r = bitcast float %a to i32
  ret i32 %r
}

declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)

define void @cp15(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 15, i1 false)
  ret void
}

define void @cp300(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 300, i1 false)
  ret void
}

define void @cpn(ptr %d, ptr %s, i64 %n) {
  call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 %n, i1 false)
  ret void
}

define void @mv32(ptr %d, ptr %s) {
  call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 32, i1 false)
  ret void
}

define void @mvn(ptr %d, ptr %s, i64 %n) {
  call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 %n, i1 false)
  ret void
}

define void @z24(ptr %d) {
  call void @llvm.memset.p0.i64(ptr %d, i8 0, i64 24, i1 false)
  ret void
}

define void @fill12(ptr %d, i8 %c) {
  call void @llvm.memset.p0.i64(ptr %d, i8 %c, i64 12, i1 false)
  ret void
}

define void @setn(ptr %d, i8 %c, i64 %n) {
  call void @llvm.memset.p0.i64(ptr %d, i8 %c, i64 %n, i1 false)
  ret void
}
