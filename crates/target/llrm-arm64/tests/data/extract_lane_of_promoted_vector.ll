target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"
define void @f(ptr %p) {
  %v = load <4 x i32>, ptr %p
  %t = trunc <4 x i32> %v to <4 x i8>
  %a = extractelement <4 x i8> %t, i64 3
  store i8 %a, ptr %p
  %b = extractelement <4 x i8> %t, i64 0
  %q = getelementptr i8, ptr %p, i64 1
  store i8 %b, ptr %q
  ret void
}
