declare i32 @llvm.umin.i32(i32, i32)
declare i32 @llvm.umax.i32(i32, i32)
declare i32 @llvm.smin.i32(i32, i32)
declare i32 @llvm.smax.i32(i32, i32)
declare i32 @llvm.ctlz.i32(i32, i1)
declare i32 @llvm.cttz.i32(i32, i1)
declare i32 @llvm.ctpop.i32(i32)
declare i8 @llvm.uadd.sat.i8(i8, i8)
declare i8 @llvm.usub.sat.i8(i8, i8)
declare i8 @llvm.sadd.sat.i8(i8, i8)
declare i8 @llvm.ssub.sat.i8(i8, i8)
declare { i8, i1 } @llvm.uadd.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.sadd.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.umul.with.overflow.i8(i8, i8)
declare { i8, i1 } @llvm.usub.with.overflow.i8(i8, i8)

define i32 @minmax(i32 %x, i32 %y) {
entry:
  %a = icmp ult i32 %x, 100
  %b = icmp ult i32 %y, 50
  %c = and i1 %a, %b
  br i1 %c, label %ok, label %out
ok:
  %m1 = call i32 @llvm.umin.i32(i32 %x, i32 %y)
  %m2 = call i32 @llvm.umax.i32(i32 %x, i32 %y)
  %m3 = call i32 @llvm.smin.i32(i32 %x, i32 10)
  %m4 = call i32 @llvm.smax.i32(i32 %x, i32 10)
  %s = add i32 %m1, %m2
  %t = add i32 %m3, %m4
  %r = add i32 %s, %t
  ret i32 %r
out:
  ret i32 0
}

define i32 @bits(i32 %x) {
entry:
  %a = icmp ult i32 %x, 100
  br i1 %a, label %ok, label %out
ok:
  %l = call i32 @llvm.ctlz.i32(i32 %x, i1 false)
  %t = call i32 @llvm.cttz.i32(i32 %x, i1 true)
  %p = call i32 @llvm.ctpop.i32(i32 %x)
  %s = add i32 %l, %t
  %r = add i32 %s, %p
  ret i32 %r
out:
  ret i32 0
}

define i8 @sat(i8 %x, i8 %y) {
entry:
  %a = icmp ult i8 %x, 100
  br i1 %a, label %ok, label %out
ok:
  %u = call i8 @llvm.uadd.sat.i8(i8 %x, i8 200)
  %d = call i8 @llvm.usub.sat.i8(i8 %x, i8 50)
  %s = call i8 @llvm.sadd.sat.i8(i8 %x, i8 100)
  %t = call i8 @llvm.ssub.sat.i8(i8 %x, i8 -100)
  %r1 = add i8 %u, %d
  %r2 = add i8 %s, %t
  %r = add i8 %r1, %r2
  ret i8 %r
out:
  ret i8 0
}

define i8 @overflow(i8 %x) {
entry:
  %a = icmp ult i8 %x, 100
  br i1 %a, label %ok, label %out
ok:
  %o = call { i8, i1 } @llvm.uadd.with.overflow.i8(i8 %x, i8 100)
  %v = extractvalue { i8, i1 } %o, 0
  %f = extractvalue { i8, i1 } %o, 1
  br i1 %f, label %ovf, label %out
ovf:
  %r = add i8 %v, 0
  ret i8 %r
out:
  ret i8 0
}

define i8 @overflow_cond(i8 %x) {
entry:
  %o = call { i8, i1 } @llvm.uadd.with.overflow.i8(i8 %x, i8 10)
  %f = extractvalue { i8, i1 } %o, 1
  br i1 %f, label %ovf, label %fine
ovf:
  %r = add i8 %x, 0
  ret i8 %r
fine:
  %q = add i8 %x, 1
  ret i8 %q
}

define i8 @overflow_signed(i8 %x) {
entry:
  %o = call { i8, i1 } @llvm.sadd.with.overflow.i8(i8 %x, i8 100)
  %f = extractvalue { i8, i1 } %o, 1
  br i1 %f, label %ovf, label %fine
ovf:
  %r = add i8 %x, 0
  ret i8 %r
fine:
  %q = add i8 %x, 1
  ret i8 %q
}

define i8 @umul_one(i8 %x) {
entry:
  %o = call { i8, i1 } @llvm.umul.with.overflow.i8(i8 %x, i8 1)
  %v = extractvalue { i8, i1 } %o, 0
  %f = extractvalue { i8, i1 } %o, 1
  %r = select i1 %f, i8 %v, i8 %x
  ret i8 %v
}

define i8 @insertvalue(i8 %x) {
entry:
  %a = icmp ult i8 %x, 10
  br i1 %a, label %ok, label %out
ok:
  %i = insertvalue { i8, i1 } poison, i8 %x, 0
  %v = extractvalue { i8, i1 } %i, 0
  %r = add i8 %v, 1
  ret i8 %r
out:
  ret i8 0
}

define i8 @usub_ovf(i8 %x, i8 %y) {
entry:
  %a = icmp ult i8 %x, 10
  %b = icmp ugt i8 %y, 20
  %c = and i1 %a, %b
  br i1 %c, label %ok, label %out
ok:
  %o = call { i8, i1 } @llvm.usub.with.overflow.i8(i8 %x, i8 %y)
  %v = extractvalue { i8, i1 } %o, 0
  ret i8 %v
out:
  ret i8 0
}
