declare i32 @llvm.abs.i32(i32, i1)
declare i32 @llvm.bswap.i32(i32)
declare i32 @llvm.bitreverse.i32(i32)
declare i32 @llvm.ctpop.i32(i32)
declare i32 @llvm.umax.i32(i32, i32)
declare i32 @llvm.umin.i32(i32, i32)
declare i32 @llvm.smax.i32(i32, i32)
declare i32 @llvm.smin.i32(i32, i32)
declare i32 @llvm.uadd.sat.i32(i32, i32)
declare i32 @llvm.sadd.sat.i32(i32, i32)
declare i32 @llvm.ssub.sat.i32(i32, i32)
declare i32 @llvm.sshl.sat.i32(i32, i32)
declare i32 @llvm.fshl.i32(i32, i32, i32)
declare i32 @llvm.fshr.i32(i32, i32, i32)
declare i32 @llvm.cttz.i32(i32, i1)
declare i32 @llvm.ctlz.i32(i32, i1)
declare {i32, i1} @llvm.uadd.with.overflow.i32(i32, i32)
declare {i32, i1} @llvm.usub.with.overflow.i32(i32, i32)
declare {i32, i1} @llvm.umul.with.overflow.i32(i32, i32)
declare i64 @llvm.vscale.i64()
declare i32 @ranged() 
declare range(i32 1, 10) i32 @ranged2()
declare i32 @id32(i32 returned)

define void @args(i32 range(i32 1, 0) %a, i32 range(i32 0, 5) %b, i32 range(i32 -3, 0) %c, i32 %d) {
  %r = call i32 @ranged(), !range !0
  %r2 = call i32 @ranged2()
  %r3 = call i32 @id32(i32 %a)
  %r4 = call i32 @id32(i32 %d)
  %l = load i32, ptr null, !range !0
  %l2 = load i32, ptr null, !range !1
  %l3 = load i32, ptr null, !range !2
  ret void
}

define void @casts(i32 %x, i64 %y, i8 range(i8 1, 0) %nz8) {
  %nzx = or i32 %x, 4
  %z = zext i8 %nz8 to i32
  %s = sext i8 %nz8 to i32
  %z2 = zext i32 %nzx to i64
  %t = trunc i64 %y to i32
  %t2 = trunc nuw i64 %y to i32
  %t3 = trunc nsw i64 %y to i32
  %t4 = trunc i64 %z2 to i32
  %f = freeze i32 %nzx
  %f2 = freeze i32 %x
  %p = inttoptr i32 %nzx to ptr
  %p2 = inttoptr i32 %x to ptr
  %p3 = inttoptr i32 %z to ptr
  %i = ptrtoint ptr %p to i32
  %i2 = ptrtoint ptr %p to i64
  ret void
}

define void @arith(i32 %x, i32 %y, i32 range(i32 1, 0) %nz) {
  %a1 = add nuw i32 %x, %nz
  %a2 = add i32 %x, %nz
  %a3 = add nsw i32 %nz, %y
  %a4 = add nuw i32 %x, %y
  %m1 = mul nuw i32 %nz, %nz
  %m2 = mul nsw i32 %nz, %x
  %m3 = mul i32 %nz, %nz
  %s1 = shl nuw i32 %nz, %x
  %s2 = shl nsw i32 %nz, %x
  %s3 = shl i32 %nz, %x
  %s4 = lshr exact i32 %nz, %x
  %s5 = ashr exact i32 %nz, %x
  %s6 = lshr i32 %nz, %x
  %d1 = udiv exact i32 %nz, %nz
  %d2 = sdiv exact i32 %x, %nz
  %d3 = udiv i32 %nz, %nz
  %o1 = or i32 %nz, %x
  %o2 = or i32 %x, %nz
  %o3 = or i32 %x, %y
  %q = sub i32 %x, %nz
  %q1 = sub i32 0, %nz
  %q2 = sub i32 %x, %y
  %xo = xor i32 %x, %nz
  %xo2 = xor i32 %x, %y
  %u = urem i32 %x, %nz
  ret void
}

