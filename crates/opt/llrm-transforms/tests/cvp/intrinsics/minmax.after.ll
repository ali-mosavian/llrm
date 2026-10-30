declare i32 @llvm.smin.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.umax.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 15
  %b = or i32 %y, 16
  %d = call i32 @llvm.smin.i32(i32 %a, i32 %b)
  %e = and i32 %y, 255
  %0 = call i32 @llvm.umin.i32(i32 %a, i32 %e)
  %h = add i32 %b, %d
  %i = add i32 %h, %0
  ret i32 %i
}

declare i32 @llvm.umin.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)
