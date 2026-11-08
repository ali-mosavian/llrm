target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare {i64, i1} @llvm.umul.with.overflow.i64(i64, i64)
define i32 @umul_64(i64 %a, i64 %b, ptr %p) {
  %r = call {i64, i1} @llvm.umul.with.overflow.i64(i64 %a, i64 %b)
  %v = extractvalue {i64, i1} %r, 0
  %o = extractvalue {i64, i1} %r, 1
  store i64 %v, ptr %p
  %z = zext i1 %o to i32
  ret i32 %z
}
declare {i64, i1} @llvm.smul.with.overflow.i64(i64, i64)
define i32 @smul_64(i64 %a, i64 %b, ptr %p) {
  %r = call {i64, i1} @llvm.smul.with.overflow.i64(i64 %a, i64 %b)
  %v = extractvalue {i64, i1} %r, 0
  %o = extractvalue {i64, i1} %r, 1
  store i64 %v, ptr %p
  %z = zext i1 %o to i32
  ret i32 %z
}
declare {i32, i1} @llvm.umul.with.overflow.i32(i32, i32)
define i32 @umul_32(i32 %a, i32 %b, ptr %p) {
  %r = call {i32, i1} @llvm.umul.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue {i32, i1} %r, 0
  %o = extractvalue {i32, i1} %r, 1
  store i32 %v, ptr %p
  %z = zext i1 %o to i32
  ret i32 %z
}
declare {i32, i1} @llvm.smul.with.overflow.i32(i32, i32)
define i32 @smul_32(i32 %a, i32 %b, ptr %p) {
  %r = call {i32, i1} @llvm.smul.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue {i32, i1} %r, 0
  %o = extractvalue {i32, i1} %r, 1
  store i32 %v, ptr %p
  %z = zext i1 %o to i32
  ret i32 %z
}
declare {i16, i1} @llvm.umul.with.overflow.i16(i16, i16)
define i32 @umul_16(i16 %a, i16 %b, ptr %p) {
  %r = call {i16, i1} @llvm.umul.with.overflow.i16(i16 %a, i16 %b)
  %v = extractvalue {i16, i1} %r, 0
  %o = extractvalue {i16, i1} %r, 1
  store i16 %v, ptr %p
  %z = zext i1 %o to i32
  ret i32 %z
}
declare {i16, i1} @llvm.smul.with.overflow.i16(i16, i16)
define i32 @smul_16(i16 %a, i16 %b, ptr %p) {
  %r = call {i16, i1} @llvm.smul.with.overflow.i16(i16 %a, i16 %b)
  %v = extractvalue {i16, i1} %r, 0
  %o = extractvalue {i16, i1} %r, 1
  store i16 %v, ptr %p
  %z = zext i1 %o to i32
  ret i32 %z
}
define i64 @trunc_store(i64 %a, ptr %p) {
  %t = trunc i64 %a to i32
  store i32 %t, ptr %p
  %r = add i64 %a, 1
  ret i64 %r
}
define i32 @umul_by_two(i32 %a, ptr %p) {
  %r = call {i32, i1} @llvm.umul.with.overflow.i32(i32 %a, i32 2)
  %v = extractvalue {i32, i1} %r, 0
  %o = extractvalue {i32, i1} %r, 1
  store i32 %v, ptr %p
  %z = zext i1 %o to i32
  ret i32 %z
}
define i32 @smul_by_two(i32 %a, ptr %p) {
  %r = call {i32, i1} @llvm.smul.with.overflow.i32(i32 %a, i32 2)
  %v = extractvalue {i32, i1} %r, 0
  %o = extractvalue {i32, i1} %r, 1
  store i32 %v, ptr %p
  %z = zext i1 %o to i32
  ret i32 %z
}
