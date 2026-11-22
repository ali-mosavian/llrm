target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define i1 @is_inf(i64 %x) {
  %m = and i64 %x, 9218868437227405312
  %c = icmp eq i64 %m, 9218868437227405312
  ret i1 %c
}

define i1 @all_set(i32 %x) {
  %m = and i32 %x, 65280
  %c = icmp ne i32 %m, 65280
  ret i1 %c
}
