target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

; out[i] = min(max(in[i], 0), 255) over 1024 ints: the clamp of jpeg_fixed.
define void @clamp(ptr noalias %out, ptr noalias %in) {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %src = getelementptr inbounds i32, ptr %in, i64 %i
  %x = load i32, ptr %src, align 4
  %low = call i32 @llvm.smax.i32(i32 %x, i32 0)
  %high = call i32 @llvm.smin.i32(i32 %low, i32 255)
  %dst = getelementptr inbounds i32, ptr %out, i64 %i
  store i32 %high, ptr %dst, align 4
  %next = add nuw nsw i64 %i, 1
  %done = icmp eq i64 %next, 1024
  br i1 %done, label %exit, label %loop
exit:
  ret void
}

declare i32 @llvm.smax.i32(i32, i32)
declare i32 @llvm.smin.i32(i32, i32)
