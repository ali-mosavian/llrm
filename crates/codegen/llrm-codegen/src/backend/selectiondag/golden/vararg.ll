target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
declare void @llvm.va_copy.p0(ptr, ptr)

define i32 @sum(i32 %n, ...) {
  %ap = alloca ptr
  call void @llvm.va_start.p0(ptr %ap)
  %p = load ptr, ptr %ap
  %v = load i32, ptr %p
  call void @llvm.va_end.p0(ptr %ap)
  ret i32 %v
}

define i32 @copy(i32 %n, ...) {
  %ap = alloca ptr
  %bp = alloca ptr
  call void @llvm.va_start.p0(ptr %ap)
  call void @llvm.va_copy.p0(ptr %bp, ptr %ap)
  %p = load ptr, ptr %bp
  %v = load i32, ptr %p
  call void @llvm.va_end.p0(ptr %bp)
  ret i32 %v
}
