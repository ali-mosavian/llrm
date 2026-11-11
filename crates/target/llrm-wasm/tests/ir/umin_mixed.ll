target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

declare i32 @llvm.umin.i32(i32, i32)

define i32 @clamp(i32 %x) {
  %r = call i32 @llvm.umin.i32(i32 %x, i32 255)
  ret i32 %r
}
