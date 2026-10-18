declare i32 @llvm.abs.i32(i32, i1)
declare i32 @llvm.bitreverse.i32(i32)
declare i32 @llvm.bswap.i32(i32)
declare i32 @llvm.ctlz.i32(i32, i1)
declare i32 @llvm.cttz.i32(i32, i1)
declare i32 @llvm.ctpop.i32(i32)
declare i32 @llvm.fshl.i32(i32, i32, i32)
declare i32 @llvm.fshr.i32(i32, i32, i32)
declare i32 @llvm.clmul.i32(i32, i32)
declare i32 @llvm.pext.i32(i32, i32)
declare i32 @llvm.pdep.i32(i32, i32)
declare i32 @llvm.smulh.i32(i32, i32)
declare i32 @llvm.umulh.i32(i32, i32)
declare i32 @llvm.uadd.sat.i32(i32, i32)
declare i32 @llvm.usub.sat.i32(i32, i32)
declare i32 @llvm.sadd.sat.i32(i32, i32)
declare i32 @llvm.ssub.sat.i32(i32, i32)
declare i32 @llvm.umin.i32(i32, i32)
declare i32 @llvm.umax.i32(i32, i32)
declare i32 @llvm.smin.i32(i32, i32)
declare i32 @llvm.smax.i32(i32, i32)
declare ptr @llvm.ptrmask.p0.i64(ptr, i64)
declare {i32, i1} @llvm.uadd.with.overflow.i32(i32, i32)
declare {i32, i1} @llvm.sadd.with.overflow.i32(i32, i32)
declare {i32, i1} @llvm.usub.with.overflow.i32(i32, i32)
declare {i32, i1} @llvm.ssub.with.overflow.i32(i32, i32)
declare {i32, i1} @llvm.umul.with.overflow.i32(i32, i32)
declare {i32, i1} @llvm.smul.with.overflow.i32(i32, i32)
declare i64 @llvm.x86.sse42.crc32.64.64(i64, i64)
declare i32 @llvm.amdgcn.mbcnt.lo(i32, i32)
declare i32 @llvm.amdgcn.mbcnt.hi(i32, i32)
declare i64 @llvm.vscale.i64()
declare i64 @llvm.riscv.vsetvli.i64(i64, i64, i64)
declare i64 @llvm.riscv.vsetvlimax.i64(i64, i64)

define i32 @unary(i32 %x, i32 %y) {
  %a = and i32 %x, 255
  %o = or i32 %y, 3
  %n = or i32 %y, -2147483648
  %ab1 = call i32 @llvm.abs.i32(i32 %n, i1 true)
  %ab2 = call i32 @llvm.abs.i32(i32 %n, i1 false)
  %ab3 = call i32 @llvm.abs.i32(i32 %a, i1 false)
  %ab4 = call i32 @llvm.abs.i32(i32 %x, i1 true)
  %br = call i32 @llvm.bitreverse.i32(i32 %a)
  %bs = call i32 @llvm.bswap.i32(i32 %a)
  %lz = call i32 @llvm.ctlz.i32(i32 %a, i1 false)
  %lz2 = call i32 @llvm.ctlz.i32(i32 %x, i1 true)
  %tz = call i32 @llvm.cttz.i32(i32 %o, i1 false)
  %tz2 = call i32 @llvm.cttz.i32(i32 %x, i1 true)
  %pc = call i32 @llvm.ctpop.i32(i32 %a)
  %pc2 = call i32 @llvm.ctpop.i32(i32 %x)
  ret i32 %a
}

