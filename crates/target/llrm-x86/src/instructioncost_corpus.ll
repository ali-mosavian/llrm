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
define void @intrinsics(i8 %b, i16 %a, i32 %w, i64 %q, i128 %o, double %d, float %f, x86_fp80 %e, ptr %p, ptr addrspace(1) %p1, ptr addrspace(2) %p2, ...) {
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
  %v12 = call i8 @llvm.cttz.i8(i8 %b, i1 false)
  %v13 = call i8 @llvm.cttz.i8(i8 %b, i1 true)
  %v14 = call i8 @llvm.ctlz.i8(i8 %b, i1 false)
  %v15 = call i8 @llvm.ctlz.i8(i8 %b, i1 true)
  %v16 = call i8 @llvm.bitreverse.i8(i8 %b)
  %v17 = call i8 @llvm.sadd.sat.i8(i8 %b, i8 %b)
  %v18 = call i8 @llvm.uadd.sat.i8(i8 %b, i8 %b)
  %v19 = call i8 @llvm.ssub.sat.i8(i8 %b, i8 %b)
  %v20 = call i8 @llvm.usub.sat.i8(i8 %b, i8 %b)
  %v21 = call i8 @llvm.smul.fix.i8(i8 %b, i8 %b, i32 3)
  %v22 = call i8 @llvm.fshl.i8(i8 %b, i8 %b, i8 %b)
  %v23 = call i8 @llvm.fshr.i8(i8 %b, i8 %b, i8 %b)
  %v24 = call i16 @llvm.smax.i16(i16 %a, i16 %a)
  %v25 = call i16 @llvm.smin.i16(i16 %a, i16 %a)
  %v26 = call i16 @llvm.umax.i16(i16 %a, i16 %a)
  %v27 = call i16 @llvm.umin.i16(i16 %a, i16 %a)
  %v28 = call { i16, i1 } @llvm.sadd.with.overflow.i16(i16 %a, i16 %a)
  %v29 = call { i16, i1 } @llvm.uadd.with.overflow.i16(i16 %a, i16 %a)
  %v30 = call { i16, i1 } @llvm.ssub.with.overflow.i16(i16 %a, i16 %a)
  %v31 = call { i16, i1 } @llvm.usub.with.overflow.i16(i16 %a, i16 %a)
  %v32 = call { i16, i1 } @llvm.smul.with.overflow.i16(i16 %a, i16 %a)
  %v33 = call { i16, i1 } @llvm.umul.with.overflow.i16(i16 %a, i16 %a)
  %v34 = call i16 @llvm.ctpop.i16(i16 %a)
  %v35 = call i16 @llvm.bswap.i16(i16 %a)
  %v36 = call i16 @llvm.abs.i16(i16 %a, i1 false)
  %v37 = call i16 @llvm.cttz.i16(i16 %a, i1 false)
  %v38 = call i16 @llvm.cttz.i16(i16 %a, i1 true)
  %v39 = call i16 @llvm.ctlz.i16(i16 %a, i1 false)
  %v40 = call i16 @llvm.ctlz.i16(i16 %a, i1 true)
  %v41 = call i16 @llvm.bitreverse.i16(i16 %a)
  %v42 = call i16 @llvm.sadd.sat.i16(i16 %a, i16 %a)
  %v43 = call i16 @llvm.uadd.sat.i16(i16 %a, i16 %a)
  %v44 = call i16 @llvm.ssub.sat.i16(i16 %a, i16 %a)
  %v45 = call i16 @llvm.usub.sat.i16(i16 %a, i16 %a)
  %v46 = call i16 @llvm.smul.fix.i16(i16 %a, i16 %a, i32 3)
  %v47 = call i16 @llvm.fshl.i16(i16 %a, i16 %a, i16 %a)
  %v48 = call i16 @llvm.fshr.i16(i16 %a, i16 %a, i16 %a)
  %v49 = call i32 @llvm.smax.i32(i32 %w, i32 %w)
  %v50 = call i32 @llvm.smin.i32(i32 %w, i32 %w)
  %v51 = call i32 @llvm.umax.i32(i32 %w, i32 %w)
  %v52 = call i32 @llvm.umin.i32(i32 %w, i32 %w)
  %v53 = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %w, i32 %w)
  %v54 = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %w, i32 %w)
  %v55 = call { i32, i1 } @llvm.ssub.with.overflow.i32(i32 %w, i32 %w)
  %v56 = call { i32, i1 } @llvm.usub.with.overflow.i32(i32 %w, i32 %w)
  %v57 = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %w, i32 %w)
  %v58 = call { i32, i1 } @llvm.umul.with.overflow.i32(i32 %w, i32 %w)
  %v59 = call i32 @llvm.ctpop.i32(i32 %w)
  %v60 = call i32 @llvm.bswap.i32(i32 %w)
  %v61 = call i32 @llvm.abs.i32(i32 %w, i1 false)
  %v62 = call i32 @llvm.cttz.i32(i32 %w, i1 false)
  %v63 = call i32 @llvm.cttz.i32(i32 %w, i1 true)
  %v64 = call i32 @llvm.ctlz.i32(i32 %w, i1 false)
  %v65 = call i32 @llvm.ctlz.i32(i32 %w, i1 true)
  %v66 = call i32 @llvm.bitreverse.i32(i32 %w)
  %v67 = call i32 @llvm.sadd.sat.i32(i32 %w, i32 %w)
  %v68 = call i32 @llvm.uadd.sat.i32(i32 %w, i32 %w)
  %v69 = call i32 @llvm.ssub.sat.i32(i32 %w, i32 %w)
  %v70 = call i32 @llvm.usub.sat.i32(i32 %w, i32 %w)
  %v71 = call i32 @llvm.smul.fix.i32(i32 %w, i32 %w, i32 3)
  %v72 = call i32 @llvm.fshl.i32(i32 %w, i32 %w, i32 %w)
  %v73 = call i32 @llvm.fshr.i32(i32 %w, i32 %w, i32 %w)
  %v74 = call i64 @llvm.smax.i64(i64 %q, i64 %q)
  %v75 = call i64 @llvm.smin.i64(i64 %q, i64 %q)
  %v76 = call i64 @llvm.umax.i64(i64 %q, i64 %q)
  %v77 = call i64 @llvm.umin.i64(i64 %q, i64 %q)
  %v78 = call { i64, i1 } @llvm.sadd.with.overflow.i64(i64 %q, i64 %q)
  %v79 = call { i64, i1 } @llvm.uadd.with.overflow.i64(i64 %q, i64 %q)
  %v80 = call { i64, i1 } @llvm.ssub.with.overflow.i64(i64 %q, i64 %q)
  %v81 = call { i64, i1 } @llvm.usub.with.overflow.i64(i64 %q, i64 %q)
  %v82 = call { i64, i1 } @llvm.smul.with.overflow.i64(i64 %q, i64 %q)
  %v83 = call { i64, i1 } @llvm.umul.with.overflow.i64(i64 %q, i64 %q)
  %v84 = call i64 @llvm.ctpop.i64(i64 %q)
  %v85 = call i64 @llvm.bswap.i64(i64 %q)
  %v86 = call i64 @llvm.abs.i64(i64 %q, i1 false)
  %v87 = call i64 @llvm.cttz.i64(i64 %q, i1 false)
  %v88 = call i64 @llvm.cttz.i64(i64 %q, i1 true)
  %v89 = call i64 @llvm.ctlz.i64(i64 %q, i1 false)
  %v90 = call i64 @llvm.ctlz.i64(i64 %q, i1 true)
  %v91 = call i64 @llvm.bitreverse.i64(i64 %q)
  %v92 = call i64 @llvm.sadd.sat.i64(i64 %q, i64 %q)
  %v93 = call i64 @llvm.uadd.sat.i64(i64 %q, i64 %q)
  %v94 = call i64 @llvm.ssub.sat.i64(i64 %q, i64 %q)
  %v95 = call i64 @llvm.usub.sat.i64(i64 %q, i64 %q)
  %v96 = call i64 @llvm.smul.fix.i64(i64 %q, i64 %q, i32 3)
  %v97 = call i64 @llvm.fshl.i64(i64 %q, i64 %q, i64 %q)
  %v98 = call i64 @llvm.fshr.i64(i64 %q, i64 %q, i64 %q)
  %v99 = call i128 @llvm.smax.i128(i128 %o, i128 %o)
  %v100 = call i128 @llvm.smin.i128(i128 %o, i128 %o)
  %v101 = call i128 @llvm.umax.i128(i128 %o, i128 %o)
  %v102 = call i128 @llvm.umin.i128(i128 %o, i128 %o)
  %v103 = call { i128, i1 } @llvm.sadd.with.overflow.i128(i128 %o, i128 %o)
  %v104 = call { i128, i1 } @llvm.uadd.with.overflow.i128(i128 %o, i128 %o)
  %v105 = call { i128, i1 } @llvm.ssub.with.overflow.i128(i128 %o, i128 %o)
  %v106 = call { i128, i1 } @llvm.usub.with.overflow.i128(i128 %o, i128 %o)
  %v107 = call { i128, i1 } @llvm.smul.with.overflow.i128(i128 %o, i128 %o)
  %v108 = call { i128, i1 } @llvm.umul.with.overflow.i128(i128 %o, i128 %o)
  %v109 = call i128 @llvm.ctpop.i128(i128 %o)
  %v110 = call i128 @llvm.bswap.i128(i128 %o)
  %v111 = call i128 @llvm.abs.i128(i128 %o, i1 false)
  %v112 = call i128 @llvm.cttz.i128(i128 %o, i1 false)
  %v113 = call i128 @llvm.cttz.i128(i128 %o, i1 true)
  %v114 = call i128 @llvm.ctlz.i128(i128 %o, i1 false)
  %v115 = call i128 @llvm.ctlz.i128(i128 %o, i1 true)
  %v116 = call i128 @llvm.bitreverse.i128(i128 %o)
  %v117 = call i128 @llvm.sadd.sat.i128(i128 %o, i128 %o)
  %v118 = call i128 @llvm.uadd.sat.i128(i128 %o, i128 %o)
  %v119 = call i128 @llvm.ssub.sat.i128(i128 %o, i128 %o)
  %v120 = call i128 @llvm.usub.sat.i128(i128 %o, i128 %o)
  %v121 = call i128 @llvm.smul.fix.i128(i128 %o, i128 %o, i32 3)
  %v122 = call i128 @llvm.fshl.i128(i128 %o, i128 %o, i128 %o)
  %v123 = call i128 @llvm.fshr.i128(i128 %o, i128 %o, i128 %o)
  %v124 = call double @llvm.fabs.f64(double %d)
  %v125 = call double @llvm.sqrt.f64(double %d)
  %v126 = call double @llvm.sin.f64(double %d)
  %v127 = call double @llvm.cos.f64(double %d)
  %v128 = call double @llvm.atan.f64(double %d)
  %v129 = call double @llvm.log2.f64(double %d)
  %v130 = call double @llvm.exp2.f64(double %d)
  %v131 = call double @llvm.rint.f64(double %d)
  %v132 = call double @llvm.fmuladd.f64(double %d, double %d, double %d)
  %v133 = call i16 @llvm.lrint.i16.f64(double %d)
  %v134 = call i32 @llvm.lrint.i32.f64(double %d)
  %v135 = call float @llvm.fabs.f32(float %f)
  %v136 = call float @llvm.sqrt.f32(float %f)
  %v137 = call float @llvm.sin.f32(float %f)
  %v138 = call float @llvm.cos.f32(float %f)
  %v139 = call float @llvm.atan.f32(float %f)
  %v140 = call float @llvm.log2.f32(float %f)
  %v141 = call float @llvm.exp2.f32(float %f)
  %v142 = call float @llvm.rint.f32(float %f)
  %v143 = call float @llvm.fmuladd.f32(float %f, float %f, float %f)
  %v144 = call i16 @llvm.lrint.i16.f32(float %f)
  %v145 = call i32 @llvm.lrint.i32.f32(float %f)
  %v146 = call x86_fp80 @llvm.fabs.f80(x86_fp80 %e)
  %v147 = call x86_fp80 @llvm.sqrt.f80(x86_fp80 %e)
  %v148 = call x86_fp80 @llvm.sin.f80(x86_fp80 %e)
  %v149 = call x86_fp80 @llvm.cos.f80(x86_fp80 %e)
  %v150 = call x86_fp80 @llvm.atan.f80(x86_fp80 %e)
  %v151 = call x86_fp80 @llvm.log2.f80(x86_fp80 %e)
  %v152 = call x86_fp80 @llvm.exp2.f80(x86_fp80 %e)
  %v153 = call x86_fp80 @llvm.rint.f80(x86_fp80 %e)
  %v154 = call x86_fp80 @llvm.fmuladd.f80(x86_fp80 %e, x86_fp80 %e, x86_fp80 %e)
  %v155 = call i16 @llvm.lrint.i16.f80(x86_fp80 %e)
  %v156 = call i32 @llvm.lrint.i32.f80(x86_fp80 %e)
  call void @llvm.assume(i1 false)
  call void @llvm.prefetch.p0(ptr %p, i32 0, i32 3, i32 1)
  %v159 = call ptr @llvm.ptrmask.p0.i32(ptr %p, i32 %w)
  call void @llvm.va_start.p0(ptr %p)
  call void @llvm.va_end.p0(ptr %p)
  call void @llvm.va_copy.p0(ptr %p, ptr %p)
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
declare i8 @llvm.ctlz.i8(i8, i1)
declare i8 @llvm.bitreverse.i8(i8)
declare i8 @llvm.sadd.sat.i8(i8, i8)
declare i8 @llvm.uadd.sat.i8(i8, i8)
declare i8 @llvm.ssub.sat.i8(i8, i8)
declare i8 @llvm.usub.sat.i8(i8, i8)
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
declare i16 @llvm.ctlz.i16(i16, i1)
declare i16 @llvm.bitreverse.i16(i16)
declare i16 @llvm.sadd.sat.i16(i16, i16)
declare i16 @llvm.uadd.sat.i16(i16, i16)
declare i16 @llvm.ssub.sat.i16(i16, i16)
declare i16 @llvm.usub.sat.i16(i16, i16)
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
declare i32 @llvm.ctlz.i32(i32, i1)
declare i32 @llvm.bitreverse.i32(i32)
declare i32 @llvm.sadd.sat.i32(i32, i32)
declare i32 @llvm.uadd.sat.i32(i32, i32)
declare i32 @llvm.ssub.sat.i32(i32, i32)
declare i32 @llvm.usub.sat.i32(i32, i32)
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
declare i64 @llvm.ctlz.i64(i64, i1)
declare i64 @llvm.bitreverse.i64(i64)
declare i64 @llvm.sadd.sat.i64(i64, i64)
declare i64 @llvm.uadd.sat.i64(i64, i64)
declare i64 @llvm.ssub.sat.i64(i64, i64)
declare i64 @llvm.usub.sat.i64(i64, i64)
declare i64 @llvm.smul.fix.i64(i64, i64, i32)
declare i64 @llvm.fshl.i64(i64, i64, i64)
declare i64 @llvm.fshr.i64(i64, i64, i64)
declare i128 @llvm.smax.i128(i128, i128)
declare i128 @llvm.smin.i128(i128, i128)
declare i128 @llvm.umax.i128(i128, i128)
declare i128 @llvm.umin.i128(i128, i128)
declare { i128, i1 } @llvm.sadd.with.overflow.i128(i128, i128)
declare { i128, i1 } @llvm.uadd.with.overflow.i128(i128, i128)
declare { i128, i1 } @llvm.ssub.with.overflow.i128(i128, i128)
declare { i128, i1 } @llvm.usub.with.overflow.i128(i128, i128)
declare { i128, i1 } @llvm.smul.with.overflow.i128(i128, i128)
declare { i128, i1 } @llvm.umul.with.overflow.i128(i128, i128)
declare i128 @llvm.ctpop.i128(i128)
declare i128 @llvm.bswap.i128(i128)
declare i128 @llvm.abs.i128(i128, i1)
declare i128 @llvm.cttz.i128(i128, i1)
declare i128 @llvm.ctlz.i128(i128, i1)
declare i128 @llvm.bitreverse.i128(i128)
declare i128 @llvm.sadd.sat.i128(i128, i128)
declare i128 @llvm.uadd.sat.i128(i128, i128)
declare i128 @llvm.ssub.sat.i128(i128, i128)
declare i128 @llvm.usub.sat.i128(i128, i128)
declare i128 @llvm.smul.fix.i128(i128, i128, i32)
declare i128 @llvm.fshl.i128(i128, i128, i128)
declare i128 @llvm.fshr.i128(i128, i128, i128)
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
declare x86_fp80 @llvm.fabs.f80(x86_fp80)
declare x86_fp80 @llvm.sqrt.f80(x86_fp80)
declare x86_fp80 @llvm.sin.f80(x86_fp80)
declare x86_fp80 @llvm.cos.f80(x86_fp80)
declare x86_fp80 @llvm.atan.f80(x86_fp80)
declare x86_fp80 @llvm.log2.f80(x86_fp80)
declare x86_fp80 @llvm.exp2.f80(x86_fp80)
declare x86_fp80 @llvm.rint.f80(x86_fp80)
declare x86_fp80 @llvm.fmuladd.f80(x86_fp80, x86_fp80, x86_fp80)
declare i16 @llvm.lrint.i16.f80(x86_fp80)
declare i32 @llvm.lrint.i32.f80(x86_fp80)
declare void @llvm.assume(i1)
declare void @llvm.prefetch.p0(ptr, i32, i32, i32)
declare ptr @llvm.ptrmask.p0.i32(ptr, i32)
declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
declare void @llvm.va_copy.p0(ptr, ptr)
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
