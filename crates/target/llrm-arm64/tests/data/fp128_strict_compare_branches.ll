target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
%struct.R = type { fp128, i8 }
declare fp128 @log2l(fp128)
declare fp128 @llvm.experimental.constrained.sitofp.f128.i32(i32, metadata, metadata)
declare i1 @llvm.experimental.constrained.fcmps.f128(fp128, fp128, metadata, metadata)
declare fp128 @llvm.experimental.constrained.fpext.f128.f64(double, metadata)
declare fp128 @llvm.experimental.constrained.fpext.f128.f32(float, metadata)
declare fp128 @llvm.experimental.constrained.fmul.f128(fp128, fp128, metadata, metadata)
declare void @llvm.memset.p0.i64(ptr, i8, i64, i1)
define void @checked(ptr noalias sret(%struct.R) align 16 %0, fp128 noundef %1, fp128 noundef %2, fp128 noundef %3) strictfp {
  %5 = tail call fp128 @llvm.experimental.constrained.sitofp.f128.i32(i32 0, metadata !"round.dynamic", metadata !"fpexcept.strict") strictfp
  %6 = tail call i1 @llvm.experimental.constrained.fcmps.f128(fp128 %1, fp128 %5, metadata !"olt", metadata !"fpexcept.strict") strictfp
  %7 = fneg fp128 %1
  %8 = select i1 %6, fp128 %7, fp128 %1
  %9 = tail call fp128 @llvm.experimental.constrained.fpext.f128.f64(double 0x7FEFFFFFFFFFFFFF, metadata !"fpexcept.strict") strictfp
  %10 = tail call i1 @llvm.experimental.constrained.fcmps.f128(fp128 %8, fp128 %9, metadata !"ogt", metadata !"fpexcept.strict") strictfp
  br i1 %10, label %11, label %22

11:
  %12 = tail call fp128 @llvm.experimental.constrained.sitofp.f128.i32(i32 0, metadata !"round.dynamic", metadata !"fpexcept.strict") strictfp
  %13 = tail call i1 @llvm.experimental.constrained.fcmps.f128(fp128 %2, fp128 %12, metadata !"olt", metadata !"fpexcept.strict") strictfp
  %14 = fneg fp128 %2
  %15 = select i1 %13, fp128 %14, fp128 %2
  %16 = tail call fp128 @log2l(fp128 noundef %15) nounwind optsize strictfp
  %17 = tail call fp128 @llvm.experimental.constrained.fmul.f128(fp128 %3, fp128 %16, metadata !"round.dynamic", metadata !"fpexcept.strict") strictfp
  %18 = tail call fp128 @llvm.experimental.constrained.fpext.f128.f32(float 3.276750e+04, metadata !"fpexcept.strict") strictfp
  %19 = tail call i1 @llvm.experimental.constrained.fcmps.f128(fp128 %17, fp128 %18, metadata !"oge", metadata !"fpexcept.strict") strictfp
  br i1 %19, label %20, label %22

20:
  tail call void @llvm.memset.p0.i64(ptr nonnull align 16 %0, i8 0, i64 32, i1 false) strictfp
  %21 = getelementptr inbounds nuw i8, ptr %0, i64 16
  store i8 2, ptr %21, align 16
  br label %24

22:
  store fp128 %1, ptr %0, align 16
  %23 = getelementptr inbounds nuw i8, ptr %0, i64 16
  tail call void @llvm.memset.p0.i64(ptr nonnull align 16 %23, i8 0, i64 16, i1 false)
  ret void

24:
  ret void
}
