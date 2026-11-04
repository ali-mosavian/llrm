define i64 @callit(ptr %f, double %x) {
  %r = call i64 %f(double %x, i32 5)
  ret i64 %r
}
define void @v(ptr %f) {
  call void %f()
  ret void
}
define i32 @addr_call(i32 %x) {
  %r = call i32 @self(i32 %x)
  ret i32 %r
}
define i32 @self(i32 %x) {
  ret i32 %x
}
define ptr @addr() {
  ret ptr @self
}
define i32 @via_table(i32 %x) {
  %r = call i32 @self(i32 %x)
  %p = call ptr @addr()
  %q = call i32 %p(i32 %r)
  ret i32 %q
}
