define range(i32 0, 16) i32 @f(i32 %x) {
entry:
  %a = and i32 %x, 15
  ret i32 %a
}
