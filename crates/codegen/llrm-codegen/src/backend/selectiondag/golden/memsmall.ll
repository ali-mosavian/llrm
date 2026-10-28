target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)

define void @cpy16(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 16, i1 false)
  ret void
}

define void @cpy7(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 7, i1 false)
  ret void
}

define void @cpy1(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 1, i1 false)
  ret void
}

define void @cpy32a(ptr align 4 %d, ptr align 4 %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %d, ptr align 4 %s, i32 32, i1 false)
  ret void
}

define void @cpy66(ptr align 4 %d, ptr align 4 %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %d, ptr align 4 %s, i32 66, i1 false)
  ret void
}

define void @set8(ptr %d) {
  call void @llvm.memset.p0.i32(ptr %d, i8 0, i32 8, i1 false)
  ret void
}

define void @set13v(ptr %d, i8 %v) {
  call void @llvm.memset.p0.i32(ptr %d, i8 %v, i32 13, i1 false)
  ret void
}

define void @set101(ptr align 4 %d) {
  call void @llvm.memset.p0.i32(ptr align 4 %d, i8 7, i32 101, i1 false)
  ret void
}

define void @set102v(ptr align 4 %d, i8 %v) {
  call void @llvm.memset.p0.i32(ptr align 4 %d, i8 %v, i32 102, i1 false)
  ret void
}