define void @eqzero(i32 %x, i32 %y) {
  %c = icmp eq i32 %x, 0
  %z = zext i1 %c to i32
  %s = sext i1 %c to i32
  %a = add i32 %x, %z
  %b = add i32 %s, %x
  %d = sub i32 %x, %z
  %e = sub i32 %z, %x
  %f = or i32 %x, %z
  %g = or i32 %s, %x
  %h = call i32 @llvm.umax.i32(i32 %x, i32 %z)
  %i = call i32 @llvm.uadd.sat.i32(i32 %z, i32 %x)
  %c2 = icmp eq i32 %y, 0
  %z2 = zext i1 %c2 to i32
  %j = add i32 %x, %z2
  ret void
}

define void @intrinsics(i32 %x, i32 %y, i32 range(i32 1, 0) %nz, i32 range(i32 1, 100) %pos) {
  %a = call i32 @llvm.abs.i32(i32 %nz, i1 false)
  %b = call i32 @llvm.bswap.i32(i32 %nz)
  %c = call i32 @llvm.bitreverse.i32(i32 %nz)
  %d = call i32 @llvm.ctpop.i32(i32 %nz)
  %e = call i32 @llvm.umax.i32(i32 %nz, i32 %x)
  %f = call i32 @llvm.umin.i32(i32 %nz, i32 %y)
  %f2 = call i32 @llvm.umin.i32(i32 %nz, i32 %nz)
  %g = call i32 @llvm.smax.i32(i32 %pos, i32 %x)
  %g2 = call i32 @llvm.smax.i32(i32 %nz, i32 %nz)
  %g3 = call i32 @llvm.smax.i32(i32 %nz, i32 %x)
  %h = call i32 @llvm.smin.i32(i32 %nz, i32 %nz)
  %h2 = call i32 @llvm.smin.i32(i32 %x, i32 %nz)
  %i = call i32 @llvm.uadd.sat.i32(i32 %nz, i32 %x)
  %j = call i32 @llvm.sadd.sat.i32(i32 %pos, i32 %pos)
  %k = call i32 @llvm.ssub.sat.i32(i32 %nz, i32 %x)
  %k2 = call i32 @llvm.ssub.sat.i32(i32 %x, i32 %y)
  %l = call i32 @llvm.sshl.sat.i32(i32 %nz, i32 %x)
  %m = call i32 @llvm.fshl.i32(i32 %nz, i32 %nz, i32 %x)
  %m2 = call i32 @llvm.fshl.i32(i32 %nz, i32 %x, i32 %x)
  %n = call i32 @llvm.fshr.i32(i32 %nz, i32 %nz, i32 %x)
  %o = call i64 @llvm.vscale.i64()
  %p = call i32 @llvm.cttz.i32(i32 %x, i1 false)
  %q = call i32 @llvm.ctlz.i32(i32 %x, i1 false)
  %w = call {i32, i1} @llvm.uadd.with.overflow.i32(i32 %nz, i32 %x)
  %w0 = extractvalue {i32, i1} %w, 0
  %w1 = extractvalue {i32, i1} %w, 1
  %v = call {i32, i1} @llvm.usub.with.overflow.i32(i32 %x, i32 %nz)
  %v0 = extractvalue {i32, i1} %v, 0
  %u = call {i32, i1} @llvm.umul.with.overflow.i32(i32 %nz, i32 %nz)
  %u0 = extractvalue {i32, i1} %u, 0
  ret void
}

