target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

declare { i32, i1 } @llvm.uadd.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.usub.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.umul.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32)
declare { i64, i1 } @llvm.umul.with.overflow.i64(i64, i64)

; Each returns the wrapped value in the low 32 bits and the overflow bit above.
define i64 @uaddo(i32 %a, i32 %b) {
  %p = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue { i32, i1 } %p, 0
  %o = extractvalue { i32, i1 } %p, 1
  %lo = zext i32 %v to i64
  %hi = zext i1 %o to i64
  %s = shl i64 %hi, 32
  %r = or i64 %lo, %s
  ret i64 %r
}
define i64 @usubo(i32 %a, i32 %b) {
  %p = call { i32, i1 } @llvm.usub.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue { i32, i1 } %p, 0
  %o = extractvalue { i32, i1 } %p, 1
  %lo = zext i32 %v to i64
  %hi = zext i1 %o to i64
  %s = shl i64 %hi, 32
  %r = or i64 %lo, %s
  ret i64 %r
}
define i64 @saddo(i32 %a, i32 %b) {
  %p = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue { i32, i1 } %p, 0
  %o = extractvalue { i32, i1 } %p, 1
  %lo = zext i32 %v to i64
  %hi = zext i1 %o to i64
  %s = shl i64 %hi, 32
  %r = or i64 %lo, %s
  ret i64 %r
}
define i64 @ssubo(i32 %a, i32 %b) {
  %p = call { i32, i1 } @llvm.ssub.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue { i32, i1 } %p, 0
  %o = extractvalue { i32, i1 } %p, 1
  %lo = zext i32 %v to i64
  %hi = zext i1 %o to i64
  %s = shl i64 %hi, 32
  %r = or i64 %lo, %s
  ret i64 %r
}
define i64 @umulo(i32 %a, i32 %b) {
  %p = call { i32, i1 } @llvm.umul.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue { i32, i1 } %p, 0
  %o = extractvalue { i32, i1 } %p, 1
  %lo = zext i32 %v to i64
  %hi = zext i1 %o to i64
  %s = shl i64 %hi, 32
  %r = or i64 %lo, %s
  ret i64 %r
}
define i64 @smulo(i32 %a, i32 %b) {
  %p = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue { i32, i1 } %p, 0
  %o = extractvalue { i32, i1 } %p, 1
  %lo = zext i32 %v to i64
  %hi = zext i1 %o to i64
  %s = shl i64 %hi, 32
  %r = or i64 %lo, %s
  ret i64 %r
}
; The 64-bit product has no wider multiply: wrapped value xor overflow.
define i64 @umulo64(i64 %a, i64 %b) {
  %p = call { i64, i1 } @llvm.umul.with.overflow.i64(i64 %a, i64 %b)
  %v = extractvalue { i64, i1 } %p, 0
  %o = extractvalue { i64, i1 } %p, 1
  %z = zext i1 %o to i64
  %r = xor i64 %v, %z
  ret i64 %r
}
