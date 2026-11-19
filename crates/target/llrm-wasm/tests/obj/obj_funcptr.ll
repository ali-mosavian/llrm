declare void @ext(i32)
@d = global ptr @c
define void @a() {
  call void @ext(i32 1)
  ret void
}
define i64 @callit(ptr %f, double %x) {
  %r = call i64 %f(double %x, i32 5)
  ret i64 %r
}
define ptr @addr() {
  ret ptr @a
}
define void @c() {
  ret void
}
define ptr @addr2() {
  ret ptr @ext2
}
declare void @ext2()
