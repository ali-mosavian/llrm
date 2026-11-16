; RUN: llrm-llc -O2 -S -o /dev/stdout %s
; memory_expansion -O3 (x86-64) failed to compile: an i128 cast to `<16 x i8>` was `CannotSelect("BuildPair")`; a
; `zext` of an `i104` load panicked "an extending load widens"; a lane-0 `movd` store was `Unmapped("MOVPDI2DImr")`.
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

define void @i128_to_bytes(ptr %p, ptr %q) {
  %v = load <16 x i8>, ptr %p, align 16
  %i = bitcast <16 x i8> %v to i128
  %s = shl i128 %i, 16
  %a = and i128 %s, 4278190080
  %o = or disjoint i128 %a, -22685491128062564230891640499444191506
  %r = bitcast i128 %o to <16 x i8>
  store <16 x i8> %r, ptr %q, align 16
  ret void
}

define void @zext_i104(ptr %p, ptr %q) {
  %c = load i104, ptr %p, align 1
  %z = zext i104 %c to i128
  %s = shl nuw i128 %z, 24
  %o = or disjoint i128 %s, 15658734
  %r = bitcast i128 %o to <16 x i8>
  store <16 x i8> %r, ptr %q, align 16
  ret void
}

define void @lane0_store(ptr %p, ptr %q) {
  %v = load <4 x i32>, ptr %p, align 16
  %x = xor <4 x i32> %v, <i32 1, i32 2, i32 3, i32 4>
  %e = extractelement <4 x i32> %x, i32 0
  store i32 %e, ptr %q, align 1
  ret void
}

; CHECK-LABEL: i128_to_bytes proc
; CHECK: movaps xmmword ptr [rsi], xmm0
; CHECK-LABEL: zext_i104 proc
; CHECK: movaps xmmword ptr [rsi], xmm0
; CHECK-LABEL: lane0_store proc
; CHECK: movd dword ptr [rsi], xmm0
