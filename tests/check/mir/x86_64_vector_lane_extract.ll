; RUN: llrm-llc -O2 -S -o /dev/stdout %s
; A constant lane of a 128-bit integer vector, read after the vector was computed, was `CannotSelect("ExtractElement")`
; (memory_expansion -O3, x86-64): only lane 0 of a dword or quadword had a selection.
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

define i32 @dword(ptr %p, ptr %r) {
  %x = load <4 x i32>, ptr %p, align 16
  %y = load <4 x i32>, ptr %r, align 16
  %z = add <4 x i32> %x, %y
  %e = extractelement <4 x i32> %z, i32 3
  ret i32 %e
}

define i64 @quad(ptr %p, ptr %r) {
  %x = load <2 x i64>, ptr %p, align 16
  %y = load <2 x i64>, ptr %r, align 16
  %z = add <2 x i64> %x, %y
  %e = extractelement <2 x i64> %z, i32 1
  ret i64 %e
}

define i16 @word(ptr %p, ptr %r) {
  %x = load <8 x i16>, ptr %p, align 16
  %y = load <8 x i16>, ptr %r, align 16
  %z = add <8 x i16> %x, %y
  %e = extractelement <8 x i16> %z, i32 5
  ret i16 %e
}

define i8 @byte(ptr %p, ptr %r) {
  %x = load <16 x i8>, ptr %p, align 16
  %y = load <16 x i8>, ptr %r, align 16
  %z = add <16 x i8> %x, %y
  %e = extractelement <16 x i8> %z, i32 13
  ret i8 %e
}

define void @copy_lane(ptr %p, ptr %q) {
  %v = load <16 x i8>, ptr %p, align 16
  %e = extractelement <16 x i8> %v, i32 1
  %w = insertelement <16 x i8> %v, i8 %e, i32 5
  store <16 x i8> %w, ptr %q, align 16
  ret void
}

; CHECK-LABEL: dword proc
; CHECK: pshufd xmm0, xmm0, -1
; CHECK-NEXT: movd eax, xmm0
; CHECK-LABEL: quad proc
; CHECK: pshufd xmm0, xmm0, -18
; CHECK-NEXT: movq rax, xmm0
; CHECK-LABEL: word proc
; CHECK: pshufd xmm0, xmm0, -86
; CHECK-NEXT: movd eax, xmm0
; CHECK-NEXT: shr eax, 16
; CHECK-LABEL: byte proc
; CHECK: pshufd xmm0, xmm0, -1
; CHECK-NEXT: movd eax, xmm0
; CHECK-NEXT: shr eax, 8
; CHECK-LABEL: copy_lane proc
; CHECK: ret
