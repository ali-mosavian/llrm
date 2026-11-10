target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32"

declare void @llvm.lifetime.start.p0(ptr)
declare void @llvm.lifetime.end.p0(ptr)
declare void @llvm.va_start.p0(ptr)
declare void @llvm.va_end.p0(ptr)

define i32 @walk(i32 %n, ...) {
entry:
  %slot = alloca ptr, align 4
  call void @llvm.lifetime.start.p0(ptr %slot)
  call void @llvm.va_start.p0(ptr %slot)
  %first = load ptr, ptr %slot, align 4
  %go = icmp sgt i32 %n, 0
  br i1 %go, label %loop, label %exit

loop:
  %i = phi i32 [ 0, %entry ], [ %i1, %loop ]
  %sum = phi i32 [ 0, %entry ], [ %sum1, %loop ]
  %p = phi ptr [ %first, %entry ], [ %next, %loop ]
  %v = load i32, ptr %p, align 4
  %sum1 = add i32 %sum, %v
  %next = getelementptr inbounds i8, ptr %p, i32 4
  %i1 = add i32 %i, 1
  %more = icmp ne i32 %i1, %n
  br i1 %more, label %loop, label %exit

exit:
  %last = phi ptr [ %first, %entry ], [ %next, %loop ]
  %total = phi i32 [ 0, %entry ], [ %sum1, %loop ]
  store ptr %last, ptr %slot, align 4
  call void @llvm.va_end.p0(ptr %slot)
  call void @llvm.lifetime.end.p0(ptr %slot)
  ret i32 %total
}
