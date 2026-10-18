declare range(i8 -1, 2) i8 @llvm.scmp.i8.i32(i32, i32)

declare range(i8 -1, 2) i8 @llvm.ucmp.i8.i32(i32, i32)

define range(i8 -2, 1) i8 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 15
  %b = or i32 %y, 16
  %d = call i8 @llvm.scmp.i8.i32(i32 %b, i32 %a)
  %e = add nsw i8 -1, %d
  ret i8 %e
}
