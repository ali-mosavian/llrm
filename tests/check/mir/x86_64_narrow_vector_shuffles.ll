; RUN: llrm-llc -O2 -S -o /dev/stdout %s
; SROA's `<2 x i8>` pieces of a 16-byte copy (memory_expansion -O3, x86-64) were refused: `ExtractSubvector` and `bitcast`
; of a narrow vector could not be widened, `ConcatVectors` of one neither, a `shufflevector` was `CannotSelect`, and an
; `extract_element` made twice was returned unlowered the second time (the replaced node was still in the CSE table).
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
target triple = "x86_64-unknown-linux-gnu"

@src = global [64 x i8] zeroinitializer, align 16
@dst = global [64 x i8] zeroinitializer, align 16

define void @piece() {
  %v = load <16 x i8>, ptr @src, align 16
  %x = shufflevector <16 x i8> %v, <16 x i8> poison, <2 x i32> <i32 1, i32 2>
  %i = bitcast <2 x i8> %x to i16
  %y = bitcast i16 %i to <2 x i8>
  %w = shufflevector <2 x i8> %y, <2 x i8> poison, <16 x i32> <i32 poison, i32 poison, i32 poison, i32 0, i32 1, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison>
  %s = insertelement <16 x i8> poison, i8 -18, i32 0
  %t = shufflevector <16 x i8> %s, <16 x i8> poison, <16 x i32> zeroinitializer
  %b = shufflevector <16 x i8> %w, <16 x i8> %t, <16 x i32> <i32 16, i32 17, i32 18, i32 3, i32 4, i32 21, i32 22, i32 23, i32 24, i32 25, i32 26, i32 27, i32 28, i32 29, i32 30, i32 31>
  store <16 x i8> %b, ptr @dst, align 16
  %x2 = shufflevector <16 x i8> %v, <16 x i8> poison, <2 x i32> <i32 2, i32 3>
  %i2 = bitcast <2 x i8> %x2 to i16
  %y2 = bitcast i16 %i2 to <2 x i8>
  %w2 = shufflevector <2 x i8> %y2, <2 x i8> poison, <16 x i32> <i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 0, i32 1, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison, i32 poison>
  %b2 = shufflevector <16 x i8> %w2, <16 x i8> %v, <16 x i32> <i32 16, i32 17, i32 18, i32 19, i32 20, i32 5, i32 6, i32 23, i32 24, i32 25, i32 26, i32 27, i32 28, i32 29, i32 30, i32 31>
  store <16 x i8> %b2, ptr getelementptr inbounds (i8, ptr @dst, i64 16), align 16
  ret void
}

define void @interleave(ptr %p, ptr %q) {
  %v = load <16 x i8>, ptr %p, align 16
  %w = load <16 x i8>, ptr %q, align 16
  %s = shufflevector <16 x i8> %v, <16 x i8> %w, <16 x i32> <i32 0, i32 16, i32 1, i32 17, i32 2, i32 18, i32 3, i32 19, i32 4, i32 20, i32 5, i32 21, i32 6, i32 22, i32 7, i32 23>
  store <16 x i8> %s, ptr %q, align 16
  ret void
}

; CHECK-LABEL: piece proc
; CHECK: movaps xmmword ptr dst, xmm1
; CHECK: movaps xmmword ptr dst+16, xmm0
; CHECK-LABEL: interleave proc
; CHECK: movaps xmmword ptr [rsi], xmm
