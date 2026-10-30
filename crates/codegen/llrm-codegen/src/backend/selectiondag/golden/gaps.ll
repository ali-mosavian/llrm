target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

declare i32 @llvm.scmp.i32.i32(i32, i32)
declare i8 @llvm.ucmp.i8.i32(i32, i32)
declare double @llvm.fmuladd.f64(double, double, double)
declare float @llvm.fmuladd.f32(float, float, float)
declare i32 @llvm.smul.fix.i32(i32, i32, i32)
declare i32 @llvm.sdiv.fix.i32(i32, i32, i32)
declare void @llvm.assume(i1)
declare i1 @llvm.expect.i1(i1, i1)
declare i32 @llvm.expect.i32(i32, i32)
declare ptr @llvm.stacksave.p0()
declare void @llvm.stackrestore.p0(ptr)

define i32 @cmp(i32 %a, i32 %b) {
  %s = call i32 @llvm.scmp.i32.i32(i32 %a, i32 %b)
  %u = call i8 @llvm.ucmp.i8.i32(i32 %a, i32 %b)
  %z = zext i8 %u to i32
  %r = add i32 %s, %z
  ret i32 %r
}

define double @fma64(double %a, double %b, double %c) {
  %r = call double @llvm.fmuladd.f64(double %a, double %b, double %c)
  ret double %r
}

define float @fma32(float %a, float %b, float %c) {
  %r = call float @llvm.fmuladd.f32(float %a, float %b, float %c)
  ret float %r
}

define i32 @fixed(i32 %a, i32 %b) {
  %m = call i32 @llvm.smul.fix.i32(i32 %a, i32 %b, i32 8)
  %d = call i32 @llvm.sdiv.fix.i32(i32 %a, i32 %b, i32 8)
  %r = add i32 %m, %d
  ret i32 %r
}

define i32 @hints(i32 %a, i32 %b) {
  %c = icmp slt i32 %a, %b
  call void @llvm.assume(i1 %c)
  %e = call i32 @llvm.expect.i32(i32 %a, i32 3)
  %f = call i1 @llvm.expect.i1(i1 %c, i1 true)
  br i1 %f, label %t, label %o
t:
  ret i32 %e
o:
  ret i32 %b
}

define void @stack() {
  %s = call ptr @llvm.stacksave.p0()
  call void @llvm.stackrestore.p0(ptr %s)
  ret void
}
