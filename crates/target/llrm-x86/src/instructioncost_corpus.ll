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
define void @casts(i16 %a, i32 %w, double %d, float %f, ptr %p, ptr %q, ptr addrspace(2) %p2) {
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
  %c11 = inttoptr i16 %a to ptr addrspace(2)
  %c12 = addrspacecast ptr addrspace(2) %p2 to ptr
  %c13 = ptrtoint ptr %q to i16
  %c14 = ptrtoint ptr %q to i64
  %k1 = load i16, ptr %q
  %c15 = sitofp i16 %k1 to float
  %k2 = load i8, ptr %q
  %c16 = uitofp i8 %k2 to double
  %k3 = load i32, ptr %q
  %c17 = sitofp i32 %k3 to double
  %l1 = load i16, ptr %q
  %c18 = sext i16 %l1 to i32
  %l2 = load i16, ptr %q
  %c19 = zext i16 %l2 to i32
  %l3 = load i8, ptr %q
  %c20 = sext i8 %l3 to i16
  %l4 = load i8, ptr %q
  %c21 = zext i8 %l4 to i32
  %l5 = load i32, ptr %q
  %c22 = zext i32 %l5 to i64
  %l6 = load i16, ptr %q
  %c23 = sext i16 %l6 to i64
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
  %gi = getelementptr i32, ptr %p, i32 %w
  store i32 1, ptr %gi
  %gc = getelementptr i32, ptr %p, i32 2
  store i32 1, ptr %gc
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
define void @intrinsics(i8 %b, i16 %a, i32 %w, i64 %q, double %d, float %f, ptr %p, ptr addrspace(1) %p1, ptr addrspace(2) %p2) {
  %v0 = call i8 @llvm.smax.i8(i8 %b, i8 %b)
  %v1 = call i8 @llvm.smin.i8(i8 %b, i8 %b)
  %v2 = call i8 @llvm.umax.i8(i8 %b, i8 %b)
  %v3 = call i8 @llvm.umin.i8(i8 %b, i8 %b)
  %v4 = call { i8, i1 } @llvm.sadd.with.overflow.i8(i8 %b, i8 %b)
  %v5 = call { i8, i1 } @llvm.uadd.with.overflow.i8(i8 %b, i8 %b)
  %v6 = call { i8, i1 } @llvm.ssub.with.overflow.i8(i8 %b, i8 %b)
  %v7 = call { i8, i1 } @llvm.usub.with.overflow.i8(i8 %b, i8 %b)
  %v8 = call { i8, i1 } @llvm.smul.with.overflow.i8(i8 %b, i8 %b)
  %v9 = call { i8, i1 } @llvm.umul.with.overflow.i8(i8 %b, i8 %b)
  %v10 = call i8 @llvm.ctpop.i8(i8 %b)
  %v11 = call i8 @llvm.abs.i8(i8 %b, i1 false)
  %v12 = call i8 @llvm.cttz.i8(i8 %b, i1 true)
  %v13 = call i8 @llvm.smul.fix.i8(i8 %b, i8 %b, i32 3)
  %v14 = call i8 @llvm.fshl.i8(i8 %b, i8 %b, i8 %b)
  %v15 = call i8 @llvm.fshr.i8(i8 %b, i8 %b, i8 %b)
  %v16 = call i16 @llvm.smax.i16(i16 %a, i16 %a)
  %v17 = call i16 @llvm.smin.i16(i16 %a, i16 %a)
  %v18 = call i16 @llvm.umax.i16(i16 %a, i16 %a)
  %v19 = call i16 @llvm.umin.i16(i16 %a, i16 %a)
  %v20 = call { i16, i1 } @llvm.sadd.with.overflow.i16(i16 %a, i16 %a)
  %v21 = call { i16, i1 } @llvm.uadd.with.overflow.i16(i16 %a, i16 %a)
  %v22 = call { i16, i1 } @llvm.ssub.with.overflow.i16(i16 %a, i16 %a)
  %v23 = call { i16, i1 } @llvm.usub.with.overflow.i16(i16 %a, i16 %a)
  %v24 = call { i16, i1 } @llvm.smul.with.overflow.i16(i16 %a, i16 %a)
  %v25 = call { i16, i1 } @llvm.umul.with.overflow.i16(i16 %a, i16 %a)
  %v26 = call i16 @llvm.ctpop.i16(i16 %a)
  %v27 = call i16 @llvm.bswap.i16(i16 %a)
  %v28 = call i16 @llvm.abs.i16(i16 %a, i1 false)
  %v29 = call i16 @llvm.cttz.i16(i16 %a, i1 true)
  %v30 = call i16 @llvm.smul.fix.i16(i16 %a, i16 %a, i32 3)
  %v31 = call i16 @llvm.fshl.i16(i16 %a, i16 %a, i16 %a)
  %v32 = call i16 @llvm.fshr.i16(i16 %a, i16 %a, i16 %a)
  %v33 = call i32 @llvm.smax.i32(i32 %w, i32 %w)
  %v34 = call i32 @llvm.smin.i32(i32 %w, i32 %w)
  %v35 = call i32 @llvm.umax.i32(i32 %w, i32 %w)
  %v36 = call i32 @llvm.umin.i32(i32 %w, i32 %w)
  %v37 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %w, i32 %w)
  %v38 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %w, i32 %w)
  %v39 = call { i32, i1 } @llvm.ssub.with.overflow.i32(i32 %w, i32 %w)
  %v40 = call { i32, i1 } @llvm.usub.with.overflow.i32(i32 %w, i32 %w)
  %v41 = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %w, i32 %w)
  %v42 = call { i32, i1 } @llvm.umul.with.overflow.i32(i32 %w, i32 %w)
  %v43 = call i32 @llvm.ctpop.i32(i32 %w)
  %v44 = call i32 @llvm.bswap.i32(i32 %w)
  %v45 = call i32 @llvm.abs.i32(i32 %w, i1 false)
  %v46 = call i32 @llvm.cttz.i32(i32 %w, i1 true)
  %v47 = call i32 @llvm.smul.fix.i32(i32 %w, i32 %w, i32 3)
  %v48 = call i32 @llvm.fshl.i32(i32 %w, i32 %w, i32 %w)
  %v49 = call i32 @llvm.fshr.i32(i32 %w, i32 %w, i32 %w)
  %v50 = call i64 @llvm.smax.i64(i64 %q, i64 %q)
  %v51 = call i64 @llvm.smin.i64(i64 %q, i64 %q)
  %v52 = call i64 @llvm.umax.i64(i64 %q, i64 %q)
  %v53 = call i64 @llvm.umin.i64(i64 %q, i64 %q)
  %v54 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %q, i64 %q)
  %v55 = call { i64, i1 } @llvm.uadd.with.overflow.i64(i64 %q, i64 %q)
  %v56 = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %q, i64 %q)
  %v57 = call { i64, i1 } @llvm.usub.with.overflow.i64(i64 %q, i64 %q)
  %v58 = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %q, i64 %q)
  %v59 = call { i64, i1 } @llvm.umul.with.overflow.i64(i64 %q, i64 %q)
  %v60 = call i64 @llvm.ctpop.i64(i64 %q)
  %v61 = call i64 @llvm.bswap.i64(i64 %q)
  %v62 = call i64 @llvm.abs.i64(i64 %q, i1 false)
  %v63 = call i64 @llvm.cttz.i64(i64 %q, i1 true)
  %v64 = call i64 @llvm.smul.fix.i64(i64 %q, i64 %q, i32 3)
  %v65 = call i64 @llvm.fshl.i64(i64 %q, i64 %q, i64 %q)
  %v66 = call i64 @llvm.fshr.i64(i64 %q, i64 %q, i64 %q)
  %v67 = call double @llvm.fabs.f64(double %d)
  %v68 = call double @llvm.sqrt.f64(double %d)
  %v69 = call double @llvm.sin.f64(double %d)
  %v70 = call double @llvm.cos.f64(double %d)
  %v71 = call double @llvm.atan.f64(double %d)
  %v72 = call double @llvm.log2.f64(double %d)
  %v73 = call double @llvm.exp2.f64(double %d)
  %v74 = call double @llvm.rint.f64(double %d)
  %v75 = call double @llvm.fmuladd.f64(double %d, double %d, double %d)
  %v76 = call i16 @llvm.lrint.i16.f64(double %d)
  %v77 = call i32 @llvm.lrint.i32.f64(double %d)
  %v78 = call float @llvm.fabs.f32(float %f)
  %v79 = call float @llvm.sqrt.f32(float %f)
  %v80 = call float @llvm.sin.f32(float %f)
  %v81 = call float @llvm.cos.f32(float %f)
  %v82 = call float @llvm.atan.f32(float %f)
  %v83 = call float @llvm.log2.f32(float %f)
  %v84 = call float @llvm.exp2.f32(float %f)
  %v85 = call float @llvm.rint.f32(float %f)
  %v86 = call float @llvm.fmuladd.f32(float %f, float %f, float %f)
  %v87 = call i16 @llvm.lrint.i16.f32(float %f)
  %v88 = call i32 @llvm.lrint.i32.f32(float %f)
  call void @llvm.assume(i1 false)
  call void @llvm.memset.p0.i16(ptr %p, i8 %b, i16 %a, i1 false)
  call void @llvm.memcpy.p0.p0.i16(ptr %p, ptr %p, i16 %a, i1 false)
  call void @llvm.memmove.p0.p0.i16(ptr %p, ptr %p, i16 %a, i1 false)
  call void @llvm.memcpy.p0.p1.i16(ptr %p, ptr addrspace(1) %p1, i16 %a, i1 false)
  call void @llvm.memmove.p0.p1.i16(ptr %p, ptr addrspace(1) %p1, i16 %a, i1 false)
  call void @llvm.memcpy.p0.p2.i16(ptr %p, ptr addrspace(2) %p2, i16 %a, i1 false)
  call void @llvm.memmove.p0.p2.i16(ptr %p, ptr addrspace(2) %p2, i16 %a, i1 false)
  call void @llvm.memset.p0.i32(ptr %p, i8 %b, i32 %w, i1 false)
  call void @llvm.memcpy.p0.p0.i32(ptr %p, ptr %p, i32 %w, i1 false)
  call void @llvm.memmove.p0.p0.i32(ptr %p, ptr %p, i32 %w, i1 false)
  call void @llvm.memcpy.p0.p1.i32(ptr %p, ptr addrspace(1) %p1, i32 %w, i1 false)
  call void @llvm.memmove.p0.p1.i32(ptr %p, ptr addrspace(1) %p1, i32 %w, i1 false)
  call void @llvm.memcpy.p0.p2.i32(ptr %p, ptr addrspace(2) %p2, i32 %w, i1 false)
  call void @llvm.memmove.p0.p2.i32(ptr %p, ptr addrspace(2) %p2, i32 %w, i1 false)
  call void @llvm.memset.p1.i16(ptr addrspace(1) %p1, i8 %b, i16 %a, i1 false)
  call void @llvm.memcpy.p1.p0.i16(ptr addrspace(1) %p1, ptr %p, i16 %a, i1 false)
  call void @llvm.memmove.p1.p0.i16(ptr addrspace(1) %p1, ptr %p, i16 %a, i1 false)
  call void @llvm.memcpy.p1.p1.i16(ptr addrspace(1) %p1, ptr addrspace(1) %p1, i16 %a, i1 false)
  call void @llvm.memmove.p1.p1.i16(ptr addrspace(1) %p1, ptr addrspace(1) %p1, i16 %a, i1 false)
  call void @llvm.memcpy.p1.p2.i16(ptr addrspace(1) %p1, ptr addrspace(2) %p2, i16 %a, i1 false)
  call void @llvm.memmove.p1.p2.i16(ptr addrspace(1) %p1, ptr addrspace(2) %p2, i16 %a, i1 false)
  call void @llvm.memset.p1.i32(ptr addrspace(1) %p1, i8 %b, i32 %w, i1 false)
  call void @llvm.memcpy.p1.p0.i32(ptr addrspace(1) %p1, ptr %p, i32 %w, i1 false)
  call void @llvm.memmove.p1.p0.i32(ptr addrspace(1) %p1, ptr %p, i32 %w, i1 false)
  call void @llvm.memcpy.p1.p1.i32(ptr addrspace(1) %p1, ptr addrspace(1) %p1, i32 %w, i1 false)
  call void @llvm.memmove.p1.p1.i32(ptr addrspace(1) %p1, ptr addrspace(1) %p1, i32 %w, i1 false)
  call void @llvm.memcpy.p1.p2.i32(ptr addrspace(1) %p1, ptr addrspace(2) %p2, i32 %w, i1 false)
  call void @llvm.memmove.p1.p2.i32(ptr addrspace(1) %p1, ptr addrspace(2) %p2, i32 %w, i1 false)
  call void @llvm.memset.p2.i16(ptr addrspace(2) %p2, i8 %b, i16 %a, i1 false)
  call void @llvm.memcpy.p2.p0.i16(ptr addrspace(2) %p2, ptr %p, i16 %a, i1 false)
  call void @llvm.memmove.p2.p0.i16(ptr addrspace(2) %p2, ptr %p, i16 %a, i1 false)
  call void @llvm.memcpy.p2.p1.i16(ptr addrspace(2) %p2, ptr addrspace(1) %p1, i16 %a, i1 false)
  call void @llvm.memmove.p2.p1.i16(ptr addrspace(2) %p2, ptr addrspace(1) %p1, i16 %a, i1 false)
  call void @llvm.memcpy.p2.p2.i16(ptr addrspace(2) %p2, ptr addrspace(2) %p2, i16 %a, i1 false)
  call void @llvm.memmove.p2.p2.i16(ptr addrspace(2) %p2, ptr addrspace(2) %p2, i16 %a, i1 false)
  call void @llvm.memset.p2.i32(ptr addrspace(2) %p2, i8 %b, i32 %w, i1 false)
  call void @llvm.memcpy.p2.p0.i32(ptr addrspace(2) %p2, ptr %p, i32 %w, i1 false)
  call void @llvm.memmove.p2.p0.i32(ptr addrspace(2) %p2, ptr %p, i32 %w, i1 false)
  call void @llvm.memcpy.p2.p1.i32(ptr addrspace(2) %p2, ptr addrspace(1) %p1, i32 %w, i1 false)
  call void @llvm.memmove.p2.p1.i32(ptr addrspace(2) %p2, ptr addrspace(1) %p1, i32 %w, i1 false)
  call void @llvm.memcpy.p2.p2.i32(ptr addrspace(2) %p2, ptr addrspace(2) %p2, i32 %w, i1 false)
  call void @llvm.memmove.p2.p2.i32(ptr addrspace(2) %p2, ptr addrspace(2) %p2, i32 %w, i1 false)
  ret void
}
declare i8 @llvm.smax.i8(i8, i8)
declare i8 @llvm.smin.i8(i8, i8)
declare i8 @llvm.umax.i8(i8, i8)
declare i8 @llvm.umin.i8(i8, i8)
declare { i8, i1 } @llvm.sadd.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.uadd.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.ssub.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.usub.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.smul.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.umul.with.overflow.i8(i8, i8)
declare i8 @llvm.ctpop.i8(i8)
declare i8 @llvm.abs.i8(i8, i1)
declare i8 @llvm.cttz.i8(i8, i1)
declare i8 @llvm.smul.fix.i8(i8, i8, i32)
declare i8 @llvm.fshl.i8(i8, i8, i8)
declare i8 @llvm.fshr.i8(i8, i8, i8)
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
declare i16 @llvm.ctpop.i16(i16)
declare i16 @llvm.bswap.i16(i16)
declare i16 @llvm.abs.i16(i16, i1)
declare i16 @llvm.cttz.i16(i16, i1)
declare i16 @llvm.smul.fix.i16(i16, i16, i32)
declare i16 @llvm.fshl.i16(i16, i16, i16)
declare i16 @llvm.fshr.i16(i16, i16, i16)
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
declare i32 @llvm.ctpop.i32(i32)
declare i32 @llvm.bswap.i32(i32)
declare i32 @llvm.abs.i32(i32, i1)
declare i32 @llvm.cttz.i32(i32, i1)
declare i32 @llvm.smul.fix.i32(i32, i32, i32)
declare i32 @llvm.fshl.i32(i32, i32, i32)
declare i32 @llvm.fshr.i32(i32, i32, i32)
declare i64 @llvm.smax.i64(i64, i64)
declare i64 @llvm.smin.i64(i64, i64)
declare i64 @llvm.umax.i64(i64, i64)
declare i64 @llvm.umin.i64(i64, i64)
declare { i64, i1 } @llvm.sadd.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.uadd.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.ssub.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.usub.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.smul.with.overflow.i64(i64, i64)
declare { i64, i1 } @llvm.umul.with.overflow.i64(i64, i64)
declare i64 @llvm.ctpop.i64(i64)
declare i64 @llvm.bswap.i64(i64)
declare i64 @llvm.abs.i64(i64, i1)
declare i64 @llvm.cttz.i64(i64, i1)
declare i64 @llvm.smul.fix.i64(i64, i64, i32)
declare i64 @llvm.fshl.i64(i64, i64, i64)
declare i64 @llvm.fshr.i64(i64, i64, i64)
declare double @llvm.fabs.f64(double)
declare double @llvm.sqrt.f64(double)
declare double @llvm.sin.f64(double)
declare double @llvm.cos.f64(double)
declare double @llvm.atan.f64(double)
declare double @llvm.log2.f64(double)
declare double @llvm.exp2.f64(double)
declare double @llvm.rint.f64(double)
declare double @llvm.fmuladd.f64(double, double, double)
declare i16 @llvm.lrint.i16.f64(double)
declare i32 @llvm.lrint.i32.f64(double)
declare float @llvm.fabs.f32(float)
declare float @llvm.sqrt.f32(float)
declare float @llvm.sin.f32(float)
declare float @llvm.cos.f32(float)
declare float @llvm.atan.f32(float)
declare float @llvm.log2.f32(float)
declare float @llvm.exp2.f32(float)
declare float @llvm.rint.f32(float)
declare float @llvm.fmuladd.f32(float, float, float)
declare i16 @llvm.lrint.i16.f32(float)
declare i32 @llvm.lrint.i32.f32(float)
declare void @llvm.assume(i1)
declare void @llvm.memset.p0.i16(ptr, i8, i16, i1)
declare void @llvm.memcpy.p0.p0.i16(ptr, ptr, i16, i1)
declare void @llvm.memmove.p0.p0.i16(ptr, ptr, i16, i1)
declare void @llvm.memcpy.p0.p1.i16(ptr, ptr addrspace(1), i16, i1)
declare void @llvm.memmove.p0.p1.i16(ptr, ptr addrspace(1), i16, i1)
declare void @llvm.memcpy.p0.p2.i16(ptr, ptr addrspace(2), i16, i1)
declare void @llvm.memmove.p0.p2.i16(ptr, ptr addrspace(2), i16, i1)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memcpy.p0.p1.i32(ptr, ptr addrspace(1), i32, i1)
declare void @llvm.memmove.p0.p1.i32(ptr, ptr addrspace(1), i32, i1)
declare void @llvm.memcpy.p0.p2.i32(ptr, ptr addrspace(2), i32, i1)
declare void @llvm.memmove.p0.p2.i32(ptr, ptr addrspace(2), i32, i1)
declare void @llvm.memset.p1.i16(ptr addrspace(1), i8, i16, i1)
declare void @llvm.memcpy.p1.p0.i16(ptr addrspace(1), ptr, i16, i1)
declare void @llvm.memmove.p1.p0.i16(ptr addrspace(1), ptr, i16, i1)
declare void @llvm.memcpy.p1.p1.i16(ptr addrspace(1), ptr addrspace(1), i16, i1)
declare void @llvm.memmove.p1.p1.i16(ptr addrspace(1), ptr addrspace(1), i16, i1)
declare void @llvm.memcpy.p1.p2.i16(ptr addrspace(1), ptr addrspace(2), i16, i1)
declare void @llvm.memmove.p1.p2.i16(ptr addrspace(1), ptr addrspace(2), i16, i1)
declare void @llvm.memset.p1.i32(ptr addrspace(1), i8, i32, i1)
declare void @llvm.memcpy.p1.p0.i32(ptr addrspace(1), ptr, i32, i1)
declare void @llvm.memmove.p1.p0.i32(ptr addrspace(1), ptr, i32, i1)
declare void @llvm.memcpy.p1.p1.i32(ptr addrspace(1), ptr addrspace(1), i32, i1)
declare void @llvm.memmove.p1.p1.i32(ptr addrspace(1), ptr addrspace(1), i32, i1)
declare void @llvm.memcpy.p1.p2.i32(ptr addrspace(1), ptr addrspace(2), i32, i1)
declare void @llvm.memmove.p1.p2.i32(ptr addrspace(1), ptr addrspace(2), i32, i1)
declare void @llvm.memset.p2.i16(ptr addrspace(2), i8, i16, i1)
declare void @llvm.memcpy.p2.p0.i16(ptr addrspace(2), ptr, i16, i1)
declare void @llvm.memmove.p2.p0.i16(ptr addrspace(2), ptr, i16, i1)
declare void @llvm.memcpy.p2.p1.i16(ptr addrspace(2), ptr addrspace(1), i16, i1)
declare void @llvm.memmove.p2.p1.i16(ptr addrspace(2), ptr addrspace(1), i16, i1)
declare void @llvm.memcpy.p2.p2.i16(ptr addrspace(2), ptr addrspace(2), i16, i1)
declare void @llvm.memmove.p2.p2.i16(ptr addrspace(2), ptr addrspace(2), i16, i1)
declare void @llvm.memset.p2.i32(ptr addrspace(2), i8, i32, i1)
declare void @llvm.memcpy.p2.p0.i32(ptr addrspace(2), ptr, i32, i1)
declare void @llvm.memmove.p2.p0.i32(ptr addrspace(2), ptr, i32, i1)
declare void @llvm.memcpy.p2.p1.i32(ptr addrspace(2), ptr addrspace(1), i32, i1)
declare void @llvm.memmove.p2.p1.i32(ptr addrspace(2), ptr addrspace(1), i32, i1)
declare void @llvm.memcpy.p2.p2.i32(ptr addrspace(2), ptr addrspace(2), i32, i1)
declare void @llvm.memmove.p2.p2.i32(ptr addrspace(2), ptr addrspace(2), i32, i1)
