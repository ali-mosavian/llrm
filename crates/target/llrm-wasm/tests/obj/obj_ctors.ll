declare void @ext()
@llvm.global_ctors = appending global [3 x { i32, ptr, ptr }] [{ i32, ptr, ptr } { i32 65535, ptr @a, ptr null }, { i32, ptr, ptr } { i32 100, ptr @b, ptr null }, { i32, ptr, ptr } { i32 65535, ptr @c, ptr null }]
define internal void @a() {
  call void @ext()
  ret void
}
define void @b() {
  ret void
}
define void @c() {
  ret void
}
