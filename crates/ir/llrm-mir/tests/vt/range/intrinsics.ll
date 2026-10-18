declare i32 @llvm.ctlz.i32(i32, i1)
declare i32 @llvm.cttz.i32(i32, i1)
declare i32 @llvm.ctpop.i32(i32)
declare i8 @llvm.uadd.sat.i8(i8, i8)
declare i8 @llvm.sadd.sat.i8(i8, i8)
declare i8 @llvm.usub.sat.i8(i8, i8)
declare i8 @llvm.ssub.sat.i8(i8, i8)
declare i8 @llvm.umin.i8(i8, i8)
declare i8 @llvm.umax.i8(i8, i8)
declare i8 @llvm.smin.i8(i8, i8)
declare i8 @llvm.smax.i8(i8, i8)
declare i8 @llvm.abs.i8(i8, i1)
declare i32 @llvm.vscale.i32()
declare i32 @llvm.unknown.i32(i32)

define void @counts(i32 %x) {
  %a = call i32 @llvm.ctlz.i32(i32 %x, i1 false)
  %b = call i32 @llvm.ctlz.i32(i32 %x, i1 true)
  %c = call i32 @llvm.cttz.i32(i32 %x, i1 false)
  %d = call i32 @llvm.cttz.i32(i32 %x, i1 true)
  %e = call i32 @llvm.ctpop.i32(i32 %x)
  %f = call i32 @llvm.vscale.i32()
  %g = call i32 @llvm.unknown.i32(i32 %x)
  ret void
}

define void @sat(i8 %x) {
  %a = call i8 @llvm.uadd.sat.i8(i8 %x, i8 10)
  %b = call i8 @llvm.uadd.sat.i8(i8 10, i8 %x)
  %c = call i8 @llvm.sadd.sat.i8(i8 %x, i8 -10)
  %d = call i8 @llvm.sadd.sat.i8(i8 %x, i8 10)
  %e = call i8 @llvm.usub.sat.i8(i8 10, i8 %x)
  %f = call i8 @llvm.usub.sat.i8(i8 %x, i8 10)
  %g = call i8 @llvm.ssub.sat.i8(i8 -10, i8 %x)
  %h = call i8 @llvm.ssub.sat.i8(i8 10, i8 %x)
  %i = call i8 @llvm.ssub.sat.i8(i8 %x, i8 -10)
  %j = call i8 @llvm.ssub.sat.i8(i8 %x, i8 10)
  %k = call i8 @llvm.uadd.sat.i8(i8 %x, i8 %x)
  ret void
}

define void @minmax(i8 %x, i8 %y) {
  %a = call i8 @llvm.umin.i8(i8 %x, i8 10)
  %b = call i8 @llvm.umax.i8(i8 10, i8 %x)
  %c = call i8 @llvm.smin.i8(i8 %x, i8 -10)
  %d = call i8 @llvm.smax.i8(i8 %x, i8 -10)
  %e = call i8 @llvm.smax.i8(i8 %x, i8 %y)
  %f = call i8 @llvm.abs.i8(i8 %x, i1 true)
  %g = call i8 @llvm.abs.i8(i8 %x, i1 false)
  ret void
}
