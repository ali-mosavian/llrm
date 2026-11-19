target datalayout = "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "arm64-apple-macosx10.4.0"

declare i32 @llvm.fshl.i32(i32, i32, i32)
declare i64 @llvm.fshl.i64(i64, i64, i64)
declare i32 @llvm.fshr.i32(i32, i32, i32)

define i32 @fl32c(i32 %a, i32 %b) {
  %r = call i32 @llvm.fshl.i32(i32 %a, i32 %b, i32 5)
  ret i32 %r
}

define i64 @fl64c(i64 %a, i64 %b) {
  %r = call i64 @llvm.fshl.i64(i64 %a, i64 %b, i64 70)
  ret i64 %r
}

define i32 @fr32c(i32 %a, i32 %b) {
  %r = call i32 @llvm.fshr.i32(i32 %a, i32 %b, i32 9)
  ret i32 %r
}

define i32 @fl32v(i32 %a, i32 %b, i32 %n) {
  %r = call i32 @llvm.fshl.i32(i32 %a, i32 %b, i32 %n)
  ret i32 %r
}

define i64 @fl64v(i64 %a, i64 %b, i64 %n) {
  %r = call i64 @llvm.fshl.i64(i64 %a, i64 %b, i64 %n)
  ret i64 %r
}
