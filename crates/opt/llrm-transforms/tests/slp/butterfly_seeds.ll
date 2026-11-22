target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"

; stft_fixed -Os: one fixed-point butterfly. The four results go to four
; different arrays, so no store chain exists; the pin bundles the stores'
; value operands (vectorizeNonVectorizableInsts) and, pricing at code size
; under optsize, makes the four `sdiv x, 2` one <4 x i64> (cost -2).
define void @bf(ptr %a, ptr %b, ptr %c, ptr %d, i64 %wr, i64 %wi) optsize {
entry:
  %c0 = load i64, ptr %c, align 8
  %m0 = mul nsw i64 %c0, %wr
  %s0 = ashr i64 %m0, 9
  %d0 = load i64, ptr %d, align 8
  %m1 = mul nsw i64 %d0, %wi
  %s1 = ashr i64 %m1, 9
  %tr = sub nsw i64 %s0, %s1
  %m2 = mul nsw i64 %c0, %wi
  %s2 = ashr i64 %m2, 9
  %m3 = mul nsw i64 %d0, %wr
  %s3 = ashr i64 %m3, 9
  %ti = add nsw i64 %s3, %s2
  %ar = load i64, ptr %a, align 8
  %ai = load i64, ptr %b, align 8
  %n1 = add nsw i64 %tr, %ar
  %q1 = sdiv i64 %n1, 2
  store i64 %q1, ptr %a, align 8
  %n2 = add nsw i64 %ti, %ai
  %q2 = sdiv i64 %n2, 2
  store i64 %q2, ptr %b, align 8
  %n3 = sub nsw i64 %ar, %tr
  %q3 = sdiv i64 %n3, 2
  store i64 %q3, ptr %c, align 8
  %n4 = sub nsw i64 %ai, %ti
  %q4 = sdiv i64 %n4, 2
  store i64 %q4, ptr %d, align 8
  ret void
}
