target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"
declare {i32, i1} @llvm.sadd.with.overflow.i32(i32, i32)
declare {i64, i1} @llvm.uadd.with.overflow.i64(i64, i64)


define i32 @ao(i32 %a, i32 %b) {
  %p = call {i32, i1} @llvm.sadd.with.overflow.i32(i32 %a, i32 %b)
  %v = extractvalue {i32, i1} %p, 0
  %o = extractvalue {i32, i1} %p, 1
  %z = zext i1 %o to i32
  %s = add i32 %v, %z
  ret i32 %s
}

define i1 @no(i64 %a, i64 %b) {
  %p = call {i64, i1} @llvm.uadd.with.overflow.i64(i64 %a, i64 %b)
  %o = extractvalue {i64, i1} %p, 1
  %n = xor i1 %o, true
  ret i1 %n
}

