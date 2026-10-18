declare i32 @llvm.abs.i32(i32, i1)

define i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 255
  %b = call i32 @llvm.abs.i32(i32 %a, i1 false)
  %c = or i32 %y, -256
  %d = call i32 @llvm.abs.i32(i32 %c, i1 true)
  %e = add i32 %b, %d
  ret i32 %e
}
