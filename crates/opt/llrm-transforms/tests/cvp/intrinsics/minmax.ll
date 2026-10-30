declare i32 @llvm.smin.i32(i32, i32)
declare i32 @llvm.umax.i32(i32, i32)

define i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 15
  %b = or i32 %y, 16
  %c = call i32 @llvm.umax.i32(i32 %a, i32 %b)
  %d = call i32 @llvm.smin.i32(i32 %a, i32 %b)
  %e = and i32 %y, 255
  %g = call i32 @llvm.smin.i32(i32 %a, i32 %e)
  %h = add i32 %c, %d
  %i = add i32 %h, %g
  ret i32 %i
}
