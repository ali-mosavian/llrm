target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"
declare void @use(ptr)
declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)
define void @two() {
  %a = alloca [64 x i32], align 4
  %b = alloca [64 x i32], align 4
  call void @llvm.lifetime.start.p0(i64 256, ptr %a)
  call void @use(ptr %a)
  call void @llvm.lifetime.end.p0(i64 256, ptr %a)
  call void @llvm.lifetime.start.p0(i64 256, ptr %b)
  call void @use(ptr %b)
  call void @llvm.lifetime.end.p0(i64 256, ptr %b)
  ret void
}
