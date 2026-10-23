target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

declare i32 @llvm.smin.i32(i32, i32)
declare i32 @llvm.smax.i32(i32, i32)
declare i32 @llvm.umin.i32(i32, i32)
declare i32 @llvm.umax.i32(i32, i32)
declare i32 @llvm.ctpop.i32(i32)
declare i32 @llvm.cttz.i32(i32, i1)
declare i32 @llvm.ctlz.i32(i32, i1)
declare i32 @llvm.bswap.i32(i32)
declare i32 @llvm.abs.i32(i32, i1)
declare float @llvm.fabs.f32(float)
declare float @llvm.sqrt.f32(float)
declare float @llvm.sin.f32(float)
declare float @llvm.cos.f32(float)
declare float @llvm.log.f32(float)
declare float @llvm.log2.f32(float)
declare float @llvm.exp.f32(float)
declare float @llvm.exp2.f32(float)
declare float @llvm.rint.f32(float)
declare i32 @llvm.lrint.i32.f32(float)
declare double @llvm.fabs.f64(double)
declare double @llvm.sqrt.f64(double)
declare double @llvm.sin.f64(double)
declare double @llvm.cos.f64(double)
declare double @llvm.log.f64(double)
declare double @llvm.log2.f64(double)
declare double @llvm.exp.f64(double)
declare double @llvm.exp2.f64(double)
declare double @llvm.rint.f64(double)
declare i32 @llvm.lrint.i32.f64(double)

define i32 @smin(i32 %a, i32 %b) {
  %r = call i32 @llvm.smin.i32(i32 %a, i32 %b)
  ret i32 %r
}

define i32 @smax(i32 %a, i32 %b) {
  %r = call i32 @llvm.smax.i32(i32 %a, i32 %b)
  ret i32 %r
}

define i32 @umin(i32 %a, i32 %b) {
  %r = call i32 @llvm.umin.i32(i32 %a, i32 %b)
  ret i32 %r
}

define i32 @umax(i32 %a, i32 %b) {
  %r = call i32 @llvm.umax.i32(i32 %a, i32 %b)
  ret i32 %r
}

define i32 @counts(i32 %a) {
  %p = call i32 @llvm.ctpop.i32(i32 %a)
  %t = call i32 @llvm.cttz.i32(i32 %a, i1 false)
  %u = call i32 @llvm.cttz.i32(i32 %a, i1 true)
  %l = call i32 @llvm.ctlz.i32(i32 %a, i1 false)
  %m = call i32 @llvm.ctlz.i32(i32 %a, i1 true)
  %s = add i32 %p, %t
  %s2 = add i32 %s, %u
  %s3 = add i32 %s2, %l
  %s4 = add i32 %s3, %m
  ret i32 %s4
}

define i32 @swap_abs(i32 %a) {
  %b = call i32 @llvm.bswap.i32(i32 %a)
  %c = call i32 @llvm.abs.i32(i32 %b, i1 false)
  %d = call i32 @llvm.abs.i32(i32 %c, i1 true)
  ret i32 %d
}

define float @float_f32(float %x) {
  %fabs = call float @llvm.fabs.f32(float %x)
  %sqrt = call float @llvm.sqrt.f32(float %x)
  %sin = call float @llvm.sin.f32(float %x)
  %cos = call float @llvm.cos.f32(float %x)
  %log = call float @llvm.log.f32(float %x)
  %log2 = call float @llvm.log2.f32(float %x)
  %exp = call float @llvm.exp.f32(float %x)
  %exp2 = call float @llvm.exp2.f32(float %x)
  %rint = call float @llvm.rint.f32(float %x)
  %s1 = fadd float %fabs, %sqrt
  %s2 = fadd float %s1, %sin
  %s3 = fadd float %s2, %cos
  %s4 = fadd float %s3, %log
  %s5 = fadd float %s4, %log2
  %s6 = fadd float %s5, %exp
  %s7 = fadd float %s6, %exp2
  %s8 = fadd float %s7, %rint
  ret float %s8
}

define i32 @lrint_f32(float %x) {
  %r = call i32 @llvm.lrint.i32.f32(float %x)
  ret i32 %r
}

define double @float_f64(double %x) {
  %fabs = call double @llvm.fabs.f64(double %x)
  %sqrt = call double @llvm.sqrt.f64(double %x)
  %sin = call double @llvm.sin.f64(double %x)
  %cos = call double @llvm.cos.f64(double %x)
  %log = call double @llvm.log.f64(double %x)
  %log2 = call double @llvm.log2.f64(double %x)
  %exp = call double @llvm.exp.f64(double %x)
  %exp2 = call double @llvm.exp2.f64(double %x)
  %rint = call double @llvm.rint.f64(double %x)
  %s1 = fadd double %fabs, %sqrt
  %s2 = fadd double %s1, %sin
  %s3 = fadd double %s2, %cos
  %s4 = fadd double %s3, %log
  %s5 = fadd double %s4, %log2
  %s6 = fadd double %s5, %exp
  %s7 = fadd double %s6, %exp2
  %s8 = fadd double %s7, %rint
  ret double %s8
}

define i32 @lrint_f64(double %x) {
  %r = call i32 @llvm.lrint.i32.f64(double %x)
  ret i32 %r
}
