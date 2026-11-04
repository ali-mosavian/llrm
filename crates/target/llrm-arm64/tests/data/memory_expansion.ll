target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"
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

@g = global [16 x i8] zeroinitializer

define i16 @odd_global() {
  %p = getelementptr i8, ptr @g, i64 1
  %v = load i16, ptr %p, align 1
  ret i16 %v
}
