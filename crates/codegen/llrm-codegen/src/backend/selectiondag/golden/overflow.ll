target datalayout = "e-m:e-p:32:32-f64:32:64-f80:32-n8:16:32-S128"
target triple = "i486-unknown-linux-gnu"

declare { i32, i1 } @llvm.sadd.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.uadd.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.ssub.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.usub.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.smul.with.overflow.i32(i32, i32)
declare { i32, i1 } @llvm.umul.with.overflow.i32(i32, i32)
declare { i16, i1 } @llvm.uadd.with.overflow.i16(i16, i16)

define i32 @f(i32 %a, i32 %b, ptr %o) {
  %s = call { i32, i1 } @llvm.sadd.with.overflow.i32(i32 %a, i32 %b)
  %sv = extractvalue { i32, i1 } %s, 0
  %so = extractvalue { i32, i1 } %s, 1
  %u = call { i32, i1 } @llvm.uadd.with.overflow.i32(i32 %a, i32 %b)
  %uv = extractvalue { i32, i1 } %u, 0
  %uo = extractvalue { i32, i1 } %u, 1
  %d = call { i32, i1 } @llvm.ssub.with.overflow.i32(i32 %a, i32 %b)
  %dv = extractvalue { i32, i1 } %d, 0
  %e = call { i32, i1 } @llvm.usub.with.overflow.i32(i32 %a, i32 %b)
  %ev = extractvalue { i32, i1 } %e, 0
  %eo = extractvalue { i32, i1 } %e, 1
  %m = call { i32, i1 } @llvm.smul.with.overflow.i32(i32 %a, i32 %b)
  %mv = extractvalue { i32, i1 } %m, 0
  %n = call { i32, i1 } @llvm.umul.with.overflow.i32(i32 %a, i32 %b)
  %nv = extractvalue { i32, i1 } %n, 0
  %no = extractvalue { i32, i1 } %n, 1
  %x1 = xor i1 %so, %uo
  %x2 = xor i1 %x1, %eo
  %x3 = xor i1 %x2, %no
  %z = zext i1 %x3 to i32
  %t1 = add i32 %sv, %uv
  %t2 = add i32 %t1, %dv
  %t3 = add i32 %t2, %ev
  %t4 = add i32 %t3, %mv
  %t5 = add i32 %t4, %nv
  %t6 = add i32 %t5, %z
  ret i32 %t6
}

define i1 @g(i16 %a, i16 %b) {
  %u = call { i16, i1 } @llvm.uadd.with.overflow.i16(i16 %a, i16 %b)
  %o = extractvalue { i16, i1 } %u, 1
  ret i1 %o
}

define zeroext i1 @h(i1 %a) {
  ret i1 %a
}

define signext i8 @s(i8 %a) {
  ret i8 %a
}

define i16 @w(i1 %a) {
  %z = zext i1 %a to i16
  ret i16 %z
}
