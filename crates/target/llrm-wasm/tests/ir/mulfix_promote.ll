target datalayout = "e-m:e-p:32:32-p10:8:8-p20:8:8-i64:64-i128:128-n32:64-S128-ni:1:10:20"
target triple = "wasm32-unknown-unknown"
declare i8 @llvm.smul.fix.i8(i8, i8, i32)
define i8 @smul_fix_8(i8 %x, i8 %y) {
  %r = call i8 @llvm.smul.fix.i8(i8 %x, i8 %y, i32 3)
  ret i8 %r
}
declare i16 @llvm.smul.fix.i16(i16, i16, i32)
define i16 @smul_fix_16(i16 %x, i16 %y) {
  %r = call i16 @llvm.smul.fix.i16(i16 %x, i16 %y, i32 7)
  ret i16 %r
}
declare i8 @llvm.smul.fix.sat.i8(i8, i8, i32)
define i8 @smul_fix_sat_8(i8 %x, i8 %y) {
  %r = call i8 @llvm.smul.fix.sat.i8(i8 %x, i8 %y, i32 3)
  ret i8 %r
}
declare i16 @llvm.smul.fix.sat.i16(i16, i16, i32)
define i16 @smul_fix_sat_16(i16 %x, i16 %y) {
  %r = call i16 @llvm.smul.fix.sat.i16(i16 %x, i16 %y, i32 7)
  ret i16 %r
}
declare i8 @llvm.umul.fix.i8(i8, i8, i32)
define i8 @umul_fix_8(i8 %x, i8 %y) {
  %r = call i8 @llvm.umul.fix.i8(i8 %x, i8 %y, i32 3)
  ret i8 %r
}
declare i16 @llvm.umul.fix.i16(i16, i16, i32)
define i16 @umul_fix_16(i16 %x, i16 %y) {
  %r = call i16 @llvm.umul.fix.i16(i16 %x, i16 %y, i32 7)
  ret i16 %r
}
declare i8 @llvm.umul.fix.sat.i8(i8, i8, i32)
define i8 @umul_fix_sat_8(i8 %x, i8 %y) {
  %r = call i8 @llvm.umul.fix.sat.i8(i8 %x, i8 %y, i32 3)
  ret i8 %r
}
declare i16 @llvm.umul.fix.sat.i16(i16, i16, i32)
define i16 @umul_fix_sat_16(i16 %x, i16 %y) {
  %r = call i16 @llvm.umul.fix.sat.i16(i16 %x, i16 %y, i32 7)
  ret i16 %r
}
declare i8 @llvm.sadd.sat.i8(i8, i8)
define i8 @sadd_sat_8(i8 %x, i8 %y) {
  %r = call i8 @llvm.sadd.sat.i8(i8 %x, i8 %y)
  ret i8 %r
}
declare i16 @llvm.sadd.sat.i16(i16, i16)
define i16 @sadd_sat_16(i16 %x, i16 %y) {
  %r = call i16 @llvm.sadd.sat.i16(i16 %x, i16 %y)
  ret i16 %r
}
declare i8 @llvm.uadd.sat.i8(i8, i8)
define i8 @uadd_sat_8(i8 %x, i8 %y) {
  %r = call i8 @llvm.uadd.sat.i8(i8 %x, i8 %y)
  ret i8 %r
}
declare i16 @llvm.uadd.sat.i16(i16, i16)
define i16 @uadd_sat_16(i16 %x, i16 %y) {
  %r = call i16 @llvm.uadd.sat.i16(i16 %x, i16 %y)
  ret i16 %r
}
declare i8 @llvm.ssub.sat.i8(i8, i8)
define i8 @ssub_sat_8(i8 %x, i8 %y) {
  %r = call i8 @llvm.ssub.sat.i8(i8 %x, i8 %y)
  ret i8 %r
}
declare i16 @llvm.ssub.sat.i16(i16, i16)
define i16 @ssub_sat_16(i16 %x, i16 %y) {
  %r = call i16 @llvm.ssub.sat.i16(i16 %x, i16 %y)
  ret i16 %r
}
declare i8 @llvm.usub.sat.i8(i8, i8)
define i8 @usub_sat_8(i8 %x, i8 %y) {
  %r = call i8 @llvm.usub.sat.i8(i8 %x, i8 %y)
  ret i8 %r
}
declare i16 @llvm.usub.sat.i16(i16, i16)
define i16 @usub_sat_16(i16 %x, i16 %y) {
  %r = call i16 @llvm.usub.sat.i16(i16 %x, i16 %y)
  ret i16 %r
}
declare i8 @llvm.ushl.sat.i8(i8, i8)
define i8 @ushl_sat_8(i8 %x, i8 %y) {
  %r = call i8 @llvm.ushl.sat.i8(i8 %x, i8 %y)
  ret i8 %r
}
declare i16 @llvm.ushl.sat.i16(i16, i16)
define i16 @ushl_sat_16(i16 %x, i16 %y) {
  %r = call i16 @llvm.ushl.sat.i16(i16 %x, i16 %y)
  ret i16 %r
}
