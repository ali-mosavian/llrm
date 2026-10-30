define i32 @f(ptr %p) {
entry:
  %a = load i32, ptr %p
  %b = load i32, ptr %p
  %r = add i32 %a, %b
  ret i32 %r
}
