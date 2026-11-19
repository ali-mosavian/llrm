target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

declare void @llvm.lifetime.start.p0(i64, ptr)
declare void @llvm.lifetime.end.p0(i64, ptr)
declare void @use(ptr)

define void @disjoint() {
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

define void @offset() {
  %a = alloca [16 x i32], align 4
  %b = alloca [16 x i32], align 4
  %p = getelementptr i8, ptr %a, i32 8
  call void @use(ptr %p)
  call void @use(ptr %b)
  ret void
}
