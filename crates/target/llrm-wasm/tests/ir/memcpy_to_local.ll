declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
declare void @use(ptr)
define void @f(ptr %src) {
  %a = alloca [100 x i8], align 4
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %a, ptr align 4 %src, i32 100, i1 false)
  call void @use(ptr %a)
  ret void
}
