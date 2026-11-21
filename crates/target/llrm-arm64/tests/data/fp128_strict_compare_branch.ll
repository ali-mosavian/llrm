target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
declare void @cold(i32)
declare i1 @llvm.experimental.constrained.fcmps.f128(fp128, fp128, metadata, metadata)
define void @q(fp128 %x, fp128 %y) strictfp {
  %c = call i1 @llvm.experimental.constrained.fcmps.f128(fp128 %x, fp128 %y, metadata !"ogt", metadata !"fpexcept.strict") strictfp
  br i1 %c, label %t, label %f
t:
  call void @cold(i32 1)
  ret void
f:
  ret void
}
