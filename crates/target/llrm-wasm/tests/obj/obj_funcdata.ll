@p = global ptr @a
@q = global [2 x ptr] [ptr @a, ptr @b]
define void @a() {
  ret void
}
define void @b() {
  ret void
}
define void @callit(ptr %f) {
  call void %f()
  ret void
}
define ptr @addr() {
  ret ptr @b
}
