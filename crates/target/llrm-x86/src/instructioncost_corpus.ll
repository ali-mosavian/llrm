target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"
@g = global [8 x i16] zeroinitializer
@s = global { i8, i32, [4 x i16] } zeroinitializer
define void @arith(i16 %a, i16 %b, i32 %w, i32 %x, double %d, float %f) {
  %a1 = add i16 %a, %b
  %a2 = mul i16 %a, 8
  %a3 = sdiv i32 %w, 3
  %a4 = shl i32 %w, 4
  %a5 = and i16 %a, -8
  %a6 = fmul double %d, %d
  %a7 = fadd float %f, 1.0
  %a8 = fneg double %d
  %a9 = udiv i32 %w, %x
  ret void
}
define void @casts(i16 %a, i32 %w, double %d, float %f, ptr %p) {
  %c1 = zext i16 %a to i32
  %c2 = sext i16 %a to i32
  %c3 = trunc i32 %w to i16
  %c4 = sitofp i32 %w to double
  %c5 = fptosi double %d to i32
  %c6 = fpext float %f to double
  %c7 = fptrunc double %d to float
  %c8 = ptrtoint ptr %p to i32
  %c9 = inttoptr i32 %w to ptr
  %c10 = bitcast float %f to i32
  ret void
}
define void @cmpsel(i16 %a, i16 %b, i1 %c, i1 %e, double %d) {
  %m1 = icmp slt i16 %a, %b
  %m2 = icmp eq i16 %a, 0
  %m3 = fcmp olt double %d, %d
  %s1 = select i1 %c, i16 %a, i16 %b
  %s2 = select i1 %c, i1 %e, i1 false
  %s3 = select i1 %c, i1 true, i1 %e
  %s4 = select i1 %m1, double %d, double %d
  ret void
}
define void @memory(ptr %p, i16 %a, i32 %w, double %d) {
  %l1 = load i16, ptr %p
  %l2 = load i32, ptr %p
  %l3 = load double, ptr %p
  %l4 = load i32, ptr %p
  %l5 = trunc i32 %l4 to i16
  store i16 %a, ptr %p
  store i32 7, ptr %p
  store double %d, ptr %p
  ret void
}
define void @geps(ptr %p, i16 %i, i32 %w, i32 %j) {
  %g1 = getelementptr i16, ptr %p, i16 %i
  %g2 = getelementptr i16, ptr %p, i32 3
  %g3 = getelementptr i8, ptr %p, i32 %w
  %g4 = getelementptr [8 x i16], ptr %p, i32 %w, i32 %j
  %g5 = getelementptr i32, ptr %p
  %g6 = getelementptr i16, ptr @g, i16 %i
  %g7 = getelementptr i16, ptr @g, i32 3
  %g8 = getelementptr { i8, i32, [4 x i16] }, ptr %p, i32 0, i32 2, i32 %w
  %g9 = getelementptr { i8, i32, [4 x i16] }, ptr @s, i32 0, i32 1
  %g10 = getelementptr i32, ptr %p, i32 1000000
  %g11 = getelementptr i64, ptr %p, i32 %w
  %g12 = getelementptr i32, ptr %p, i32 %w
  %g13 = getelementptr i8, ptr %p, i32 -4
  %u1 = load i32, ptr %g12
  %g14 = getelementptr i16, ptr %p, i32 %j
  %u2 = load i32, ptr %g14
  ret void
}
define void @flow(i32 %w, i16 %a) {
entry:
  switch i32 %w, label %d [ i32 1, label %x
                           i32 2, label %x ]
x:
  %p = phi i16 [ %a, %entry ], [ %a, %entry ]
  br label %e
d:
  br label %e
e:
  %q = phi i16 [ 1, %d ], [ %p, %x ]
  %al = alloca i32
  %an = alloca i32, i32 %w
  %fr = freeze i16 %q
  ret void
}
define void @calls(ptr %p, i16 %a, i32 %w, double %d, ptr %q) {
  %r1 = call i16 @ext(i16 %a, i16 %a)
  call void @ext0()
  %r2 = call double @sqrt(double %d)
  %r3 = call double @pow(double %d, double %d)
  %r4 = call i32 @in(i32 %w)
  %r5 = call i16 %q(i16 %a)
  %r6 = call i16 @llvm.smax.i16(i16 %a, i16 %a)
  call void @llvm.assume(i1 true)
  call void @llvm.memcpy.p0.p0.i32(ptr %p, ptr %p, i32 8, i1 false)
  call void @llvm.memset.p0.i32(ptr %p, i8 0, i32 %w, i1 false)
  ret void
}
declare i16 @ext(i16, i16)
declare void @ext0()
declare double @sqrt(double)
declare double @pow(double, double)
define internal i32 @in(i32 %x) {
  ret i32 %x
}
define void @intrinsics(i16 %a, i32 %w, double %d, float %f, ptr %p) {
  %v0 = call i16 @llvm.smax.i16(i16 %a, i16 %a)
  %v1 = call i16 @llvm.smin.i16(i16 %a, i16 %a)
  %v2 = call i16 @llvm.umax.i16(i16 %a, i16 %a)
  %v3 = call i16 @llvm.umin.i16(i16 %a, i16 %a)
  %v4 = call { i16, i1 } @llvm.sadd.with.overflow.i16(i16 %a, i16 %a)
  %v5 = call { i16, i1 } @llvm.uadd.with.overflow.i16(i16 %a, i16 %a)
  %v6 = call { i16, i1 } @llvm.ssub.with.overflow.i16(i16 %a, i16 %a)
  %v7 = call { i16, i1 } @llvm.usub.with.overflow.i16(i16 %a, i16 %a)
  %v8 = call { i16, i1 } @llvm.smul.with.overflow.i16(i16 %a, i16 %a)
  %v9 = call { i16, i1 } @llvm.umul.with.overflow.i16(i16 %a, i16 %a)
  %v10 = call i32 @llvm.smax.i32(i32 %w, i32 %w)
  %v11 = call i32 @llvm.smin.i32(i32 %w, i32 %w)
  %v12 = call i32 @llvm.umax.i32(i32 %w, i32 %w)
  %v13 = call i32 @llvm.umin.i32(i32 %w, i32 %w)
  %v14 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %w, i32 %w)
  %v15 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %w, i32 %w)
  %v16 = call { i32, i1 } @llvm.ssub.with.overflow.i32(i32 %w, i32 %w)
  %v17 = call { i32, i1 } @llvm.usub.with.overflow.i32(i32 %w, i32 %w)
  %v18 = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %w, i32 %w)
  %v19 = call { i32, i1 } @llvm.umul.with.overflow.i32(i32 %w, i32 %w)
  %v20 = call double @llvm.fabs.f64(double %d)
  %v21 = call double @llvm.sqrt.f64(double %d)
  %v22 = call double @llvm.sin.f64(double %d)
  %v23 = call double @llvm.cos.f64(double %d)
  %v24 = call double @llvm.log2.f64(double %d)
  %v25 = call double @llvm.exp2.f64(double %d)
  %v26 = call double @llvm.rint.f64(double %d)
  %v27 = call double @llvm.fmuladd.f64(double %d, double %d, double %d)
  %v28 = call float @llvm.fabs.f32(float %f)
  %v29 = call float @llvm.sqrt.f32(float %f)
  %v30 = call float @llvm.sin.f32(float %f)
  %v31 = call float @llvm.cos.f32(float %f)
  %v32 = call float @llvm.log2.f32(float %f)
  %v33 = call float @llvm.exp2.f32(float %f)
  %v34 = call float @llvm.rint.f32(float %f)
  %v35 = call float @llvm.fmuladd.f32(float %f, float %f, float %f)
  %v36 = call i32 @llvm.lrint.i32.f64(double %d)
  %v37 = call i16 @llvm.ctpop.i16(i16 %a)
  %v38 = call i32 @llvm.bswap.i32(i32 %w)
  %v39 = call i16 @llvm.abs.i16(i16 %a, i1 false)
  %v40 = call i16 @llvm.smul.fix.i16(i16 %a, i16 %a, i32 3)
  call void @llvm.assume(i1 false)
  call void @llvm.memcpy.p0.p0.i32(ptr %p, ptr %p, i32 %w, i1 false)
  call void @llvm.memmove.p0.p0.i32(ptr %p, ptr %p, i32 %w, i1 false)
  call void @llvm.memset.p0.i32(ptr %p, i8 0, i32 %w, i1 false)
  ret void
}
declare i16 @llvm.smax.i16(i16, i16)
declare i16 @llvm.smin.i16(i16, i16)
declare i16 @llvm.umax.i16(i16, i16)
declare i16 @llvm.umin.i16(i16, i16)
declare { i16, i1 } @llvm.sadd.with.overflow.i16(i16, i16)
declare { i16, i1 } @llvm.uadd.with.overflow.i16(i16, i16)
declare { i16, i1 } @llvm.ssub.with.overflow.i16(i16, i16)
declare { i16, i1 } @llvm.usub.with.overflow.i16(i16, i16)
declare { i16, i1 } @llvm.smul.with.overflow.i16(i16, i16)
declare { i16, i1 } @llvm.umul.with.overflow.i16(i16, i16)
declare i32 @llvm.smax.i32(i32, i32)
declare i32 @llvm.smin.i32(i32, i32)
declare i32 @llvm.umax.i32(i32, i32)
declare i32 @llvm.umin.i32(i32, i32)
declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.uadd.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.usub.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.umul.with.overflow.i32(i32, i32)
declare double @llvm.fabs.f64(double)
declare double @llvm.sqrt.f64(double)
declare double @llvm.sin.f64(double)
declare double @llvm.cos.f64(double)
declare double @llvm.log2.f64(double)
declare double @llvm.exp2.f64(double)
declare double @llvm.rint.f64(double)
declare double @llvm.fmuladd.f64(double, double, double)
declare float @llvm.fabs.f32(float)
declare float @llvm.sqrt.f32(float)
declare float @llvm.sin.f32(float)
declare float @llvm.cos.f32(float)
declare float @llvm.log2.f32(float)
declare float @llvm.exp2.f32(float)
declare float @llvm.rint.f32(float)
declare float @llvm.fmuladd.f32(float, float, float)
declare i32 @llvm.lrint.i32.f64(double)
declare i16 @llvm.ctpop.i16(i16)
declare i32 @llvm.bswap.i32(i32)
declare i16 @llvm.abs.i16(i16, i1)
declare i16 @llvm.smul.fix.i16(i16, i16, i32)
declare void @llvm.assume(i1)
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
