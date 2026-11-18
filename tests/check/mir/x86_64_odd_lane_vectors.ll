; RUN: llrm-llc -O2 -S -o /dev/stdout %s
; memory_expansion -O3 (x86-64): a vector of an odd lane count (`<3 x i8>`) was refused, "vector with no register class"; the
; pin widens it (`v3i8`, `v4i8`, `v16i8`). A dword loaded into lane 0 of an xmm register (a `<7 x i8>` load) was `Unmapped("MOVDI2PDIrm")`.
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

@src = global [64 x i8] zeroinitializer, align 16
@dst = global [64 x i8] zeroinitializer, align 16

define void @xor3() {
  %v = load <3 x i8>, ptr getelementptr inbounds (i8, ptr @src, i64 5), align 1
  %w = xor <3 x i8> %v, <i8 1, i8 2, i8 3>
  store <3 x i8> %w, ptr getelementptr inbounds (i8, ptr @dst, i64 9), align 1
  ret void
}

define i8 @lane3(ptr %p) {
  %v = load <3 x i8>, ptr %p, align 1
  %e = extractelement <3 x i8> %v, i32 2
  ret i8 %e
}

define void @seven(ptr %p, ptr %q) {
  %v = load <7 x i8>, ptr %p, align 1
  %w = xor <7 x i8> %v, <i8 1, i8 2, i8 3, i8 4, i8 5, i8 6, i8 7>
  store <7 x i8> %w, ptr %q, align 1
  ret void
}

; CHECK-LABEL: xor3 proc
; CHECK: mov word ptr dst+9
; CHECK: mov byte ptr dst+11
; CHECK-LABEL: lane3 proc
; CHECK: movzx eax, byte ptr [rdi+2]
; CHECK-LABEL: seven proc
; CHECK: movd xmm0, dword ptr [rdi]
