define i32 @f(ptr %p) {
entry:
  %a = load i32, ptr %p
  %r = add i32 %a, %a
  ret i32 %r
}
