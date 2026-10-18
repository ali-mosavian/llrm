declare i32 @llvm.abs.i32(i32, i1 immarg) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define range(i32 1, 512) i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 255
  %c = or i32 %y, -256
  %d1 = sub nsw i32 0, %c
  %e = add nuw nsw i32 %a, %d1
  ret i32 %e
}
