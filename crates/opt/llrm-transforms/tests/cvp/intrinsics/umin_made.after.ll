declare i32 @llvm.smin.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)

define range(i32 100, 116) i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 15
  %b = and i32 %y, 15
  %0 = call i32 @llvm.umin.i32(i32 %a, i32 %b)
  %z = add nuw nsw i32 %0, 100
  ret i32 %z
}

declare i32 @llvm.umin.i32(i32, i32) nocallback nofree nosync nounwind speculatable willreturn memory(none)
