declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memmove.p0.p0.i64(ptr, ptr, i64, i1)
declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)

define void @copy_224(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 224, i1 false)
  ret void
}
define void @copy_257(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i64(ptr %d, ptr %s, i64 257, i1 false)
  ret void
}
define void @move_40(ptr %d, ptr %s) {
  call void @llvm.memmove.p0.p0.i64(ptr %d, ptr %s, i64 40, i1 false)
  ret void
}
define void @zero_300(ptr %d) {
  call void @llvm.memset.p0.i64(ptr %d, i8 0, i64 300, i1 false)
  ret void
}
define void @fill_20(ptr %d, i8 %v) {
  call void @llvm.memset.p0.i64(ptr %d, i8 %v, i64 20, i1 false)
  ret void
}
