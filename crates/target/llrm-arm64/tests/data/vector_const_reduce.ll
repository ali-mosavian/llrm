target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define i64 @f(ptr %p) {
  %v = load <8 x i16>, ptr %p
  %s = sext <8 x i16> %v to <8 x i64>
  %m = mul nsw <8 x i64> %s, <i64 70, i64 68, i64 66, i64 64, i64 62, i64 60, i64 58, i64 56>
  %a = add nsw <8 x i64> %m, <i64 -5820, i64 -5340, i64 -4920, i64 -4560, i64 -4260, i64 -4020, i64 -3840, i64 -3720>
  %r = tail call i64 @llvm.vector.reduce.add.v8i64(<8 x i64> %a)
  ret i64 %r
}
declare i64 @llvm.vector.reduce.add.v8i64(<8 x i64>)
