; The umin made from a smin is known to LVI: the add after it is nuw nsw.
declare i32 @llvm.smin.i32(i32, i32)

define i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 15
  %b = and i32 %y, 15
  %m = call i32 @llvm.smin.i32(i32 %a, i32 %b)
  %z = add i32 %m, 100
  ret i32 %z
}
