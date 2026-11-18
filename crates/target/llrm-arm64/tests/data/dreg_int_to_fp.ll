target triple = "aarch64-unknown-linux-gnu"
target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
define void @u(ptr %p, ptr %o) {
  %v = load <2 x i32>, ptr %p, align 8
  %f = uitofp <2 x i32> %v to <2 x double>
  store <2 x double> %f, ptr %o, align 16
  ret void
}
define void @s(ptr %p, ptr %o) {
  %v = load <2 x i32>, ptr %p, align 8
  %f = sitofp <2 x i32> %v to <2 x double>
  store <2 x double> %f, ptr %o, align 16
  ret void
}
