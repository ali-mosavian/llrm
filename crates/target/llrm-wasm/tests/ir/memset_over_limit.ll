target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare void @llvm.memset.p0.i32(ptr, i8, i32, i1)
define void @fill(ptr %d) {
  call void @llvm.memset.p0.i32(ptr %d, i8 0, i32 72, i1 false)
  ret void
}
