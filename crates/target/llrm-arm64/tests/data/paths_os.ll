target datalayout = "e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32"
target triple = "aarch64-unknown-linux-gnu"

define i64 @paths(i32 %n, i32 %k) {
entry:
  %n0 = icmp eq i32 %n, 0
  br i1 %n0, label %base, label %second

second:
  %k0 = icmp eq i32 %k, 0
  br i1 %k0, label %base, label %step

step:
  %m = sub nsw i32 %n, 1
  %a = call i64 @paths(i32 %m, i32 %k)
  %j = sub nsw i32 %k, 1
  %b = call i64 @paths(i32 %n, i32 %j)
  %sum = add nsw i64 %a, %b
  ret i64 %sum

base:
  ret i64 1
}
