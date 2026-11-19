target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define i32 @rev(i32 %x) {
  %t = trunc i32 %x to i8
  %r = call i8 @llvm.bitreverse.i8(i8 %t)
  %z = zext i8 %r to i32
  ret i32 %z
}

declare i8 @llvm.bitreverse.i8(i8)
