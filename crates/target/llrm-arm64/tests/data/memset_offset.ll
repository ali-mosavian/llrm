target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define void @zr300(ptr %d) {
  call void @llvm.memset.p0.i64(ptr align 1 %d, i8 0, i64 300, i1 false)
  ret void
}

define void @cp300(ptr %d, ptr %s) {
  call void @llvm.memcpy.p0.p0.i64(ptr align 1 %d, ptr align 1 %s, i64 300, i1 false)
  ret void
}

declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
declare void @llvm.memcpy.p0.p0.i64(ptr, ptr, i64, i1)