define i32 @binary(i32 %x, i32 %y) {
  %a = and i32 %x, 255
  %b = or i32 %y, 3
  %c = and i32 %y, 4080
  %fl = call i32 @llvm.fshl.i32(i32 %a, i32 %b, i32 5)
  %fr = call i32 @llvm.fshr.i32(i32 %a, i32 %b, i32 5)
  %fv = call i32 @llvm.fshl.i32(i32 %a, i32 %b, i32 %y)
  %cl = call i32 @llvm.clmul.i32(i32 %a, i32 %c)
  %pe = call i32 @llvm.pext.i32(i32 %x, i32 %c)
  %pd = call i32 @llvm.pdep.i32(i32 %a, i32 %c)
  %sh = call i32 @llvm.smulh.i32(i32 %a, i32 %c)
  %uh = call i32 @llvm.umulh.i32(i32 %a, i32 %c)
  %ua = call i32 @llvm.uadd.sat.i32(i32 %a, i32 %c)
  %us = call i32 @llvm.usub.sat.i32(i32 %a, i32 %c)
  %sa = call i32 @llvm.sadd.sat.i32(i32 %a, i32 %c)
  %ss = call i32 @llvm.ssub.sat.i32(i32 %a, i32 %c)
  %un = call i32 @llvm.umin.i32(i32 %a, i32 %c)
  %ux = call i32 @llvm.umax.i32(i32 %a, i32 %b)
  %sn = call i32 @llvm.smin.i32(i32 %a, i32 %y)
  %sx = call i32 @llvm.smax.i32(i32 %a, i32 %y)
  ret i32 %a
}

define i32 @clamp(i32 %x) {
  %lo = call i32 @llvm.smax.i32(i32 %x, i32 -3)
  %c1 = call i32 @llvm.smin.i32(i32 %lo, i32 12)
  %hi = call i32 @llvm.smin.i32(i32 %x, i32 12)
  %c2 = call i32 @llvm.smax.i32(i32 %hi, i32 -3)
  %bad = call i32 @llvm.smax.i32(i32 %hi, i32 100)
  %bad2 = call i32 @llvm.smin.i32(i32 %lo, i32 -100)
  %same = call i32 @llvm.smin.i32(i32 %lo, i32 -3)
  %u1 = call i32 @llvm.umax.i32(i32 %x, i32 3)
  %u2 = call i32 @llvm.umin.i32(i32 %u1, i32 12)
  ret i32 %c1
}

define ptr @mask(ptr align 16 %p, i64 %m) {
  %r = call ptr @llvm.ptrmask.p0.i64(ptr %p, i64 -4)
  %r2 = call ptr @llvm.ptrmask.p0.i64(ptr %p, i64 %m)
  %r3 = call ptr @llvm.ptrmask.p0.i64(ptr %p, i64 -64)
  ret ptr %r
}

define i32 @overflow(i32 %x, i32 %y) {
  %a = and i32 %x, 255
  %b = and i32 %y, 255
  %c = or i32 %y, 1
  %u = call {i32, i1} @llvm.uadd.with.overflow.i32(i32 %a, i32 %b)
  %s = call {i32, i1} @llvm.sadd.with.overflow.i32(i32 %a, i32 %c)
  %us = call {i32, i1} @llvm.usub.with.overflow.i32(i32 %a, i32 %b)
  %ss = call {i32, i1} @llvm.ssub.with.overflow.i32(i32 %a, i32 %b)
  %um = call {i32, i1} @llvm.umul.with.overflow.i32(i32 %a, i32 %b)
  %sm = call {i32, i1} @llvm.smul.with.overflow.i32(i32 %a, i32 %b)
  %u0 = extractvalue {i32, i1} %u, 0
  %s0 = extractvalue {i32, i1} %s, 0
  %us0 = extractvalue {i32, i1} %us, 0
  %ss0 = extractvalue {i32, i1} %ss, 0
  %um0 = extractvalue {i32, i1} %um, 0
  %sm0 = extractvalue {i32, i1} %sm, 0
  %u1 = extractvalue {i32, i1} %u, 1
  ret i32 %u0
}

define i64 @targets(i64 %x, i64 %y, i32 %z) {
  %c = call i64 @llvm.x86.sse42.crc32.64.64(i64 %x, i64 %y)
  %zo = and i32 %z, 255
  %m1 = call i32 @llvm.amdgcn.mbcnt.lo(i32 -1, i32 %zo)
  %m2 = call i32 @llvm.amdgcn.mbcnt.hi(i32 -1, i32 %zo)
  %v = call i64 @llvm.vscale.i64()
  %vl1 = call i64 @llvm.riscv.vsetvli.i64(i64 %x, i64 3, i64 0)
  %vl2 = call i64 @llvm.riscv.vsetvli.i64(i64 5, i64 2, i64 5)
  %vl3 = call i64 @llvm.riscv.vsetvlimax.i64(i64 0, i64 1)
  %vl4 = call i64 @llvm.riscv.vsetvlimax.i64(i64 2, i64 7)
  ret i64 %c
}
