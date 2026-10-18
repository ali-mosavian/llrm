define range(i32 0, 228) i32 @f(i16 %x, i32 %y) {
entry:
  %a = and i32 %y, 1023
  %b1 = lshr i32 %a, 3
  %c = and i16 %x, 100
  %d1 = zext nneg i16 %c to i32
  %e = add nuw nsw i32 %b1, %d1
  ret i32 %e
}
