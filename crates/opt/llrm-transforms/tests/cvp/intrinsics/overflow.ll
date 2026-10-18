declare { i32, i1 } @llvm.uadd.with.overflow.i32(i32, i32)
declare i32 @llvm.usub.sat.i32(i32, i32)

define i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 255
  %b = and i32 %y, 255
  %o = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue { i32, i1 } %o, 0
  %f = extractvalue { i32, i1 } %o, 1
  %z = zext i1 %f to i32
  %c = or i32 %x, 1024
  %s = call i32 @llvm.usub.sat.i32(i32 %c, i32 %a)
  %t = add i32 %v, %z
  %u = add i32 %t, %s
  ret i32 %u
}
