target datalayout = "E-m:e-p:32:16:32-i8:8:8-i16:16:16-i32:16:32-n8:16:32-a:0:16-S16"
target triple = "m68k-unknown-linux-gnu"

declare i32 @llvm.smul.fix.i32(i32, i32, i32)
declare i32 @llvm.smul.fix.sat.i32(i32, i32, i32)
declare i32 @llvm.umul.fix.i32(i32, i32, i32)
declare i32 @llvm.umul.fix.sat.i32(i32, i32, i32)

define i32 @sf(i32 %a, i32 %b) {
  %r = call i32 @llvm.smul.fix.i32(i32 %a, i32 %b, i32 15)
  ret i32 %r
}

define i32 @sfs(i32 %a, i32 %b) {
  %r = call i32 @llvm.smul.fix.sat.i32(i32 %a, i32 %b, i32 15)
  ret i32 %r
}

define i32 @uf(i32 %a, i32 %b) {
  %r = call i32 @llvm.umul.fix.i32(i32 %a, i32 %b, i32 16)
  ret i32 %r
}

define i32 @ufs(i32 %a, i32 %b) {
  %r = call i32 @llvm.umul.fix.sat.i32(i32 %a, i32 %b, i32 16)
  ret i32 %r
}
