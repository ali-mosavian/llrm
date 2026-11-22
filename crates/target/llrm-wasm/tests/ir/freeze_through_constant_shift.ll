target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"

define i32 @sext8(i32 %x) {
  %shl = shl nsw i32 %x, 24
  %fr = freeze i32 %shl
  %sra = ashr i32 %fr, 24
  ret i32 %sra
}
