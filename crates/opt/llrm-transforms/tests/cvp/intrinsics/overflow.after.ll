declare { i32, i1 } @llvm.uadd.with.overflow.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

declare i32 @llvm.usub.sat.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define range(i32 769, 511) i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 255
  %b = and i32 %y, 255
  %o1 = add nuw nsw i32 %a, %b
  %0 = insertvalue { i32, i1 } { i32 poison, i1 false }, i32 %o1, 0
  %v = extractvalue { i32, i1 } %0, 0
  %f = extractvalue { i32, i1 } %0, 1
  %z = zext i1 %f to i32
  %c = or i32 %x, 1024
  %s1 = sub nuw i32 %c, %a
  %t = add nuw nsw i32 %v, %z
  %u = add i32 %t, %s1
  ret i32 %u
}
