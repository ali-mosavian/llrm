target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare {i16, i1} @llvm.sadd.with.overflow.i16(i16, i16)
define i32 @sadd16(ptr %p, ptr %q, ptr %o) {
  %a = load volatile i16, ptr %p
  %b = load volatile i16, ptr %q
  %r = call {i16, i1} @llvm.sadd.with.overflow.i16(i16 %a, i16 %b)
  %v = extractvalue {i16, i1} %r, 0
  %f = extractvalue {i16, i1} %r, 1
  %z = zext i1 %f to i8
  store i8 %z, ptr %o
  %s = sext i16 %v to i32
  ret i32 %s
}
define i64 @sext32(ptr %p) {
  %a = load volatile i32, ptr %p
  %s = sext i32 %a to i64
  ret i64 %s
}
define i32 @sext8(ptr %p) {
  %a = load volatile i8, ptr %p
  %s = sext i8 %a to i32
  ret i32 %s
}
