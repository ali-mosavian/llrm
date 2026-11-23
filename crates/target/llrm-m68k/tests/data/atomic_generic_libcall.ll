target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
@started = global i32 0, align 4
declare void @__atomic_load(i32, ptr, ptr, i32)
declare void @__atomic_store(i32, ptr, ptr, i32)
define i32 @started_now() {
  %out = alloca i32, align 4
  call void @__atomic_load(i32 4, ptr @started, ptr %out, i32 5)
  %v = load i32, ptr %out, align 4
  ret i32 %v
}
define void @start() {
  %in = alloca i32, align 4
  store i32 1, ptr %in, align 4
  call void @__atomic_store(i32 4, ptr @started, ptr %in, i32 5)
  ret void
}
@odd = global i32 0, align 1
define i32 @odd_now() {
  %out = alloca i32, align 4
  call void @__atomic_load(i32 4, ptr @odd, ptr %out, i32 5)
  %v = load i32, ptr %out, align 4
  ret i32 %v
}
define i32 @through_pointer(ptr %p) {
  %out = alloca i32, align 4
  call void @__atomic_load(i32 4, ptr %p, ptr %out, i32 5)
  %v = load i32, ptr %out, align 4
  ret i32 %v
}
