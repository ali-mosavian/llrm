target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

declare void @llvm.memcpy.p0.p0.i32(ptr, ptr, i32, i1)
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)

define void @cpy(ptr %d, ptr %s, i32 %n) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)
  ret void
}

define void @cpy1k(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr %d, ptr %s, i32 1024, i1 false)
  ret void
}

define void @set(ptr %d, i8 %v, i32 %n) {
  call void @llvm.memset.p0.i32(ptr %d, i8 %v, i32 %n, i1 false)
  ret void
}

define void @cpy64(ptr align 4 %d, ptr align 4 %s) {
  call void @llvm.memcpy.p0.p0.i32(ptr align 4 %d, ptr align 4 %s, i32 64, i1 false)
  ret void
}

define void @set100(ptr align 4 %d) {
  call void @llvm.memset.p0.i32(ptr align 4 %d, i8 7, i32 100, i1 false)
  ret void
}

define void @set100v(ptr align 4 %d, i8 %v) {
  call void @llvm.memset.p0.i32(ptr align 4 %d, i8 %v, i32 100, i1 false)
  ret void
}
