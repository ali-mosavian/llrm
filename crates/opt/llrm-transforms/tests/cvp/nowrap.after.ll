define range(i32 -255, 70126) i32 @f(i32 %x, i32 %y) {
entry:
  %a = and i32 %x, 255
  %b = and i32 %y, 255
  %c = add nuw nsw i32 %a, %b
  %d = mul nuw nsw i32 %a, %b
  %e = sub nsw i32 %c, %a
  %g = shl nuw nsw i32 %a, 4
  %h = add nuw nsw i32 %c, %d
  %i = add nsw i32 %h, %e
  %j = add nsw i32 %i, %g
  ret i32 %j
}
