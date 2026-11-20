target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

declare i1 @llvm.is.fpclass.f32(float, i32)
declare i1 @llvm.is.fpclass.f64(double, i32)

define i1 @isnan(float %x) { %r = call i1 @llvm.is.fpclass.f32(float %x, i32 3) ret i1 %r }
define i1 @iszero(double %x) { %r = call i1 @llvm.is.fpclass.f64(double %x, i32 96) ret i1 %r }
define i1 @isinf(float %x) { %r = call i1 @llvm.is.fpclass.f32(float %x, i32 516) ret i1 %r }
define i1 @issubnormal(float %x) { %r = call i1 @llvm.is.fpclass.f32(float %x, i32 144) ret i1 %r }
define i1 @ispositivenormal(double %x) { %r = call i1 @llvm.is.fpclass.f64(double %x, i32 256) ret i1 %r }
define i1 @mixed(float %x) { %r = call i1 @llvm.is.fpclass.f32(float %x, i32 360) ret i1 %r }
