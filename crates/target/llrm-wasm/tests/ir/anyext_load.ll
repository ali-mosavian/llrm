define i32 @g(ptr %p, ptr %q) {
  %x = load i8, ptr %p, align 1
  store i8 %x, ptr %q, align 1
  %y = load i8, ptr %q, align 1
  %z = sext i8 %y to i32
  ret i32 %z
}
