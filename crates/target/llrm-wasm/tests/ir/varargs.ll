declare i32 @printf(ptr, ...)

define i32 @none(ptr %f) {
  %r = call i32 (ptr, ...) @printf(ptr %f)
  ret i32 %r
}

define i32 @one(ptr %f, i32 %a) {
  %r = call i32 (ptr, ...) @printf(ptr %f, i32 %a)
  ret i32 %r
}

define i32 @through_a_pointer(ptr %g, ptr %f, i32 %a) {
  %r = call i32 (ptr, ...) %g(ptr %f, i32 %a)
  ret i32 %r
}
