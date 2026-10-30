define range(i8 0, -128) i8 @f(i32 %x) {
entry:
  %a = and i32 %x, 127
  %b = trunc nuw nsw i32 %a to i8
  ret i8 %b
}
