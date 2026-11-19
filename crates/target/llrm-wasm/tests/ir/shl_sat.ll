target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare i8 @llvm.sshl.sat.i8(i8, i8)
define i8 @sshl_sat_8(i8 %x, i8 %y) {
  %r = call i8 @llvm.sshl.sat.i8(i8 %x, i8 %y)
  ret i8 %r
}
declare i16 @llvm.sshl.sat.i16(i16, i16)
define i16 @sshl_sat_16(i16 %x, i16 %y) {
  %r = call i16 @llvm.sshl.sat.i16(i16 %x, i16 %y)
  ret i16 %r
}
declare i32 @llvm.sshl.sat.i32(i32, i32)
define i32 @sshl_sat_32(i32 %x, i32 %y) {
  %r = call i32 @llvm.sshl.sat.i32(i32 %x, i32 %y)
  ret i32 %r
}
define i32 @sshl_sat_then_sra(i32 %x, i32 %y) {
  %t = call i32 @llvm.sshl.sat.i32(i32 %x, i32 %y)
  %r = ashr i32 %t, 24
  ret i32 %r
}