define void @selects(i1 %c, i32 %x, i32 range(i32 1, 0) %nz, i32 range(i32 1, 0) %nz2) {
  %s1 = select i1 %c, i32 %nz, i32 %nz2
  %s2 = select i1 %c, i32 %nz, i32 %x
  %s3 = select i1 %c, i32 4, i32 7
  %s4 = select i1 %c, i32 4, i32 0
  %ne = icmp ne i32 %x, 0
  %s5 = select i1 %ne, i32 %x, i32 %nz
  %s6 = select i1 %ne, i32 %nz, i32 %x
  %eq = icmp eq i32 %x, 0
  %s7 = select i1 %eq, i32 %nz, i32 %x
  %ugt = icmp ugt i32 %x, 5
  %s8 = select i1 %ugt, i32 %x, i32 %nz
  %ult = icmp ult i32 5, %x
  %s9 = select i1 %ult, i32 %x, i32 %nz
  ret void
}

define void @phis(i1 %c, i32 %x, i32 range(i32 1, 0) %nz, i32 %n) {
entry:
  br i1 %c, label %a, label %b
a:
  br label %m
b:
  br label %m
m:
  %p1 = phi i32 [ %nz, %a ], [ 3, %b ]
  %p2 = phi i32 [ %nz, %a ], [ %x, %b ]
  %p3 = phi i32 [ 1, %a ], [ %p3n, %m2 ]
  %p3n = add nuw i32 %p3, 2
  br label %m2
m2:
  %i1 = phi i32 [ 1, %m ], [ %i1n, %m2 ]
  %i1n = add nuw i32 %i1, %n
  %i2 = phi i32 [ 5, %m ], [ %i2n, %m2 ]
  %i2n = add nsw i32 %i2, 3
  %i3 = phi i32 [ -5, %m ], [ %i3n, %m2 ]
  %i3n = add nsw i32 %i3, 3
  %i4 = phi i32 [ 5, %m ], [ %i4n, %m2 ]
  %i4n = mul nuw i32 %i4, 3
  %i5 = phi i32 [ 5, %m ], [ %i5n, %m2 ]
  %i5n = shl nuw i32 %i5, %n
  %i6 = phi i32 [ 5, %m ], [ %i6n, %m2 ]
  %i6n = lshr exact i32 %i6, %n
  %i7 = phi i32 [ 5, %m ], [ %i7n, %m2 ]
  %i7n = or i32 %i7, %n
  %i8 = phi i32 [ 5, %m ], [ %i8n, %m2 ]
  %i8n = sub i32 %i8, %n
  %i9 = phi i32 [ 0, %m ], [ %i9n, %m2 ]
  %i9n = add nuw i32 %i9, 1
  %c2 = icmp ne i32 %n, 0
  br i1 %c2, label %m2, label %exit
exit:
  ret void
}

define void @phibr(i32 %x, i32 %y) {
entry:
  %c = icmp ne i32 %x, 0
  br i1 %c, label %t, label %f
t:
  br label %j
f:
  %c2 = icmp eq i32 %y, 0
  br i1 %c2, label %j, label %j2
j2:
  br label %j
j:
  %p = phi i32 [ %x, %t ], [ %y, %f ], [ 3, %j2 ]
  %q = phi i32 [ %x, %t ], [ 1, %f ], [ 3, %j2 ]
  ret void
}

define void @same(i32 %x, i32 %y, i32 range(i32 1, 0) %nz) {
  %a = add i32 %x, %nz
  %b = add i32 %x, %y
  %c = sub i32 %a, %x
  %d = sub i32 %a, %b
  %e = xor i32 %a, %x
  %f = or i32 %a, %x
  %g = mul nuw i32 %x, 3
  %h = sub i32 %g, %x
  %i = shl nuw i32 %x, 2
  %j = sub i32 %i, %x
  %k = zext i32 %a to i64
  %l = zext i32 %x to i64
  %m = sub i64 %k, %l
  %n = add i32 %x, 1
  %o = sub i32 %n, %x
  ret void
}

!0 = !{i32 1, i32 0}
!1 = !{i32 0, i32 5}
!2 = !{i32 1, i32 3, i32 5, i32 7}
