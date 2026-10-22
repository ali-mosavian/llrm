target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"
declare i32 @llvm.fshl.i32(i32, i32, i32)
declare i32 @llvm.fshr.i32(i32, i32, i32)
declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)
declare void @llvm.memmove.p0.p0.i32(ptr, ptr, i32, i1)
declare void @use(ptr)
define i32 @funnel(i32 %a, i32 %b, i32 %c) {
  %x = call i32 @llvm.fshl.i32(i32 %a, i32 %b, i32 %c)
  %y = call i32 @llvm.fshr.i32(i32 %x, i32 %x, i32 %c)
  ret i32 %y
}
define void @life() {
  %buf = alloca [16 x i8]
  call void @llvm.lifetime.start.p0(i64 16, ptr %buf)
  call void @use(ptr %buf)
  call void @llvm.lifetime.end.p0(i64 16, ptr %buf)
  ret void
}
define void @move16(ptr %d, ptr %s) {
  call void @llvm.memmove.p0.p0.i32(ptr %d, ptr %s, i32 16, i1 false)
  ret void
}
define void @move7(ptr %d, ptr %s) {
  call void @llvm.memmove.p0.p0.i32(ptr %d, ptr %s, i32 7, i1 false)
  ret void
}
define void @movevar(ptr %d, ptr %s, i32 %n) {
  call void @llvm.memmove.p0.p0.i32(ptr %d, ptr %s, i32 %n, i1 false)
  ret void
}
