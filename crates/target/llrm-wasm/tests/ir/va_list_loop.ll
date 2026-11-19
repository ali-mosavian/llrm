target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-wasi"

define fp128 @sum(i32 %n, ...) {
  %ap = alloca ptr, align 4
  call void @llvm.va_start.p0(ptr %ap)
  br label %head

head:
  %acc = phi fp128 [ 0xL00000000000000000000000000000000, %0 ], [ %next, %body ]
  %i = phi i32 [ 0, %0 ], [ %i1, %body ]
  %c = icmp slt i32 %i, %n
  br i1 %c, label %body, label %done

body:
  %cur = load ptr, ptr %ap, align 4
  %up = getelementptr inbounds i8, ptr %cur, i32 15
  %slot = call align 16 ptr @llvm.ptrmask.p0.i32(ptr %up, i32 -16)
  %after = getelementptr inbounds i8, ptr %slot, i32 16
  store ptr %after, ptr %ap, align 4
  %arg = load fp128, ptr %slot, align 16
  %next = fadd fp128 %acc, %arg
  %i1 = add i32 %i, 1
  br label %head

done:
  call void @llvm.va_end.p0(ptr %ap)
  ret fp128 %acc
}

declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)
declare ptr @llvm.ptrmask.p0.i32(ptr, i32)
